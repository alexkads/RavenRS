//! The plugin `metrics` (#913): the text Prometheus reads, from a picture of the server and from
//! a core with builds kept and one in the queue, and its two routes.
#![allow(clippy::expect_used, clippy::panic, reason = "tests fail loudly")]

use std::sync::Arc;

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::testing::FakeHost;
use raven_ci_plugin_api::{
    AgentSnapshot, BuildRequest, BuildStatus, BuildTotal, CiPlugin, CiSnapshot, HttpRequest, JobSnapshot,
    PluginSnapshot,
};
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_metrics::{MetricsPlugin, OPEN_PATH, PATH, render};
use raven_ci_plugin_shell::ShellPlugin;
use serde_json::json;

/// The value of the series `line` starts with, as Prometheus reads it.
fn value(text: &str, series: &str) -> Option<f64> {
    text.lines()
        .find(|l| l.starts_with(series) && l[series.len()..].starts_with(' '))
        .and_then(|l| l.rsplit(' ').next().and_then(|v| v.parse().ok()))
}

#[test]
fn a_picture_becomes_the_series_prometheus_reads() {
    let snapshot = CiSnapshot {
        queued: 2,
        running: 1,
        builds: vec![(BuildStatus::Succeeded, 5), (BuildStatus::Failed, 1)],
        jobs: vec![JobSnapshot {
            job: "apps/site".into(),
            number: 6,
            status: BuildStatus::Failed,
            duration_ms: 1500,
        }],
        agents: vec![
            AgentSnapshot { name: "linux-1".into(), connected: true, executors: 2 },
            AgentSnapshot { name: "mac".into(), connected: false, executors: 1 },
        ],
        plugins: vec![
            PluginSnapshot {
                name: "shell".into(),
                origin: "builtin".into(),
                state: "on".into(),
                memory_bytes: None,
            },
            PluginSnapshot {
                name: "hello".into(),
                origin: "external".into(),
                state: "on".into(),
                memory_bytes: Some(7_340_032),
            },
        ],
        server_memory_bytes: Some(41_943_040),
        totals: vec![BuildTotal {
            job: "apps/site".into(),
            status: BuildStatus::Succeeded,
            builds: 9,
            seconds: 12.5,
        }],
        totals_since_ms: Some(1_700_000_000_000),
    };
    let text = render(&snapshot).expect("rendered");
    assert_eq!(value(&text, "raven_ci_queue_builds"), Some(2.0), "{text}");
    assert_eq!(value(&text, "raven_ci_running_builds"), Some(1.0));
    assert_eq!(value(&text, r#"raven_ci_builds{status="succeeded"}"#), Some(5.0));
    assert_eq!(value(&text, r#"raven_ci_builds{status="failed"}"#), Some(1.0));
    assert_eq!(value(&text, r#"raven_ci_last_build_duration_seconds{job="apps/site"}"#), Some(1.5));
    assert_eq!(value(&text, r#"raven_ci_last_build_status{job="apps/site",status="failed"}"#), Some(1.0));
    assert_eq!(value(&text, r#"raven_ci_agents{state="connected"}"#), Some(1.0));
    assert_eq!(value(&text, r#"raven_ci_agents{state="disconnected"}"#), Some(1.0));
    assert_eq!(value(&text, r#"raven_ci_agent_executors{agent="linux-1"}"#), Some(2.0));
    assert_eq!(value(&text, r#"raven_ci_plugins{state="on"}"#), Some(2.0));
    assert_eq!(
        value(&text, r#"raven_ci_plugin_memory_bytes{plugin="hello",origin="external"}"#),
        Some(7_340_032.0)
    );
    assert!(!text.contains(r#"plugin="shell""#), "a plugin compiled in shares the server's memory");
    assert_eq!(value(&text, "raven_ci_server_memory_bytes"), Some(41_943_040.0));
    assert!(text.contains("# TYPE raven_ci_builds gauge"), "{text}");
    assert_eq!(value(&text, r#"raven_ci_builds_total{job="apps/site",status="succeeded"}"#), Some(9.0));
    assert_eq!(
        value(&text, r#"raven_ci_build_seconds_total{job="apps/site",status="succeeded"}"#),
        Some(12.5)
    );
    assert_eq!(value(&text, "raven_ci_totals_since_seconds"), Some(1_700_000_000.0));
    assert!(text.contains("# TYPE raven_ci_builds_total counter"), "{text}");
    assert!(text.contains("# TYPE raven_ci_build_seconds_total counter"), "{text}");
}

/// The plugin `metrics` in a core on a file, with the job's shell and format.
fn core_on(dir: &std::path::Path) -> (Arc<Ci>, Arc<MetricsPlugin>) {
    let ci = Arc::new(Ci::open(dir).expect("a core"));
    ci.share();
    let plugin = Arc::new(MetricsPlugin::new());
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(plugin.clone());
    ci.apply_plugins().expect("on");
    (ci, plugin)
}

#[test]
fn the_totals_count_every_build_that_ended_though_the_retention_keeps_two() {
    let dir = std::env::temp_dir().join(format!("raven-ci-totais-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    {
        let (ci, plugin) = core_on(&dir);
        let text = "[[stages]]\nname = \"T\"\nsteps = ['echo ok']\n";
        ci.save_job(JobSpec::new("passa", "toml", text).every_request().keep_builds(2)).expect("saved");
        for _ in 0..5 {
            ci.request_build(BuildRequest::new("passa", "teste")).expect("asked");
            ci.work_until_idle("e1").expect("runs");
        }
        ci.prune().expect("pruned");
        let (status, text) = get(&plugin, PATH);
        assert_eq!(status, 200, "{text}");
        // The gauge says the builds kept; the counter, every build that ended.
        assert_eq!(value(&text, r#"raven_ci_builds{status="succeeded"}"#), Some(2.0), "{text}");
        assert_eq!(
            value(&text, r#"raven_ci_builds_total{job="passa",status="succeeded"}"#),
            Some(5.0),
            "{text}"
        );
        let seconds = value(&text, r#"raven_ci_build_seconds_total{job="passa",status="succeeded"}"#);
        assert!(seconds.is_some_and(|s| s >= 0.0), "{text}");
        assert!(value(&text, "raven_ci_totals_since_seconds").is_some_and(|s| s > 0.0), "{text}");
    }
    // A core opened again on the same file keeps counting from where it was.
    {
        let (ci, plugin) = core_on(&dir);
        let (_, text) = get(&plugin, PATH);
        assert_eq!(
            value(&text, r#"raven_ci_builds_total{job="passa",status="succeeded"}"#),
            Some(5.0),
            "{text}"
        );
        ci.request_build(BuildRequest::new("passa", "teste")).expect("asked");
        ci.work_until_idle("e1").expect("runs");
        let (_, text) = get(&plugin, PATH);
        assert_eq!(
            value(&text, r#"raven_ci_builds_total{job="passa",status="succeeded"}"#),
            Some(6.0),
            "{text}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_old_picture_with_no_totals_still_reads() {
    let old = json!({
        "queued": 0, "running": 0, "builds": [], "jobs": [], "agents": [], "plugins": [],
        "server_memory_bytes": null
    });
    let snapshot: CiSnapshot = serde_json::from_value(old).expect("an old picture reads");
    assert!(snapshot.totals.is_empty() && snapshot.totals_since_ms.is_none());
    let text = render(&snapshot).expect("rendered");
    assert!(!text.contains("raven_ci_builds_total{"), "{text}");
}

fn get(plugin: &MetricsPlugin, path: &str) -> (u16, String) {
    let route = plugin.routes().into_iter().find(|r| r.matches("GET", path)).expect("a route");
    let answer =
        (route.handler)(&HttpRequest { method: "GET".into(), path: path.into(), ..HttpRequest::default() });
    (answer.status, String::from_utf8_lossy(&answer.body).into_owned())
}

#[test]
fn a_core_with_builds_kept_and_one_waiting_says_them() {
    let ci = Arc::new(Ci::in_memory().expect("a core"));
    ci.share();
    let plugin = Arc::new(MetricsPlugin::new());
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(plugin.clone());
    ci.apply_plugins().expect("on");
    let fail = if cfg!(windows) { "exit /b 1" } else { "exit 1" };
    for (job, script) in [("passa", "echo ok"), ("falha", fail)] {
        let text = format!("[[stages]]\nname = \"T\"\nsteps = ['{script}']\n");
        ci.save_job(JobSpec::new(job, "toml", text).every_request()).expect("saved");
        ci.request_build(BuildRequest::new(job, "teste")).expect("asked");
    }
    ci.work_until_idle("e1").expect("runs");
    ci.request_build(BuildRequest::new("passa", "teste")).expect("asked");

    let (status, text) = get(&plugin, PATH);
    assert_eq!(status, 200, "{text}");
    assert_eq!(value(&text, r#"raven_ci_builds{status="succeeded"}"#), Some(1.0), "{text}");
    assert_eq!(value(&text, r#"raven_ci_builds{status="failed"}"#), Some(1.0), "{text}");
    assert_eq!(value(&text, "raven_ci_queue_builds"), Some(1.0), "{text}");
    assert_eq!(value(&text, r#"raven_ci_plugins{state="on"}"#), Some(3.0), "{text}");

    // The path with no sign-in stays closed until the settings open it.
    assert_eq!(get(&plugin, OPEN_PATH).0, 404);
    ci.configure_plugin("metrics", json!({ "open": true })).expect("configured");
    assert_eq!(get(&plugin, OPEN_PATH).0, 200);
}

#[test]
fn a_host_with_no_picture_answers_503_saying_why() {
    let plugin = MetricsPlugin::new();
    plugin.start(Arc::new(FakeHost::new("metrics", &[]))).expect("started");
    let (status, text) = get(&plugin, PATH);
    assert_eq!(status, 503);
    assert!(text.contains("retrato"), "{text}");
    let off = MetricsPlugin::new();
    assert_eq!(get(&off, PATH).0, 503);
}
