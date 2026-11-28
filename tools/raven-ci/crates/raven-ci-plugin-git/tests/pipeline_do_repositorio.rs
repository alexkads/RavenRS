//! The pipeline read from the repository itself on any git server (#928): a job of many branches
//! over `file://`, each branch with its own `.raven-ci.toml`; a change in one branch that changes
//! only its job; a job of one branch that runs the `Jenkinsfile` of its tip; and a credential
//! that shows nowhere. No network: the repositories are made here. The tests say they skip when
//! there is no `git`.
#![allow(clippy::expect_used, clippy::print_stderr, reason = "tests fail loudly and say what they skip")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::testing::FakeHost;
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CiPlugin, Head, HeadKind, Permission};
use raven_ci_plugin_format_jenkinsfile::JenkinsfilePlugin;
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_git::GitPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use ravenrs_git::Git;
use serde_json::json;

fn has_git() -> bool {
    let found = std::process::Command::new("git").arg("--version").output().is_ok();
    if !found {
        eprintln!("pulado: sem git nesta máquina");
    }
    found
}

/// A folder of its own, removed when dropped.
struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn folder(name: &str) -> Folder {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("raven-ci-git-scm-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    Folder(dir)
}

fn url(dir: &Path) -> String {
    let path = dir.display().to_string().replace('\\', "/");
    if path.starts_with('/') { format!("file://{path}") } else { format!("file:///{path}") }
}

fn toml_saying(word: &str) -> String {
    format!("[[stages]]\nname = \"Diz\"\nsteps = ['echo {word}']\n")
}

/// A repository with `main` and `dev`, each with a `.raven-ci.toml` that echoes its own word.
struct Origin {
    dir: Folder,
    git: Git,
}

impl Origin {
    fn new() -> Self {
        let dir = folder("origem");
        let git = Git::at(&dir.0).with_identity("Teste", "teste@localhost");
        let origin = Self { dir, git };
        origin.run(&["init", "--quiet"]);
        origin.run(&["checkout", "--quiet", "-b", "main"]);
        origin.commit(".raven-ci.toml", &toml_saying("pipeline-da-main"));
        origin.run(&["checkout", "--quiet", "-b", "dev"]);
        origin.commit(".raven-ci.toml", &toml_saying("pipeline-da-dev"));
        origin.run(&["checkout", "--quiet", "main"]);
        origin
    }

    fn run(&self, args: &[&str]) {
        self.git.run(args).expect("git");
    }

    fn commit(&self, file: &str, text: &str) -> String {
        std::fs::write(self.dir.0.join(file), text).expect("written");
        self.run(&["add", file]);
        self.run(&["commit", "--quiet", "-m", file]);
        self.git.head().expect("head")
    }

    fn url(&self) -> String {
        url(&self.dir.0)
    }
}

fn ci() -> Ci {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(JenkinsfilePlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(Arc::new(GitPlugin::new()));
    ci.apply_plugins().expect("on");
    ci
}

fn log_of(ci: &Ci, job: &str, number: u64) -> Vec<String> {
    ci.build_log(job, number, 0).expect("log").into_iter().map(|line| line.text.trim().to_owned()).collect()
}

#[test]
fn a_job_of_many_branches_over_plain_git_takes_each_pipeline_from_its_own_branch() {
    if !has_git() {
        return;
    }
    let origin = Origin::new();
    let ci = ci();
    let settings = json!({
        "branches": { "id": "git", "branches": ["*"] },
        "scm": { "id": "git", "url": origin.url() },
    });
    ci.save_job(JobSpec::new("repo", "toml", "").settings(settings)).expect("saved");
    let changes = ci.scan("repo").expect("scanned");
    assert_eq!(changes.len(), 2, "{changes:#?}");
    assert!(ci.job("repo/main").expect("main").pipeline.contains("pipeline-da-main"));
    assert!(ci.job("repo/dev").expect("dev").pipeline.contains("pipeline-da-dev"));
    ci.work_until_idle("e1").expect("runs");
    let main = ci.builds("repo/main").expect("builds");
    assert_eq!(main[0].status, BuildStatus::Succeeded, "{:#?}", log_of(&ci, "repo/main", main[0].number));
    assert!(log_of(&ci, "repo/main", main[0].number).contains(&"pipeline-da-main".to_owned()));

    // A change in `dev` changes the pipeline of `dev` only.
    origin.run(&["checkout", "--quiet", "dev"]);
    origin.commit(".raven-ci.toml", &toml_saying("dev-mudou"));
    origin.run(&["checkout", "--quiet", "main"]);
    let changes = ci.scan("repo").expect("scanned again");
    assert_eq!(changes, vec!["repo/dev: build #2 pedido".to_owned()]);
    assert!(ci.job("repo/dev").expect("dev").pipeline.contains("dev-mudou"));
    assert!(ci.job("repo/main").expect("main").pipeline.contains("pipeline-da-main"));
    ci.work_until_idle("e1").expect("runs");
    assert!(
        log_of(&ci, "repo/dev", 2).contains(&"dev-mudou".to_owned()),
        "{:#?}",
        log_of(&ci, "repo/dev", 2)
    );
}

#[test]
fn a_job_of_one_branch_runs_the_jenkinsfile_of_its_tip_and_says_where_it_came_from() {
    if !has_git() {
        return;
    }
    let origin = Origin::new();
    let jenkinsfile = "pipeline {\n  agent any\n  stages {\n    stage('Diz') {\n      steps {\n        sh 'echo do-jenkinsfile'\n      }\n    }\n  }\n}\n";
    let tip = origin.commit("Jenkinsfile", jenkinsfile);
    let ci = ci();
    let settings = json!({
        "pipeline_from": "scm",
        "scm": { "id": "git", "url": origin.url(), "file": "Jenkinsfile", "depth": 1 },
    });
    ci.save_job(JobSpec::new("loja/main", "toml", "").settings(settings)).expect("saved");
    let n = ci.request_build(BuildRequest::new("loja/main", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let log = log_of(&ci, "loja/main", n);
    assert_eq!(ci.build("loja/main", n).expect("build").status, BuildStatus::Succeeded, "{log:#?}");
    assert!(log.contains(&"do-jenkinsfile".to_owned()), "{log:#?}");
    assert!(
        log.iter()
            .any(|line| line
                .contains(&format!("pipeline: Jenkinsfile do repositório, em main no commit {tip}"))),
        "{log:#?}"
    );

    // A job that says neither keeps its own pipeline; a file the branch does not have is refused.
    let missing = json!({ "pipeline_from": "scm", "scm": { "id": "git", "url": origin.url(), "file": "nao/existe.toml" } });
    ci.save_job(JobSpec::new("loja/main", "toml", "").settings(missing)).expect("saved");
    let refused = ci.request_build(BuildRequest::new("loja/main", "teste")).expect_err("no such file");
    assert!(refused.to_string().contains("não tem nao/existe.toml em main"), "{refused}");
    let wrong = json!({ "pipeline_from": "repositorio", "scm": { "id": "git", "url": origin.url() } });
    ci.save_job(JobSpec::new("loja/main", "toml", "").settings(wrong)).expect("saved");
    let refused = ci.request_build(BuildRequest::new("loja/main", "teste")).expect_err("no such source");
    assert!(refused.to_string().contains("\"repositorio\" não existe"), "{refused}");
}

#[test]
fn a_parameter_picks_the_branch_a_job_of_one_branch_fetches() {
    // The parameterized job of the reference Jenkins (Azure DevOps, `BRANCH` = develop or a
    // feature branch) fetches the branch the person asks for (V-03 of #803).
    if !has_git() {
        return;
    }
    let origin = Origin::new();
    let ci = ci();
    let pipeline = "[[parameters]]\nname = \"BRANCH\"\ndefault = \"main\"\n\n[[stages]]\nname = \"Mostra\"\nsteps = ['cat .raven-ci.toml']\n";
    let settings =
        json!({ "scm": { "id": "git", "url": origin.url(), "reference": { "branch": "${BRANCH}" } } });
    ci.save_job(JobSpec::new("appogi", "toml", pipeline).settings(settings)).expect("saved");

    let of_dev = ci
        .request_build(BuildRequest::new("appogi", "teste").parameter("BRANCH", json!("dev")))
        .expect("asked");
    let of_main = ci.request_build(BuildRequest::new("appogi", "teste")).expect("asked with the default");
    ci.work_until_idle("e1").expect("runs");
    let dev = log_of(&ci, "appogi", of_dev.number);
    assert_eq!(ci.build("appogi", of_dev.number).expect("build").status, BuildStatus::Succeeded, "{dev:#?}");
    assert!(dev.iter().any(|line| line.contains("pipeline-da-dev")), "{dev:#?}");
    let main = log_of(&ci, "appogi", of_main.number);
    assert!(main.iter().any(|line| line.contains("pipeline-da-main")), "{main:#?}");
}

#[test]
fn the_source_git_reads_a_file_with_a_credential_that_shows_nowhere() {
    if !has_git() {
        return;
    }
    let origin = Origin::new();
    let data = folder("dados");
    let plugin = GitPlugin::new();
    let host = FakeHost::new(
        "git",
        &[Permission::Process, Permission::Network, Permission::Secrets, Permission::Disk],
    )
    .with_secret("privado", "usuario:token-muito-secreto")
    .with_dir(&data.0);
    plugin.start(Arc::new(host)).expect("starts");
    let source = plugin.branch_sources()[0].clone();
    let settings = json!({ "url": origin.url(), "credential": "privado", "branches": "main" });
    let heads = source.discover(&settings).expect("found");
    assert_eq!(heads.len(), 1);
    assert_eq!(heads[0].kind, HeadKind::Branch);
    let text = source.file(&settings, &heads[0], ".raven-ci.toml").expect("read").expect("there");
    assert!(text.contains("pipeline-da-main"), "{text}");
    assert_eq!(source.file(&settings, &heads[0], "nada.toml").expect("read"), None);
    // The bare repository it keeps has no token anywhere.
    let mut kept = String::new();
    for entry in walk(&data.0) {
        if let Ok(text) = std::fs::read_to_string(&entry) {
            kept.push_str(&text);
        }
    }
    assert!(!kept.contains("token-muito-secreto"), "the token went to the disk");

    // A head whose branch moved on is read at the commit it was found at.
    let found = heads[0].clone();
    origin.commit(".raven-ci.toml", &toml_saying("depois"));
    let old = source.file(&settings, &found, ".raven-ci.toml").expect("read").expect("there");
    assert!(old.contains("pipeline-da-main"), "{old}");
    let now = Head { commit: String::new(), ..found };
    let new = source.file(&settings, &now, ".raven-ci.toml").expect("read").expect("there");
    assert!(new.contains("depois"), "{new}");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() { files.extend(walk(&path)) } else { files.push(path) }
    }
    files
}
