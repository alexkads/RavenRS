//! The plugin git (#870), on a repository made here: a branch read by a step, a pull request
//! merged onto its target, one that conflicts, a commit, and a credential that never shows. The
//! tests say they skip when there is no `git`.
#![allow(clippy::expect_used, clippy::print_stderr, reason = "tests fail loudly and say what they skip")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::testing::FakeHost;
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CheckoutRequest, CiPlugin, Permission, Reference};
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
    let dir = std::env::temp_dir().join(format!("raven-ci-git-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    Folder(dir)
}

/// The repository of the tests and its commits.
struct Origin {
    dir: Folder,
    main: String,
    feature: String,
}

impl Origin {
    /// `file://` and the path, so a depth asked counts, as it does not for a plain path.
    fn url(&self) -> String {
        let path = self.dir.0.display().to_string().replace('\\', "/");
        if path.starts_with('/') { format!("file://{path}") } else { format!("file:///{path}") }
    }
}

fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).expect("written");
}

/// `main` with `a.txt`; the pull request 7 adds `b.txt` and merges clean; the pull request 8
/// changes `a.txt` as `main` does after it, and conflicts.
fn origin() -> Origin {
    let dir = folder("origem");
    let git = Git::at(&dir.0).with_identity("Teste", "teste@localhost");
    let run = |args: &[&str]| {
        git.run(args).expect("git");
    };
    run(&["init", "--quiet"]);
    run(&["checkout", "--quiet", "-b", "main"]);
    write(&dir.0, "a.txt", "main 1\n");
    run(&["add", "a.txt"]);
    run(&["commit", "--quiet", "-m", "primeiro"]);
    let first = git.head().expect("head");
    run(&["checkout", "--quiet", "-b", "feature"]);
    write(&dir.0, "b.txt", "do PR\n");
    run(&["add", "b.txt"]);
    run(&["commit", "--quiet", "-m", "pr 7"]);
    let feature = git.head().expect("head");
    run(&["update-ref", "refs/pull/7/head", &feature]);
    run(&["checkout", "--quiet", "-b", "briga", &first]);
    write(&dir.0, "a.txt", "do PR 8\n");
    run(&["commit", "--quiet", "-am", "pr 8"]);
    let fight = git.head().expect("head");
    run(&["update-ref", "refs/pull/8/head", &fight]);
    run(&["checkout", "--quiet", "main"]);
    write(&dir.0, "a.txt", "main 2\n");
    write(&dir.0, "c.txt", "depois\n");
    run(&["add", "a.txt", "c.txt"]);
    run(&["commit", "--quiet", "-m", "segundo"]);
    let main = git.head().expect("head");
    Origin { dir, main, feature }
}

fn plugin(secret: Option<(&str, &str)>) -> GitPlugin {
    let plugin = GitPlugin::new();
    let mut host = FakeHost::new("git", &[Permission::Process, Permission::Network, Permission::Secrets]);
    if let Some((name, value)) = secret {
        host = host.with_secret(name, value);
    }
    plugin.start(Arc::new(host)).expect("starts");
    plugin
}

fn request(origin: &Origin, workspace: &Path, reference: Reference) -> CheckoutRequest {
    CheckoutRequest {
        url: origin.url(),
        reference,
        workspace: workspace.join("codigo"),
        depth: None,
        credential: None,
        merge_onto_target: false,
    }
}

#[test]
fn a_branch_is_cloned_into_the_folder_of_the_build_and_a_step_reads_it() {
    if !has_git() {
        return;
    }
    let origin = origin();
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(Arc::new(GitPlugin::new()));
    ci.apply_plugins().expect("on");
    let show = if cfg!(windows) {
        "type a.txt & echo commit %GIT_COMMIT%"
    } else {
        "cat a.txt; echo commit $GIT_COMMIT"
    };
    let pipeline = format!("[[stages]]\nname = \"Lê\"\nsteps = ['{show}']\n");
    let settings = json!({ "scm": { "id": "git", "url": origin.url(), "depth": 1 } });
    ci.save_job(JobSpec::new("repo/main", "toml", pipeline).settings(settings)).expect("saved");
    let n = ci.request_build(BuildRequest::new("repo/main", "push")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let log: Vec<String> =
        ci.build_log("repo/main", n, 0).expect("log").into_iter().map(|l| l.text.trim().to_owned()).collect();
    assert_eq!(ci.build("repo/main", n).expect("build").status, BuildStatus::Succeeded, "{log:#?}");
    assert!(log.contains(&"main 2".to_owned()), "{log:#?}");
    assert!(log.contains(&format!("commit {}", origin.main)), "{log:#?}");
    assert!(log.iter().any(|l| l.contains("na branch main, últimos 1 commits")), "{log:#?}");
}

#[test]
fn a_pull_request_merged_onto_its_target_has_both_sides_and_says_the_target() {
    if !has_git() {
        return;
    }
    let origin = origin();
    let workspace = folder("pr");
    let mut asked =
        request(&origin, &workspace.0, Reference::PullRequest { number: 7, target: "main".into() });
    asked.merge_onto_target = true;
    asked.depth = Some(1);
    let lines = std::sync::Mutex::new(Vec::new());
    let scm = plugin(None).scms()[0].clone();
    let checkout =
        scm.checkout(&asked, &|line| lines.lock().expect("lock").push(line.to_owned())).expect("merged");
    assert_eq!(checkout.merged_onto.as_deref(), Some(origin.main.as_str()));
    assert_ne!(checkout.commit, origin.main);
    assert_ne!(checkout.commit, origin.feature);
    for file in ["a.txt", "b.txt", "c.txt"] {
        assert!(asked.workspace.join(file).exists(), "{file} after the merge");
    }
    let said = lines.lock().expect("lock").join("\n");
    assert!(said.contains("buscando o PR 7") && said.contains("mesclando o PR 7 em main"), "{said}");

    // Without merging, the pull request's own commit.
    let alone = folder("pr-sozinho");
    let asked = request(&origin, &alone.0, Reference::PullRequest { number: 7, target: "main".into() });
    let checkout = scm.checkout(&asked, &|_| {}).expect("checked out");
    assert_eq!(checkout.commit, origin.feature);
    assert!(!asked.workspace.join("c.txt").exists());
}

#[test]
fn a_pull_request_that_conflicts_with_its_target_fails_naming_the_file() {
    if !has_git() {
        return;
    }
    let origin = origin();
    let workspace = folder("briga");
    let mut asked =
        request(&origin, &workspace.0, Reference::PullRequest { number: 8, target: "main".into() });
    asked.merge_onto_target = true;
    let scm = plugin(None).scms()[0].clone();
    let refused = scm.checkout(&asked, &|_| {}).expect_err("conflict");
    let said = refused.to_string();
    assert!(said.contains("o PR 8 conflita com main") && said.contains("a.txt"), "{said}");
}

#[test]
fn a_commit_asked_for_is_the_one_checked_out() {
    if !has_git() {
        return;
    }
    let origin = origin();
    let workspace = folder("commit");
    let asked = request(&origin, &workspace.0, Reference::Commit(origin.feature.clone()));
    let checkout = plugin(None).scms()[0].checkout(&asked, &|_| {}).expect("checked out");
    assert_eq!(checkout.commit, origin.feature);
    assert!(asked.workspace.join("b.txt").exists());
}

#[test]
fn the_token_of_a_credential_never_shows_in_what_the_checkout_says() {
    if !has_git() {
        return;
    }
    let origin = origin();
    let workspace = folder("credencial");
    let mut asked = request(&origin, &workspace.0, Reference::Branch("main".into()));
    asked.credential = Some("github".into());
    let lines = std::sync::Mutex::new(Vec::new());
    let scm = plugin(Some(("github", "usuario:token-muito-secreto"))).scms()[0].clone();
    scm.checkout(&asked, &|line| lines.lock().expect("lock").push(line.to_owned())).expect("cloned");
    let said = lines.lock().expect("lock").join("\n");
    assert!(!said.contains("token-muito-secreto"), "{said}");
    let config = std::fs::read_to_string(asked.workspace.join(".git/config")).expect("the config");
    assert!(!config.contains("token-muito-secreto"), "the token went to .git/config");

    // A credential that is not there fails saying which.
    let mut missing = request(&origin, &folder("sem").0, Reference::Branch("main".into()));
    missing.credential = Some("nenhuma".into());
    let refused = scm.checkout(&missing, &|_| {}).expect_err("no such secret");
    assert!(refused.to_string().contains("nenhuma"), "{refused}");
}
