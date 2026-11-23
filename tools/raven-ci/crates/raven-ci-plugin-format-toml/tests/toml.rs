//! The format `.raven-ci.toml` (#862): what it reads, every error with its line, and pipelines
//! run for real in the core with the step `sh`. Scripts are said for `sh` and for `cmd`, so the
//! tests run on Linux, macOS and Windows.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::Arc;

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, PLUGIN_KEY, PipelineFormat, STAGE_KEY};
use raven_ci_plugin_format_toml::{TomlFormat, TomlFormatPlugin};
use raven_ci_plugin_shell::ShellPlugin;
use ravenrs_app::FieldKind;
use serde_json::{Value, json};

/// `unix` on Linux and macOS, `windows` on Windows.
fn os(unix: &str, windows: &str) -> String {
    if cfg!(windows) { windows.to_owned() } else { unix.to_owned() }
}

fn ci() -> Ci {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.apply_plugins().expect("on");
    ci
}

fn texts(ci: &Ci, job: &str, number: u64) -> Vec<String> {
    ci.build_log(job, number, 0).expect("log").into_iter().map(|l| l.text.trim().to_owned()).collect()
}

/// Runs a build of `pipeline` as the job `job`, with `parameters`; its status and log.
fn run(ci: &Ci, job: &str, pipeline: &str, parameters: Value) -> (BuildStatus, Vec<String>) {
    ci.save_job(JobSpec::new(job, "toml", pipeline).every_request()).expect("saved");
    let mut request = BuildRequest::new(job, "teste");
    for (name, value) in parameters.as_object().into_iter().flatten() {
        request = request.parameter(name, value.clone());
    }
    let number = ci.request_build(request).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    (ci.build(job, number).expect("build").status, texts(ci, job, number))
}

#[test]
fn two_stages_become_their_steps_in_order_each_with_its_stage_and_plugin() {
    let text = r#"
[[stages]]
name = "Build"
steps = ["cargo build", { sh = "cargo doc", name = "Documentação" }]

[[stages]]
name = "Teste"
steps = ["cargo test"]
"#;
    let pipeline = TomlFormat.read("site", text).expect("reads");
    let steps: Vec<(&str, &str, &str)> = pipeline
        .flow
        .nodes
        .iter()
        .filter(|n| n.kind == "sh")
        .map(|n| {
            (
                n.parameters["script"].as_str().unwrap_or_default(),
                n.parameters[STAGE_KEY].as_str().unwrap_or_default(),
                n.parameters[PLUGIN_KEY].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        steps,
        [
            ("cargo build", "Build", "shell"),
            ("cargo doc", "Build", "shell"),
            ("cargo test", "Teste", "shell")
        ]
    );
    let doc = pipeline.flow.nodes.iter().find(|n| n.id == "e1-2").expect("the second step");
    assert_eq!(doc.title.as_deref(), Some("Documentação"));
    assert_eq!(pipeline.start, "start");
}

#[test]
fn a_file_with_three_errors_says_the_three_each_on_its_line() {
    let text = r#"[[stages]]
name = "Build"
steps = ["true"]
cor = "azul"
timeout = "dez minutos"

[[stages]]
name = "Vazio"
"#;
    let errors = TomlFormat.read("site", text).expect_err("wrong");
    let said: Vec<String> = errors.iter().map(ToString::to_string).collect();
    assert_eq!(errors.len(), 3, "{said:#?}");
    assert!(said.iter().any(|e| e.starts_with("linha 4:") && e.contains("cor")), "{said:#?}");
    assert!(said.iter().any(|e| e.starts_with("linha 5:") && e.contains("dez minutos")), "{said:#?}");
    assert!(said.iter().any(|e| e.contains("Vazio") && e.contains("não tem steps")), "{said:#?}");
}

#[test]
fn a_file_that_is_not_toml_says_where_it_broke() {
    let errors = TomlFormat.read("site", "[[stages]]\nname = \"a\nsteps = []\n").expect_err("wrong");
    assert!(errors[0].line.is_some(), "{errors:?}");
    assert!(errors[0].message.contains("o TOML não fecha"), "{errors:?}");
}

#[test]
fn an_invalid_file_is_refused_when_the_build_is_asked_for_and_nothing_goes_to_the_queue() {
    let ci = ci();
    ci.save_job(JobSpec::new("site", "toml", "[[stages]]\nnome = \"x\"\n")).expect("saved");
    let refused = ci.request_build(BuildRequest::new("site", "teste")).expect_err("refused");
    assert!(refused.to_string().contains("linha 2"), "{refused}");
    assert!(ci.queue().expect("queue").is_empty());
}

#[test]
fn when_branch_switches_the_stage_off_in_a_pull_request_and_keeps_it_on_main() {
    let text = r#"
[[stages]]
name = "Publicar"
when = { branch = "main" }
steps = ["./publica.sh"]

[[stages]]
name = "Só no PR"
when = { pull_request = true }
steps = ["./comenta.sh"]
"#;
    let disabled = |job: &str| -> Vec<(String, bool)> {
        let pipeline = TomlFormat.read(job, text).expect("reads");
        pipeline.flow.nodes.iter().filter(|n| n.kind == "sh").map(|n| (n.id.clone(), n.disabled)).collect()
    };
    assert_eq!(disabled("repo/main"), [("e1-1".to_owned(), false), ("e2-1".to_owned(), true)]);
    assert_eq!(disabled("repo/PR-3"), [("e1-1".to_owned(), true), ("e2-1".to_owned(), false)]);
}

#[test]
fn junit_with_its_plugin_switched_off_is_refused_naming_the_plugin() {
    let ci = ci();
    let text = "junit = [\"**/junit.xml\"]\n[[stages]]\nname = \"T\"\nsteps = [\"true\"]\n";
    ci.save_job(JobSpec::new("site", "toml", text)).expect("saved");
    let refused = ci.request_build(BuildRequest::new("site", "teste")).expect_err("refused");
    assert!(refused.to_string().contains("falta o plugin junit"), "{refused}");
}

#[test]
fn a_step_of_a_plugin_switched_off_is_refused_naming_the_plugin() {
    let ci = ci();
    let text = r#"
[[stages]]
name = "Deploy"
container = { image = "rust:1" }
steps = ["cargo --version", { uses = "kubernetes/kubectl", with = { args = "apply -f k8s" } }]
"#;
    ci.save_job(JobSpec::new("site", "toml", text)).expect("saved");
    let refused = ci.request_build(BuildRequest::new("site", "teste")).expect_err("refused");
    assert!(refused.to_string().contains("falta o plugin docker"), "{refused}");
    let pipeline = TomlFormat.read("site", text).expect("reads");
    let kubectl = pipeline.flow.nodes.iter().find(|n| n.kind == "kubectl").expect("kubectl");
    assert_eq!(kubectl.parameters["args"], "apply -f k8s");
    assert_eq!(kubectl.parameters[PLUGIN_KEY], "kubernetes");
}

#[test]
fn parameters_become_the_fields_of_the_build_and_a_value_out_of_the_choices_is_refused() {
    let text = r#"
[[parameters]]
name = "E2E"
type = "bool"
default = false

[[parameters]]
name = "alvo"
type = "choice"
choices = ["linux", "web"]

[[stages]]
name = "T"
steps = ["true"]
"#;
    let pipeline = TomlFormat.read("site", text).expect("reads");
    assert_eq!(pipeline.parameters.len(), 2);
    assert_eq!(pipeline.parameters[0].kind, FieldKind::Boolean);
    assert!(matches!(&pipeline.parameters[1].kind, FieldKind::Enum(options) if options.len() == 2));
    let ci = ci();
    ci.save_job(JobSpec::new("site", "toml", text)).expect("saved");
    let refused = ci
        .request_build(BuildRequest::new("site", "teste").parameter("alvo", json!("windows")))
        .expect_err("refused");
    assert!(refused.to_string().contains("alvo"), "{refused}");
    ci.request_build(BuildRequest::new("site", "teste").parameter("alvo", json!("web"))).expect("taken");
}

#[test]
fn the_options_of_the_pipeline_come_out_as_its_limit_its_keep_its_environment_and_its_publishers() {
    let text = r#"
artifacts = ["target/*.tar.gz"]

[pipeline]
timeout = "30m"
keep = 10
agent = { docker = { image = "rust:1" } }

[environment]
CARGO_TERM_COLOR = "always"

[[stages]]
name = "T"
steps = ["true"]

[post]
always = ["./limpa.sh"]
"#;
    let pipeline = TomlFormat.read("site", text).expect("reads");
    assert_eq!(pipeline.timeout_ms, Some(30 * 60 * 1000));
    assert_eq!(pipeline.keep_builds, Some(10));
    assert_eq!(pipeline.environment["CARGO_TERM_COLOR"], "always");
    assert_eq!(pipeline.publishers[0].id, "artifacts");
    assert_eq!(pipeline.publishers[0].settings, json!({ "paths": ["target/*.tar.gz"] }));
    assert_eq!(pipeline.agent.as_ref().map(|a| a.id.as_str()), Some("docker"));
    assert_eq!(pipeline.post.len(), 1);
    let needs: Vec<String> = pipeline.needs().into_iter().map(|(plugin, _)| plugin).collect();
    assert!(needs.contains(&"artifacts".to_owned()) && needs.contains(&"docker".to_owned()), "{needs:?}");
}

#[test]
fn a_pipeline_runs_for_real_with_its_stages_post_and_variables() {
    let ci = ci();
    let text = format!(
        r#"
[environment]
SAUDA = "olá"

[[stages]]
name = "Build"
steps = ["{one}"]

[[stages]]
name = "Quebra"
steps = ["{fails}"]

[post]
always = ["echo sempre"]
failure = ["echo falhou"]
success = ["echo passou"]
"#,
        one = os("echo $SAUDA build $BUILD_NUMBER", "echo %SAUDA% build %BUILD_NUMBER%"),
        fails = os("exit 4", "exit /b 4"),
    );
    let (status, log) = run(&ci, "site", &text, json!({}));
    assert_eq!(status, BuildStatus::Failed, "{log:#?}");
    assert!(log.contains(&"olá build 1".to_owned()), "{log:#?}");
    assert!(log.contains(&"sempre".to_owned()) && log.contains(&"falhou".to_owned()), "{log:#?}");
    assert!(!log.contains(&"passou".to_owned()), "{log:#?}");
    let stages = ci.stages("site", 1).expect("stages");
    let names: Vec<(&str, &str)> = stages.iter().map(|s| (s.name.as_str(), s.status.as_str())).collect();
    assert_eq!(names, [("Build", "done"), ("Quebra", "failed")]);
}

#[test]
fn when_parameter_runs_the_stage_only_when_the_build_asks() {
    let ci = ci();
    let text = r#"
[[parameters]]
name = "E2E"
type = "bool"
default = false

[[stages]]
name = "Ponta a ponta"
when = { parameter = "E2E" }
steps = ["echo e2e"]

[[stages]]
name = "Fim"
steps = ["echo fim"]
"#;
    let (yes, with) = run(&ci, "com", text, json!({ "E2E": true }));
    let (no, without) = run(&ci, "sem", text, json!({ "E2E": false }));
    assert_eq!((yes, no), (BuildStatus::Succeeded, BuildStatus::Succeeded), "{with:#?} {without:#?}");
    assert!(with.contains(&"e2e".to_owned()) && with.contains(&"fim".to_owned()), "{with:#?}");
    assert!(!without.contains(&"e2e".to_owned()) && without.contains(&"fim".to_owned()), "{without:#?}");
}

#[test]
fn parallel_branches_all_run_and_meet_before_the_next_stage() {
    let ci = ci();
    let text = r#"
[[stages]]
name = "Plataformas"
parallel = [
  { name = "linux", steps = ["echo linux"] },
  { name = "web", steps = ["echo web"] },
  { name = "mac", steps = ["echo mac"] },
]

[[stages]]
name = "Depois"
steps = ["echo depois"]
"#;
    let (status, log) = run(&ci, "site", text, json!({}));
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    for word in ["linux", "web", "mac"] {
        assert!(log.contains(&word.to_owned()), "{word} in {log:#?}");
    }
    let after = log.iter().position(|l| l == "depois").expect("after");
    assert!(log.iter().position(|l| l == "mac").expect("mac") < after, "{log:#?}");
    assert_eq!(log.iter().filter(|l| *l == "depois").count(), 1, "the next stage runs once: {log:#?}");
}

#[test]
fn retry_tries_a_step_again_until_it_passes() {
    let ci = ci();
    // Fails the first time it runs in the folder, passes the second.
    let script = os(
        "if [ -f marca ]; then echo segunda; else touch marca; echo primeira; exit 1; fi",
        "if exist marca (echo segunda) else (echo x> marca & echo primeira & exit /b 1)",
    );
    let text = format!("[[stages]]\nname = \"T\"\nretry = 2\nsteps = ['{script}']\n");
    let (status, log) = run(&ci, "site", &text, json!({}));
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    assert!(log.contains(&"primeira".to_owned()) && log.contains(&"segunda".to_owned()), "{log:#?}");
}

/// The `keep` of the pipeline keeps that many builds of the job, unless the job says its own
/// (#803, test of the real world: a job with `keep = 3` had kept every build).
#[test]
fn the_keep_of_the_pipeline_keeps_that_many_builds_and_the_job_s_own_number_wins() {
    let ci = ci();
    let pipeline = "[pipeline]\nkeep = 2\n\n[[stages]]\nname = \"a\"\nsteps = [\"echo oi\"]\n";
    for _ in 0..3 {
        assert_eq!(run(&ci, "guarda", pipeline, json!({})).0, BuildStatus::Succeeded);
    }
    let kept: Vec<u64> = ci.builds("guarda").expect("builds").iter().map(|b| b.number).collect();
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert!(!kept.contains(&1), "the oldest goes: {kept:?}");
    ci.save_job(JobSpec::new("seu", "toml", pipeline).every_request().keep_builds(3)).expect("saved");
    for _ in 0..4 {
        let number = ci.request_build(BuildRequest::new("seu", "teste")).expect("asked").number;
        ci.work_until_idle("e1").expect("runs");
        assert_eq!(ci.build("seu", number).expect("build").status, BuildStatus::Succeeded);
    }
    assert_eq!(ci.builds("seu").expect("builds").len(), 3, "the job's own number wins");
}

/// What a command leaves in the background goes with the step, and the log names it; a step that
/// says keep_daemons keeps it (#931).
#[cfg(unix)]
#[test]
fn what_a_command_leaves_running_goes_with_its_step_unless_it_keeps_it() {
    let marker = format!("raven-ci-daemon-{}", std::process::id());
    let alive = || {
        std::process::Command::new("pgrep")
            .args(["-f", &marker])
            .output()
            .is_ok_and(|out| !out.stdout.is_empty())
    };
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.apply_plugins().expect("on");
    let pipeline =
        format!("[[stages]]\nname = \"Daemon\"\nsteps = ['sh -c \"sleep 600; : {marker}\" & echo deixei']\n");
    ci.save_job(JobSpec::new("daemon", "toml", pipeline)).expect("saved");
    let started = std::time::Instant::now();
    let n = ci.request_build(BuildRequest::new("daemon", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let log: Vec<String> = ci.build_log("daemon", n, 0).expect("log").into_iter().map(|l| l.text).collect();
    assert_eq!(ci.build("daemon", n).expect("build").status, BuildStatus::Succeeded, "{log:#?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(30), "the step waited for the daemon");
    assert!(log.iter().any(|l| l.contains("o comando deixou") && l.contains("parados")), "{log:#?}");
    assert!(!alive(), "the daemon outlived the build");

    // Kept: it stays, and the log says so; the test stops it.
    let kept = format!(
        "[[stages]]\nname = \"Fica\"\nsteps = [{{ sh = 'sh -c \"sleep 600; : {marker}\" >/dev/null 2>&1 & echo deixei', keep_daemons = true }}]\n"
    );
    ci.save_job(JobSpec::new("fica", "toml", kept)).expect("saved");
    let n = ci.request_build(BuildRequest::new("fica", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let log: Vec<String> = ci.build_log("fica", n, 0).expect("log").into_iter().map(|l| l.text).collect();
    assert!(log.iter().any(|l| l.contains("keep_daemons")), "{log:#?}");
    assert!(alive(), "keep_daemons keeps it");
    let _ = std::process::Command::new("pkill").args(["-f", &marker]).status();
}
