//! The plugin azure-devops (#881), against an Azure DevOps of its own on this machine and a real
//! repository in a folder: a pull request is fetched from the source branch the API says (Azure
//! DevOps has no `refs/pull/N/head`), merged onto its target and built; the statuses go to the
//! pull request with the PAT as HTTP basic; a branch's go to the commit; and a PAT refused says so.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests fail loudly and say what they skip"
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CiPlugin};
use raven_ci_plugin_azure_devops::AzureDevopsPlugin;
use raven_ci_plugin_credentials::CredentialsPlugin;
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use ravenrs_git::{Credential, Git};
use serde_json::{Value, json};

const PAT: &str = "pat-de-teste-0123456789";
const API: &str = "/org/proj/_apis/git/repositories/repo";

/// What the Azure DevOps of the test was asked: `(method, path, authorization, body)`.
#[derive(Default)]
struct Devops {
    asked: Vec<(String, String, String, String)>,
    refuse: bool,
}

type Shared = Arc<Mutex<Devops>>;

fn answer(devops: &Shared, method: &str, target: &str, auth: &str, body: String) -> (u16, String) {
    let mut devops = devops.lock().expect("lock");
    devops.asked.push((method.to_owned(), target.to_owned(), auth.to_owned(), body));
    if devops.refuse {
        // What Azure DevOps answers to a PAT it does not take: the page to sign in.
        return (203, "<html>entrar</html>".to_owned());
    }
    let path = target.split('?').next().unwrap_or_default();
    match (method, path.strip_prefix(API).unwrap_or_default()) {
        ("GET", "/pullRequests/7") => (200, json!({ "sourceRefName": "refs/heads/feature" }).to_string()),
        ("POST", "/pullRequests/7/statuses") => (201, "{}".to_owned()),
        ("POST", status) if status.starts_with("/commits/") && status.ends_with("/statuses") => {
            (201, "{}".to_owned())
        }
        _ => (404, json!({ "message": format!("não há {path}") }).to_string()),
    }
}

fn serve_one(devops: &Shared, stream: TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.split_whitespace();
    let (method, target) =
        (parts.next().unwrap_or_default().to_owned(), parts.next().unwrap_or_default().to_owned());
    let (mut length, mut auth) = (0, String::new());
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        let (name, value) = header.split_once(':').unwrap_or_default();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().unwrap_or(0),
            "authorization" => auth = value.trim().to_owned(),
            _ => {}
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let (status, text) = answer(devops, &method, &target, &auth, String::from_utf8_lossy(&body).into_owned());
    let mut stream = reader.into_inner();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
}

/// An Azure DevOps on a free port; the address of its collection.
fn azure(devops: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let address = format!("http://{}/org", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            serve_one(&devops, stream);
        }
    });
    address
}

/// A folder of its own, removed when dropped.
struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn folder(name: &str) -> Folder {
    let dir = std::env::temp_dir().join(format!("raven-ci-azure-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    Folder(dir)
}

/// A repository at `<dir>/org/proj/_git/repo` with `main` and `feature`, and no `refs/pull`: its
/// address and the commit of `main`.
fn repository(dir: &Path) -> (String, String) {
    let repo = dir.join("org").join("proj").join("_git").join("repo");
    std::fs::create_dir_all(&repo).expect("a folder");
    let git = Git::at(&repo).with_identity("Teste", "teste@localhost");
    let run = |args: &[&str]| {
        git.run(args).expect("git");
    };
    run(&["init", "--quiet"]);
    run(&["checkout", "--quiet", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "main\n").expect("written");
    run(&["add", "a.txt"]);
    run(&["commit", "--quiet", "-m", "main"]);
    let main = git.head().expect("head");
    run(&["checkout", "--quiet", "-b", "feature"]);
    std::fs::write(repo.join("b.txt"), "do PR\n").expect("written");
    run(&["add", "b.txt"]);
    run(&["commit", "--quiet", "-m", "pr"]);
    run(&["checkout", "--quiet", "main"]);
    (repo.display().to_string(), main)
}

fn has_git() -> bool {
    let found = std::process::Command::new("git").arg("--version").output().is_ok();
    if !found {
        eprintln!("pulado: sem git nesta máquina");
    }
    found
}

fn texts(ci: &Ci, job: &str) -> String {
    let number = ci.builds(job).expect("builds").first().expect("a build").number;
    ci.build_log(job, number, 0).expect("log").into_iter().map(|l| l.text).collect::<Vec<_>>().join("\n")
}

/// A core with the plugins, the PAT in the vault and the API of the test.
fn core(data: &Path, api: &str) -> Ci {
    let ci = Ci::open(data).expect("a core");
    for plugin in [
        Arc::new(AzureDevopsPlugin::new()) as Arc<dyn CiPlugin>,
        Arc::new(ShellPlugin::new()),
        Arc::new(TomlFormatPlugin),
        Arc::new(CredentialsPlugin),
    ] {
        ci.add_plugin(plugin);
    }
    ci.apply_plugins().expect("on");
    ci.configure_plugin("azure-devops", json!({ "api": api })).expect("the api");
    ci.save_credential("azure", "token", &json!({ "token": PAT }), None).expect("the PAT");
    ci
}

const PIPELINE: &str = "[[stages]]\nname = \"Testes\"\nsteps = [\"cat b.txt\"]\n";

/// The `(path, state, authorization)` of each status sent.
fn statuses(devops: &Shared) -> Vec<(String, String, String, Value)> {
    devops
        .lock()
        .expect("lock")
        .asked
        .iter()
        .filter(|(method, ..)| method == "POST")
        .map(|(_, path, auth, body)| {
            let sent: Value = serde_json::from_str(body).expect("json");
            let path = path.split('?').next().unwrap_or_default().trim_start_matches(API).to_owned();
            (path, sent["state"].as_str().unwrap_or_default().to_owned(), auth.clone(), sent)
        })
        .collect()
}

#[test]
fn a_pull_request_is_fetched_from_its_source_merged_built_and_told_on_the_pull_request() {
    if !has_git() {
        return;
    }
    let repo = folder("repo");
    let (url, main) = repository(&repo.0);
    let devops: Shared = Arc::default();
    let api = azure(devops.clone());
    let data = folder("dados");
    let ci = core(&data.0, &api);
    let settings = json!({
        "scm": {
            "id": "azure-devops", "url": url, "credential": "azure", "merge_onto_target": true,
            "reference": { "pull_request": { "number": 7, "target": "main" } },
        },
        "status": [{ "id": "azure-devops", "repository": url, "credential": "azure", "context": "raven-ci" }],
    });
    ci.save_job(JobSpec::new("site", "toml", PIPELINE).settings(settings)).expect("the job");
    ci.request_build(BuildRequest::new("site", "teste")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    let log = texts(&ci, "site");
    assert_eq!(ci.builds("site").expect("builds")[0].status, BuildStatus::Succeeded, "{log}");
    assert!(log.contains("o PR 7 vem de refs/heads/feature"), "{log}");
    assert!(log.contains("mesclando o PR 7 em main"), "{log}");
    assert!(log.contains("do PR"), "b.txt came from the source branch: {log}");
    assert!(!log.contains(PAT), "the PAT never shows: {log}");

    let basic = Credential::basic("raven-ci", PAT).http_authorization().to_owned();
    let sent = statuses(&devops);
    let states: Vec<(&str, &str)> = sent.iter().map(|(path, state, ..)| (path.as_str(), state.as_str())).collect();
    assert_eq!(
        states,
        [("/pullRequests/7/statuses", "pending"), ("/pullRequests/7/statuses", "succeeded")],
        "{sent:?}"
    );
    for (.., auth, body) in &sent {
        assert_eq!(auth, &basic, "the PAT goes as HTTP basic");
        assert_eq!(body["context"], json!({ "name": "raven-ci", "genre": "raven-ci" }));
    }
    let asked = devops.lock().expect("lock").asked.clone();
    assert!(asked.iter().any(|(m, p, a, _)| m == "GET" && p.contains("/pullRequests/7?api-version=7.1") && a == &basic));
    assert_ne!(main, "", "the commit of main");
}

#[test]
fn a_branch_is_told_on_its_commit_and_a_pat_refused_says_so() {
    if !has_git() {
        return;
    }
    let repo = folder("ramo");
    let (url, main) = repository(&repo.0);
    let devops: Shared = Arc::default();
    let api = azure(devops.clone());
    let data = folder("dados-ramo");
    let ci = core(&data.0, &api);
    let settings = json!({
        "scm": { "id": "azure-devops", "url": url, "credential": "azure", "reference": { "branch": "main" } },
        "status": [{ "id": "azure-devops", "repository": url, "credential": "azure", "genre": "ci" }],
    });
    let pipeline = "[[stages]]\nname = \"Testes\"\nsteps = [\"cat a.txt\"]\n";
    ci.save_job(JobSpec::new("ramo", "toml", pipeline).settings(settings)).expect("the job");
    ci.request_build(BuildRequest::new("ramo", "teste")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.builds("ramo").expect("builds")[0].status, BuildStatus::Succeeded, "{}", texts(&ci, "ramo"));
    let sent = statuses(&devops);
    let commit_path = format!("/commits/{main}/statuses");
    assert!(sent.iter().all(|(path, ..)| path == &commit_path), "{sent:?}");
    assert_eq!(sent.last().map(|s| s.1.as_str()), Some("succeeded"));
    assert_eq!(sent[0].3["context"], json!({ "name": "raven-ci/ramo", "genre": "ci" }));

    // A PAT refused: the pull request cannot be asked about, and the build says why.
    devops.lock().expect("lock").refuse = true;
    let settings = json!({
        "scm": {
            "id": "azure-devops", "url": url, "credential": "azure",
            "reference": { "pull_request": { "number": 7, "target": "main" } },
        },
    });
    ci.save_job(JobSpec::new("recusado", "toml", PIPELINE).settings(settings)).expect("the job");
    ci.request_build(BuildRequest::new("recusado", "teste")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    let build = ci.builds("recusado").expect("builds")[0].clone();
    assert_eq!(build.status, BuildStatus::Failed);
    let said = format!("{build:?} {}", texts(&ci, "recusado"));
    assert!(said.contains("203") && said.contains("PAT"), "{said}");
}
