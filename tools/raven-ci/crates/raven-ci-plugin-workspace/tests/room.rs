//! The plugin workspace-cleanup (#872): a build refused with no room on the disk, before its
//! code, and one that runs with the check off.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::Arc;

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus};
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use raven_ci_plugin_workspace::WorkspacePlugin;
use serde_json::{Value, json};

const PIPELINE: &str = "[[stages]]\nname = \"T\"\nsteps = [\"echo rodou\"]\n";

fn ci() -> Ci {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(Arc::new(WorkspacePlugin::new()));
    ci.apply_plugins().expect("on");
    ci
}

fn build(ci: &Ci, job: &str, settings: Value) -> (BuildStatus, Option<String>, Vec<String>) {
    ci.save_job(JobSpec::new(job, "toml", PIPELINE).settings(settings)).expect("saved");
    let n = ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let built = ci.build(job, n).expect("build");
    let log = ci.build_log(job, n, 0).expect("log").into_iter().map(|l| l.text.trim().to_owned()).collect();
    (built.status, built.reason, log)
}

#[test]
fn a_build_that_asks_more_room_than_the_disk_has_is_refused_before_any_step() {
    let ci = ci();
    let (status, reason, log) =
        build(&ci, "grande", json!({ "workspace": { "min_free_gb": 1_000_000_000 } }));
    assert_eq!(status, BuildStatus::Failed);
    let reason = reason.unwrap_or_default();
    assert!(reason.contains("GB livres, menos que os 1000000000 GB pedidos"), "{reason}");
    assert!(!log.contains(&"rodou".to_owned()), "no step ran: {log:?}");
}

#[test]
fn with_the_minimum_at_zero_or_within_the_disk_the_build_runs() {
    let ci = ci();
    let (off, _, log) = build(&ci, "sem-conferir", json!({ "workspace": { "min_free_gb": 0 } }));
    assert_eq!(off, BuildStatus::Succeeded, "{log:?}");
    let (small, reason, log) = build(&ci, "pequeno", json!({ "workspace": { "min_free_gb": 0.001 } }));
    assert_eq!(small, BuildStatus::Succeeded, "{reason:?} {log:?}");
    assert!(log.contains(&"rodou".to_owned()));
}

#[test]
fn the_minimum_of_the_plugin_counts_when_the_job_says_none() {
    let ci = ci();
    ci.configure_plugin("workspace-cleanup", json!({ "min_free_gb": 1_000_000_000 })).expect("configured");
    let (status, reason, _) = build(&ci, "sem-dizer", json!({}));
    assert_eq!(status, BuildStatus::Failed, "{reason:?}");
    let refused = ci.configure_plugin("workspace-cleanup", json!({ "min_free_gb": -1 }));
    assert!(refused.is_err(), "a minimum below zero is refused");
}

#[test]
fn a_kept_folder_says_its_size_in_the_log_of_the_server() {
    let ci = ci();
    let (status, _, _) = build(&ci, "fica", json!({ "workspace": { "keep": true, "min_free_gb": 0 } }));
    assert_eq!(status, BuildStatus::Succeeded);
    assert!(
        ci.log().lines().iter().any(|l| l.contains("fica #1 (passou) deixou a pasta")),
        "{:?}",
        ci.log().lines()
    );
}
