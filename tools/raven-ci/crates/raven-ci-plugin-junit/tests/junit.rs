//! The plugin junit (#880): two builds with reports of tests, one file broken, the count kept
//! with each build and the trend of the job over its route.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::path::PathBuf;
use std::sync::Arc;

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CiPlugin, HttpRequest};
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_junit::JunitPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use serde_json::{Value, json};

const FIRST: &str = r#"<testsuite name="loja" tests="3"><testcase classname="loja" name="soma" time="0.010"/><testcase classname="loja" name="cupom"><failure message="esperava 90">veio 100</failure></testcase><testcase classname="loja" name="lento"><skipped/></testcase></testsuite>"#;
const SECOND: &str = r#"<testsuites><testsuite name="api"><testcase classname="api" name="health"/><testcase classname="api" name="banco"><error message="sem conexão"/></testcase></testsuite></testsuites>"#;

/// A folder with the reports, which the steps copy into the folder of the build.
struct Reports(PathBuf);

impl Drop for Reports {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn reports() -> Reports {
    let dir = std::env::temp_dir().join(format!("raven-ci-junit-relatorios-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    std::fs::write(dir.join("a.xml"), FIRST).expect("written");
    std::fs::write(dir.join("b.xml"), SECOND).expect("written");
    std::fs::write(dir.join("quebrado.xml"), "<testsuite><testcase name=\"x\">").expect("written");
    Reports(dir)
}

#[test]
fn the_reports_of_each_build_are_counted_and_the_job_has_its_trend() {
    let source = reports();
    let plugin = Arc::new(JunitPlugin::new());
    let ci = Arc::new(Ci::in_memory().expect("a core"));
    ci.share();
    for one in [plugin.clone() as Arc<dyn CiPlugin>, Arc::new(ShellPlugin::new()), Arc::new(TomlFormatPlugin)]
    {
        ci.add_plugin(one);
    }
    ci.apply_plugins().expect("on");
    // The steps copy the reports in, with sh or with cmd; TOML's literal strings keep the quotes.
    let copy = if cfg!(windows) { "copy" } else { "cp" };
    let step = |file: &str| format!("'{copy} \"{}\" .'", source.0.join(file).display());
    let pipeline = format!(
        "junit = [\"*.xml\"]\n\n[[stages]]\nname = \"Testes\"\nsteps = [{}, {}, {}]\n",
        step("a.xml"),
        step("b.xml"),
        step("quebrado.xml")
    );
    ci.save_job(JobSpec::new("loja", "toml", pipeline)).expect("the job");
    for _ in 0..2 {
        ci.request_build(BuildRequest::new("loja", "teste")).expect("asked");
        ci.work_until_idle("e1").expect("runs");
    }
    let log =
        ci.build_log("loja", 1, 0).expect("log").into_iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
    assert_eq!(ci.build("loja", 1).expect("the build").status, BuildStatus::Succeeded, "{log}");
    assert!(log.contains("quebrado.xml: JUnit ilegível"), "{log}");
    assert!(log.contains("5 testes: 2 passaram, 1 falharam, 1 com erro, 1 pulados"), "{log}");
    let kept = ci.attachments("loja", 1).expect("attachments");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].kind, "junit");
    assert_eq!(
        kept[0].summary,
        json!({ "tests": 5, "failures": 1, "errors": 1, "skipped": 1, "time_ms": 10, "files": 2,
                "failed": ["loja.cupom", "api.banco"] })
    );
    let suites: Value =
        serde_json::from_slice(&ci.attachment_bytes(kept[0].id).expect("bytes")).expect("json");
    assert_eq!(suites["suites"].as_array().map(Vec::len), Some(2));

    let route = plugin.routes().into_iter().find(|r| r.path == "/api/ci/tests").expect("the route");
    let ask = |query: &[(&str, &str)]| {
        let request = HttpRequest {
            method: "GET".to_owned(),
            path: "/api/ci/tests".to_owned(),
            query: query.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
            actor: Some("ana".to_owned()),
            ..HttpRequest::default()
        };
        let answer = (route.handler)(&request);
        (answer.status, serde_json::from_slice::<Value>(&answer.body).unwrap_or(Value::Null))
    };
    let (status, trend) = ask(&[("job", "loja")]);
    assert_eq!(status, 200);
    let numbers: Vec<u64> =
        trend.as_array().expect("a list").iter().filter_map(|i| i["number"].as_u64()).collect();
    assert_eq!(numbers, [2, 1], "the newest first");
    assert_eq!(trend[0]["failures"], 1);
    assert_eq!(ask(&[("job", "loja"), ("limit", "1")]).1.as_array().map(Vec::len), Some(1));
    assert_eq!(ask(&[]).0, 400);
}

#[test]
fn a_build_with_no_report_says_so() {
    let ci = Arc::new(Ci::in_memory().expect("a core"));
    ci.share();
    for one in [
        Arc::new(JunitPlugin::new()) as Arc<dyn CiPlugin>,
        Arc::new(ShellPlugin::new()),
        Arc::new(TomlFormatPlugin),
    ] {
        ci.add_plugin(one);
    }
    ci.apply_plugins().expect("on");
    let pipeline = "junit = [\"**/junit.xml\"]\n\n[[stages]]\nname = \"Nada\"\nsteps = [\"echo nada\"]\n";
    ci.save_job(JobSpec::new("vazio", "toml", pipeline)).expect("the job");
    ci.request_build(BuildRequest::new("vazio", "teste")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    let log =
        ci.build_log("vazio", 1, 0).expect("log").into_iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
    assert!(log.contains("nenhum relatório achado em **/junit.xml"), "{log}");
}
