//! The plugin github (#877), against a GitHub of its own on this machine and a real repository:
//! the scan of a job of many branches makes one job per branch chosen and per pull request (forks
//! left out), reads the pipeline of each head (a pull request with none takes its target's, #958),
//! builds the pull request merged, sends the statuses
//! to the commits, asks for nothing when nothing moved, and removes the job of a closed pull request.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests fail loudly and say what they skip"
)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildStatus, CiPlugin, HeadKind};
use raven_ci_plugin_credentials::CredentialsPlugin;
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_git::GitPlugin;
use raven_ci_plugin_github::GithubPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use ravenrs_git::Git;
use serde_json::{Value, json};

const TOKEN: &str = "tok-de-teste-123456";

/// What the GitHub of the test holds, and what it was asked.
#[derive(Default)]
struct Hub {
    branches: Value,
    pulls: Value,
    files: BTreeMap<String, String>,
    /// `(method, path, authorization, body)`.
    asked: Vec<(String, String, String, String)>,
    refuse: bool,
}

type Shared = Arc<Mutex<Hub>>;

fn answer(hub: &Shared, method: &str, target: &str, auth: &str, body: String) -> (u16, String) {
    let mut hub = hub.lock().expect("lock");
    hub.asked.push((method.to_owned(), target.to_owned(), auth.to_owned(), body));
    if hub.refuse {
        return (401, json!({ "message": "Bad credentials" }).to_string());
    }
    let path = target.split('?').next().unwrap_or_default();
    match (method, path) {
        ("GET", "/repos/dono/repo/branches") => (200, hub.branches.to_string()),
        ("GET", "/repos/dono/repo/pulls") => (200, hub.pulls.to_string()),
        ("GET", file) if file.starts_with("/repos/dono/repo/contents/") => {
            let commit = target.split("ref=").nth(1).unwrap_or_default();
            match hub.files.get(commit) {
                Some(text) => (200, text.clone()),
                None => (404, json!({ "message": "Not Found" }).to_string()),
            }
        }
        ("POST", status) if status.starts_with("/repos/dono/repo/statuses/") => (201, "{}".to_owned()),
        _ => (404, json!({ "message": "Not Found" }).to_string()),
    }
}

fn serve_one(hub: &Shared, stream: TcpStream) {
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
    let (status, text) = answer(hub, &method, &target, &auth, String::from_utf8_lossy(&body).into_owned());
    let mut stream = reader.into_inner();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
}

/// A GitHub on a free port; its address.
fn github(hub: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let address = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            serve_one(&hub, stream);
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
    let dir = std::env::temp_dir().join(format!("raven-ci-github-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    Folder(dir)
}

/// A repository with `main` and the pull request 1; the commits of both.
fn repository(dir: &Path) -> (String, String) {
    let git = Git::at(dir).with_identity("Teste", "teste@localhost");
    let run = |args: &[&str]| {
        git.run(args).expect("git");
    };
    run(&["init", "--quiet"]);
    run(&["checkout", "--quiet", "-b", "main"]);
    std::fs::write(dir.join("a.txt"), "main\n").expect("written");
    run(&["add", "a.txt"]);
    run(&["commit", "--quiet", "-m", "main"]);
    let main = git.head().expect("head");
    run(&["checkout", "--quiet", "-b", "pr"]);
    std::fs::write(dir.join("b.txt"), "do PR\n").expect("written");
    run(&["add", "b.txt"]);
    run(&["commit", "--quiet", "-m", "pr"]);
    let pr = git.head().expect("head");
    run(&["update-ref", "refs/pull/1/head", &pr]);
    run(&["checkout", "--quiet", "main"]);
    (main, pr)
}

fn pull(number: u64, sha: &str, from: &str) -> Value {
    json!({ "number": number, "head": { "sha": sha, "repo": { "full_name": from } }, "base": { "ref": "main" } })
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

#[test]
fn a_job_of_many_branches_builds_main_and_the_pull_request_merged_and_tells_github() {
    if !has_git() {
        return;
    }
    let repo = folder("repo");
    let (main, pr) = repository(&repo.0);
    let hub: Shared = Arc::default();
    {
        let mut hub = hub.lock().expect("lock");
        hub.branches = json!([
            { "name": "main", "commit": { "sha": main } },
            { "name": "rascunho", "commit": { "sha": main } },
        ]);
        hub.pulls = json!([pull(1, &pr, "dono/repo"), pull(2, "f0f0f0f", "outro/repo")]);
        hub.files
            .insert(main.clone(), "[[stages]]\nname = \"Do repo\"\nsteps = [\"echo do-repo\"]\n".to_owned());
    }
    let api = github(hub.clone());
    let data = folder("dados");
    let ci = Ci::open(&data.0).expect("a core");
    for plugin in [
        Arc::new(GithubPlugin::new()) as Arc<dyn CiPlugin>,
        Arc::new(GitPlugin::new()),
        Arc::new(ShellPlugin::new()),
        Arc::new(TomlFormatPlugin),
        Arc::new(CredentialsPlugin),
    ] {
        ci.add_plugin(plugin);
    }
    ci.apply_plugins().expect("on");
    ci.save_credential("github", "token", &json!({ "token": TOKEN }), None, None).expect("the token");
    let url = repo.0.display().to_string();
    let settings = json!({
        "branches": { "id": "github", "repository": "dono/repo", "credential": "github", "api": api },
        "scm": { "id": "git", "url": url },
        "status": [{ "id": "github", "repository": "dono/repo", "credential": "github", "context": "raven-ci", "api": api }],
    });
    let model = "[[stages]]\nname = \"Do modelo\"\nsteps = [\"echo do-modelo\"]\n";
    ci.save_job(JobSpec::new("repo", "toml", model).settings(settings)).expect("the model");

    let changes = ci.scan("repo").expect("scanned");
    assert_eq!(changes.len(), 2, "{changes:?}");
    let jobs: Vec<String> = ci.jobs().expect("jobs").into_iter().map(|job| job.name).collect();
    assert_eq!(jobs, ["repo", "repo/PR-1", "repo/main"], "no branch not chosen, no fork");
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(
        ci.builds("repo/main").expect("builds")[0].status,
        BuildStatus::Succeeded,
        "{}",
        texts(&ci, "repo/main")
    );
    assert_eq!(
        ci.builds("repo/PR-1").expect("builds")[0].status,
        BuildStatus::Succeeded,
        "{}",
        texts(&ci, "repo/PR-1")
    );
    assert!(
        texts(&ci, "repo/main").contains("do-repo"),
        "the pipeline of the branch: {}",
        texts(&ci, "repo/main")
    );
    let of_pr = texts(&ci, "repo/PR-1");
    assert!(of_pr.contains("do-repo"), "a pull request with no file runs its target's (#958): {of_pr}");
    assert!(of_pr.contains("mesclando o PR 1 em main"), "{of_pr}");

    let statuses: Vec<(String, String, String)> = hub
        .lock()
        .expect("lock")
        .asked
        .iter()
        .filter(|(method, ..)| method == "POST")
        .map(|(_, path, auth, body)| {
            let sent: Value = serde_json::from_str(body).expect("json");
            (
                path.rsplit('/').next().unwrap_or_default().to_owned(),
                sent["state"].as_str().unwrap_or_default().to_owned(),
                auth.clone(),
            )
        })
        .collect();
    for commit in [&main, &pr] {
        let states: Vec<&str> =
            statuses.iter().filter(|(sha, ..)| sha == commit).map(|(_, state, _)| state.as_str()).collect();
        assert_eq!(states, ["pending", "success"], "the statuses of {commit}: {statuses:?}");
    }
    assert!(statuses.iter().all(|(.., auth)| auth == &format!("Bearer {TOKEN}")), "{statuses:?}");

    // Nothing moved: no build.
    assert!(ci.scan("repo").expect("scanned").is_empty());
    // The pull request closed: its job goes.
    hub.lock().expect("lock").pulls = json!([]);
    let changes = ci.scan("repo").expect("scanned");
    assert!(changes.iter().any(|c| c.starts_with("repo/PR-1: removido")), "{changes:?}");
    assert!(ci.job("repo/PR-1").is_err());
}

/// A pipeline of one stage that echoes `word`.
fn stage(word: &str) -> String {
    format!("[[stages]]\nname = \"Um\"\nsteps = [\"echo {word}\"]\n")
}

/// A core with the plugins of a job of many branches on GitHub and the token in its vault.
fn core_of(data: &Path) -> Ci {
    let ci = Ci::open(data).expect("a core");
    for plugin in [
        Arc::new(GithubPlugin::new()) as Arc<dyn CiPlugin>,
        Arc::new(GitPlugin::new()),
        Arc::new(ShellPlugin::new()),
        Arc::new(TomlFormatPlugin),
        Arc::new(CredentialsPlugin),
    ] {
        ci.add_plugin(plugin);
    }
    ci.apply_plugins().expect("on");
    ci.save_credential("github", "token", &json!({ "token": TOKEN }), None, None).expect("the token");
    ci
}

/// The pipeline of the job `repo/PR-1` after a scan of a model whose `scm` is `scm`.
fn pipeline_of_the_pull_request(hub: &Shared, repo: &Path, scm: Value, name: &str) -> String {
    let api = github(hub.clone());
    let data = folder(name);
    let ci = core_of(&data.0);
    let mut scm = scm;
    scm["id"] = json!("git");
    scm["url"] = json!(repo.display().to_string());
    let settings = json!({
        "branches": { "id": "github", "repository": "dono/repo", "credential": "github", "api": api },
        "scm": scm,
    });
    let model = "[[stages]]\nname = \"Do modelo\"\nsteps = [\"echo do-modelo\"]\n";
    ci.save_job(JobSpec::new("repo", "toml", model).settings(settings)).expect("the model");
    ci.scan("repo").expect("scanned");
    ci.job("repo/PR-1").expect("the pull request").pipeline
}

#[test]
fn a_pull_request_keeps_its_own_pipeline_and_one_built_alone_never_takes_its_targets() {
    if !has_git() {
        return;
    }
    let repo = folder("repo-958");
    let (main, pr) = repository(&repo.0);
    let hub: Shared = Arc::default();
    {
        let mut hub = hub.lock().expect("lock");
        hub.branches = json!([{ "name": "main", "commit": { "sha": main } }]);
        hub.pulls = json!([pull(1, &pr, "dono/repo")]);
        hub.files.insert(main.clone(), stage("do-destino"));
    }
    // Built alone, a pull request with no file never reads its target's: the model's.
    let alone = pipeline_of_the_pull_request(&hub, &repo.0, json!({ "merge_onto_target": false }), "sozinho");
    assert!(alone.contains("do-modelo"), "{alone}");
    // Merged (the default), it reads its target's.
    let merged = pipeline_of_the_pull_request(&hub, &repo.0, json!({}), "mesclado");
    assert!(merged.contains("do-destino"), "{merged}");
    // With a file of its own, it keeps it.
    hub.lock().expect("lock").files.insert(pr.clone(), stage("do-pr"));
    let own = pipeline_of_the_pull_request(&hub, &repo.0, json!({}), "proprio");
    assert!(own.contains("do-pr"), "{own}");
}

#[test]
fn forks_come_only_when_asked_and_a_refusal_says_what_github_said() {
    let hub: Shared = Arc::default();
    {
        let mut hub = hub.lock().expect("lock");
        hub.branches = json!([]);
        hub.pulls = json!([
            pull(2, "f0f0f0f", "outro/repo"),
            json!({ "number": 3, "head": { "sha": "a1", "repo": null }, "base": { "ref": "main" } })
        ]);
    }
    let api = github(hub.clone());
    let plugin = GithubPlugin::new();
    plugin
        .start(Arc::new(raven_ci_plugin_api::testing::FakeHost::new(
            "github",
            &[raven_ci_plugin_api::Permission::Network],
        )))
        .expect("on");
    let source = plugin.branch_sources()[0].clone();
    let none = source.discover(&json!({ "repository": "dono/repo", "api": api })).expect("found");
    assert!(none.is_empty(), "{none:?}");
    let forks =
        source.discover(&json!({ "repository": "dono/repo", "api": api, "forks": true })).expect("found");
    assert!(
        forks.iter().all(|head| matches!(head.kind, HeadKind::PullRequest { from_fork: true, .. })),
        "{forks:?}"
    );
    assert_eq!(forks.len(), 2, "a pull request whose fork was deleted counts as a fork");
    hub.lock().expect("lock").refuse = true;
    let refused = source.discover(&json!({ "repository": "dono/repo", "api": api })).expect_err("refused");
    assert!(
        refused.to_string().contains("401") && refused.to_string().contains("Bad credentials"),
        "{refused}"
    );
    let wrong = source.discover(&json!({ "repository": "sem-barra", "api": api })).expect_err("a name");
    assert!(wrong.to_string().contains("dono/nome"), "{wrong}");
}
