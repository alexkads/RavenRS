//! The plugin cron (#875): builds on a period, builds when a branch of a real repository moves and
//! not when it does not, a schedule it cannot read refused, and nothing after it stops watching.
#![allow(clippy::expect_used, clippy::print_stderr, reason = "tests fail loudly and say what they skip")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::Point;
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_cron::CronPlugin;
use ravenrs_git::Git;
use serde_json::{Value, json};

fn ci() -> Arc<Ci> {
    let ci = Arc::new(Ci::in_memory().expect("a core"));
    ci.add_plugin(Arc::new(CronPlugin::new()));
    ci.add_plugin(Arc::new(Fake::new(Point::Step)));
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.apply_plugins().expect("on");
    ci.configure_plugin("cron", json!({ "tick_ms": 20 })).expect("a quick tick");
    ci
}

fn job(ci: &Ci, name: &str, trigger: Value) {
    let job = JobSpec::new(name, "fake-lines", "Build: ok")
        .every_request()
        .settings(json!({ "triggers": [trigger] }));
    ci.save_job(job).expect("saved");
}

/// The causes of the builds of `job`, oldest first, after running what is due.
fn causes(ci: &Ci, job: &str) -> Vec<String> {
    ci.work_until_idle("e1").expect("runs");
    let mut builds = ci.builds(job).expect("builds");
    builds.reverse();
    builds.into_iter().map(|build| build.cause).collect()
}

/// Waits until `job` has at least `count` builds, up to five seconds.
fn wait_for(ci: &Ci, job: &str, count: usize) -> Vec<String> {
    let began = Instant::now();
    loop {
        let seen = causes(ci, job);
        if seen.len() >= count || began.elapsed() > Duration::from_secs(5) {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_period_asks_for_builds_by_itself_with_its_cause() {
    let ci = ci();
    job(&ci, "noturno", json!({ "id": "cron", "every": "200ms" }));
    assert!(ci.watch_triggers().is_empty());
    let seen = wait_for(&ci, "noturno", 2);
    assert!(seen.len() >= 2, "{seen:?}");
    assert!(seen.iter().all(|cause| cause.starts_with("agenda a cada")), "{seen:?}");
    ci.unwatch_triggers();
    let after = causes(&ci, "noturno").len();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(causes(&ci, "noturno").len(), after, "nothing after it stopped watching");
}

#[test]
fn settings_it_cannot_read_are_refused_with_why() {
    let ci = ci();
    job(&ci, "a", json!({ "id": "cron", "schedule": "sessenta" }));
    job(&ci, "b", json!({ "id": "cron" }));
    job(&ci, "c", json!({ "id": "poll", "every": "5m" }));
    job(&ci, "d", json!({ "id": "cron", "every": "10" }));
    let problems = ci.watch_triggers();
    assert_eq!(problems.len(), 4, "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("sessenta")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("url")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("no unit")), "{problems:?}");
}

/// A folder of its own, removed when dropped.
struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_poll_asks_for_a_build_when_the_branch_moves_and_not_when_it_stays() {
    if std::process::Command::new("git").arg("--version").output().is_err() {
        eprintln!("pulado: sem git nesta máquina");
        return;
    }
    let repo = Folder(std::env::temp_dir().join(format!("raven-ci-cron-repo-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&repo.0);
    std::fs::create_dir_all(&repo.0).expect("a folder");
    let git = Git::at(&repo.0).with_identity("Teste", "teste@localhost");
    let commit = |text: &str| {
        std::fs::write(repo.0.join("a.txt"), text).expect("written");
        git.run(&["add", "a.txt"]).expect("add");
        git.run(&["commit", "--quiet", "-m", text]).expect("commit");
        git.head().expect("head")
    };
    git.run(&["init", "--quiet"]).expect("init");
    git.run(&["checkout", "--quiet", "-b", "main"]).expect("branch");
    let first = commit("um");
    let ci = ci();
    let url = repo.0.display().to_string();
    job(&ci, "repo/main", json!({ "id": "poll", "url": url, "branch": "main", "every": "100ms" }));
    assert!(ci.watch_triggers().is_empty());
    let seen = wait_for(&ci, "repo/main", 1);
    assert_eq!(seen, [format!("mudança em main: {}", &first[..7])], "the first look builds");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(causes(&ci, "repo/main").len(), 1, "no build while the branch stays");
    let second = commit("dois");
    let seen = wait_for(&ci, "repo/main", 2);
    assert_eq!(seen.last(), Some(&format!("mudança em main: {}", &second[..7])), "{seen:?}");
    ci.unwatch_triggers();
}
