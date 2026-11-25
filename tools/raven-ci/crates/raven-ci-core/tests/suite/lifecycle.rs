//! The life of a build around its steps (#861): its folder, its log with secrets hidden, the code
//! fetched before the first step, the post steps, what is taken from it, the status of its commit
//! and the notices.

use std::collections::BTreeMap;
use std::sync::Arc;

use raven_ci_core::{Ci, CiOptions, JobSpec, MemorySecrets};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{
    BuildEvent, BuildRequest, BuildScope, BuildStatus, CiError, CiPlugin, CommitState, FormatError, Host,
    PLUGIN_KEY, Permission, Pipeline, PipelineFormat, Point, PostSteps, PostWhen, STAGE_KEY, Use,
};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_flows::{Catalog, FlowDefinition, NodeContext, NodeDef, NodeError, NodeKind, NodeSpec, Ports};
use serde_json::{Value, json};

/// The step `say`: writes each of its `lines` to the build's log, the folder it runs in, a file
/// `file` in it, the variable `show` of its environment, and fails with `fail`.
struct Say;

impl NodeKind for Say {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("say", 1).input("main").output("main")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let step = cx.node().id.clone();
        for line in cx.parameter("lines").and_then(Value::as_array).into_iter().flatten() {
            build.log(&step, line.as_str().unwrap_or_default());
        }
        if let Some(name) = cx.parameter("show").and_then(Value::as_str) {
            let value = build.environment().get(name).cloned().unwrap_or_default();
            build.log(&step, &format!("{name}={value}"));
        }
        if let Some(file) = cx.parameter("file").and_then(Value::as_str) {
            std::fs::write(build.context().workspace.join(file), "feito")
                .map_err(|e| NodeError::new(e.to_string()))?;
        }
        if cx.parameter("fail").and_then(Value::as_bool) == Some(true) {
            return Err(NodeError::new("falhou, como pedido"));
        }
        Ok(BuildScope::pass_on(cx))
    }
}

/// Reads the pipeline as JSON: `{"steps": [...], "post": {"always": [...]}, "publishers": [...],
/// "environment": {...}}`, each step the parameters of a `say` (with `stage`).
struct Json;

fn chain(job: &str, steps: &[Value]) -> FlowDefinition {
    let mut flow = FlowDefinition::new(job, job).node(NodeDef::new("start", "manual"));
    let mut previous = "start".to_owned();
    for (n, step) in steps.iter().enumerate() {
        let id = step.get("id").and_then(Value::as_str).map_or_else(|| format!("s{}", n + 1), str::to_owned);
        let mut node = NodeDef::new(&id, step.get("kind").and_then(Value::as_str).unwrap_or("say"))
            .parameter(STAGE_KEY, step.get("stage").cloned().unwrap_or(json!("Build")))
            .parameter(PLUGIN_KEY, step.get("plugin").cloned().unwrap_or(json!("ciclo")));
        for (key, value) in step.as_object().into_iter().flatten() {
            node = node.parameter(key, value.clone());
        }
        flow = flow.node(node).connect(&previous, "main", &id, "main");
        previous = id;
    }
    flow
}

impl PipelineFormat for Json {
    fn id(&self) -> &str {
        "json"
    }

    fn file_names(&self) -> Vec<String> {
        vec!["pipeline.json".to_owned()]
    }

    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
        let value: Value = serde_json::from_str(text).map_err(|e| vec![FormatError::whole(e.to_string())])?;
        let steps = value["steps"].as_array().cloned().unwrap_or_default();
        let mut pipeline = Pipeline::new(chain(job, &steps), "start");
        for (when, name) in
            [(PostWhen::Always, "always"), (PostWhen::Success, "success"), (PostWhen::Failure, "failure")]
        {
            if let Some(steps) = value["post"][name].as_array() {
                pipeline.post.push(PostSteps { when, flow: chain(job, steps), start: "start".to_owned() });
            }
        }
        for publisher in value["publishers"].as_array().into_iter().flatten() {
            let id = publisher.as_str().unwrap_or_default();
            pipeline.publishers.push(Use::new(id, id, json!({})));
        }
        if let Some(environment) = value["environment"].as_object() {
            pipeline.environment = environment
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
                .collect();
        }
        pipeline.timeout_ms = value["timeout_ms"].as_u64();
        Ok(pipeline)
    }
}

/// The plugin of this test: the step `say`, the format `json`, and a secret it reads when it
/// starts, `token`.
struct Ciclo;

impl Module for Ciclo {
    fn name(&self) -> &str {
        "ciclo"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version("1.0.0")
            .unwrap_or_default()
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for Ciclo {
    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.secret("token").map(|_| ())
    }

    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Say);
    }

    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        vec![Arc::new(Json)]
    }
}

struct Setup {
    ci: Ci,
    statuses: Arc<raven_ci_plugin_api::testing::Probe>,
    events: Arc<raven_ci_plugin_api::testing::Probe>,
}

fn setup() -> Setup {
    let secrets = MemorySecrets(BTreeMap::from([("token".to_owned(), "s3cr3t-valor".to_owned())]));
    let ci = Ci::new(CiOptions { secrets: Arc::new(secrets), ..CiOptions::default() }).expect("a core");
    ci.add_plugin(Arc::new(Ciclo));
    let reporter = Fake::new(Point::StatusReporter);
    let notifier = Fake::new(Point::Notifier);
    let (statuses, events) = (reporter.probe(), notifier.probe());
    for fake in
        [Fake::new(Point::Scm), Fake::new(Point::Publisher), Fake::new(Point::BuildHook), reporter, notifier]
    {
        ci.add_plugin(Arc::new(fake));
    }
    ci.apply_plugins().expect("on");
    Setup { ci, statuses, events }
}

fn job(name: &str, pipeline: Value) -> JobSpec {
    JobSpec::new(name, "json", pipeline.to_string())
}

fn texts(ci: &Ci, job: &str, number: u64) -> Vec<String> {
    ci.build_log(job, number, 0).expect("log").into_iter().map(|line| line.text).collect()
}

#[test]
fn a_step_writes_lines_to_the_log_of_its_build_in_order_with_its_step() {
    let Setup { ci, .. } = setup();
    ci.save_job(job("site", json!({ "steps": [{ "lines": ["um", "dois", "três"] }] }))).expect("saved");
    let n = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.build("site", n).expect("build").status, BuildStatus::Succeeded);
    let lines = ci.build_log("site", n, 0).expect("log");
    let of_step: Vec<(&str, &str)> =
        lines.iter().filter(|l| l.step == "s1").map(|l| (l.step.as_str(), l.text.as_str())).collect();
    assert_eq!(of_step, [("s1", "um"), ("s1", "dois"), ("s1", "três")]);
    let after = lines.iter().find(|l| l.text == "dois").expect("dois").seq;
    let rest: Vec<String> =
        ci.build_log("site", n, after).expect("log").into_iter().map(|l| l.text).collect();
    assert_eq!(rest.first().map(String::as_str), Some("três"), "reading after a line gives what came next");
}

#[test]
fn a_secret_a_plugin_read_is_hidden_in_the_log_kept() {
    let Setup { ci, .. } = setup();
    ci.save_job(job("site", json!({ "steps": [{ "lines": ["o token é s3cr3t-valor, viu"] }] })))
        .expect("saved");
    let n = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let all = texts(&ci, "site", n).join("\n");
    assert!(all.contains("o token é ****, viu"), "{all}");
    assert!(!all.contains("s3cr3t-valor"), "the secret never reaches the file: {all}");
}

#[test]
fn the_code_is_fetched_before_the_first_step_and_its_commit_gets_pending_then_the_end() {
    let Setup { ci, statuses, events } = setup();
    let settings = json!({
        "scm": { "id": "fake-scm", "url": "https://exemplo/repo.git" },
        "status": [{ "id": "fake-status", "repository": "dono/repo" }],
        "notify": [{ "id": "fake-notifier" }],
    });
    ci.save_job(job("repo/main", json!({ "steps": [{ "show": "GIT_COMMIT" }] })).settings(settings))
        .expect("saved");
    let n = ci.request_build(BuildRequest::new("repo/main", "push")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let log = texts(&ci, "repo/main", n);
    let fetched = log.iter().position(|l| l == "buscando https://exemplo/repo.git").expect("fetched");
    let step = log.iter().position(|l| l == "GIT_COMMIT=f00d").expect("the step saw the commit");
    assert!(fetched < step, "{log:?}");
    let states: Vec<CommitState> = statuses.statuses().iter().map(|s| s.state).collect();
    assert_eq!(states, [CommitState::Pending, CommitState::Success]);
    assert_eq!(statuses.statuses()[0].commit, "f00d");
    assert_eq!(statuses.statuses()[0].context, "raven-ci/repo/main");
    assert!(statuses.statuses().iter().all(|s| s.url.is_none()), "no address, no link");
    let told = events.events();
    assert!(matches!(told.first(), Some(BuildEvent::Started(_))));
    assert!(matches!(told.last(), Some(BuildEvent::Ended(info)) if info.status == BuildStatus::Succeeded));
}

#[test]
fn a_build_whose_code_cannot_be_fetched_fails_before_any_step_saying_why() {
    let Setup { ci, .. } = setup();
    let settings = json!({ "scm": { "id": "svn", "url": "x" } });
    ci.save_job(job("velho", json!({ "steps": [{ "file": "nunca.txt" }] })).settings(settings))
        .expect("saved");
    let n = ci.request_build(BuildRequest::new("velho", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let build = ci.build("velho", n).expect("build");
    assert_eq!(build.status, BuildStatus::Failed);
    assert!(build.reason.as_deref().unwrap_or_default().contains("falta o plugin svn"), "{build:?}");
    assert!(ci.stages("velho", n).expect("stages")[0].steps[0].status == "pending");
}

#[test]
fn post_failure_runs_only_when_a_step_failed_and_post_always_runs_every_time() {
    let Setup { ci, .. } = setup();
    let post = json!({
        "always": [{ "id": "sempre", "lines": ["sempre"] }],
        "success": [{ "id": "passou", "lines": ["passou"] }],
        "failure": [{ "id": "falhou", "lines": ["falhou"] }],
    });
    ci.save_job(job("ok", json!({ "steps": [{ "lines": ["x"] }], "post": post }))).expect("saved");
    ci.save_job(job("ruim", json!({ "steps": [{ "fail": true }], "post": post }))).expect("saved");
    let ok = ci.request_build(BuildRequest::new("ok", "teste")).expect("asked").number;
    let bad = ci.request_build(BuildRequest::new("ruim", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let ok_log = texts(&ci, "ok", ok);
    let bad_log = texts(&ci, "ruim", bad);
    assert!(ok_log.contains(&"sempre".to_owned()) && ok_log.contains(&"passou".to_owned()), "{ok_log:?}");
    assert!(!ok_log.contains(&"falhou".to_owned()), "{ok_log:?}");
    assert!(bad_log.contains(&"sempre".to_owned()) && bad_log.contains(&"falhou".to_owned()), "{bad_log:?}");
    assert!(!bad_log.contains(&"passou".to_owned()), "{bad_log:?}");
    assert_eq!(ci.build("ruim", bad).expect("build").status, BuildStatus::Failed);
}

#[test]
fn what_a_publisher_finds_is_kept_and_one_that_fails_leaves_the_build_as_it_ended() {
    let Setup { ci, .. } = setup();
    ci.save_job(job("pub", json!({ "steps": [{ "lines": ["x"] }], "publishers": ["fake-publisher"] })))
        .expect("saved");
    let n = ci.request_build(BuildRequest::new("pub", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let attachments = ci.attachments("pub", n).expect("attachments");
    assert_eq!(attachments.len(), 1);
    assert_eq!(ci.attachment_bytes(attachments[0].id).expect("bytes"), b"pub #1 passou");

    // A publisher that went away between the request and the end is said in the log.
    ci.save_job(job("pub2", json!({ "steps": [{ "lines": ["x"] }], "publishers": ["fake-publisher"] })))
        .expect("saved");
    let m = ci.request_build(BuildRequest::new("pub2", "teste")).expect("asked").number;
    ci.disable_plugin("fake-publisher").expect("off");
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.build("pub2", m).expect("build").status, BuildStatus::Succeeded);
    assert!(texts(&ci, "pub2", m).iter().any(|l| l.contains("falta o plugin fake-publisher")));
}

#[test]
fn the_folder_of_a_build_is_gone_when_it_ends_also_when_it_failed_or_was_cancelled() {
    let Setup { ci, .. } = setup();
    ci.save_job(job("pasta", json!({ "steps": [{ "file": "a.txt" }] }))).expect("saved");
    ci.save_job(job("quebra", json!({ "steps": [{ "file": "a.txt", "fail": true }] }))).expect("saved");
    ci.save_job(job("cancela", json!({ "steps": [{ "file": "a.txt" }] }))).expect("saved");
    let ok = ci.request_build(BuildRequest::new("pasta", "teste")).expect("asked").number;
    let bad = ci.request_build(BuildRequest::new("quebra", "teste")).expect("asked").number;
    let gone = ci.request_build(BuildRequest::new("cancela", "teste")).expect("asked").number;
    ci.cancel("cancela", gone, "não precisa").expect("cancelled");
    ci.work_until_idle("e1").expect("runs");
    for (job, n) in [("pasta", ok), ("quebra", bad), ("cancela", gone)] {
        assert!(!ci.workspace(job, n).exists(), "{job} #{n} left its folder");
    }
    let wrote = texts(&ci, "pasta", ok);
    assert!(wrote.iter().any(|l| l.starts_with("build pasta #1 em ")), "{wrote:?}");

    // A job that keeps its folder keeps it.
    ci.save_job(
        job("fica", json!({ "steps": [{ "file": "a.txt" }] }))
            .settings(json!({ "workspace": { "keep": true } })),
    )
    .expect("saved");
    let kept = ci.request_build(BuildRequest::new("fica", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    assert!(ci.workspace("fica", kept).join("a.txt").exists());
}

#[test]
fn a_pipeline_whose_step_comes_from_a_plugin_switched_off_is_refused_naming_the_plugin() {
    let Setup { ci, .. } = setup();
    ci.save_job(job("sh", json!({ "steps": [{ "kind": "say", "plugin": "shell" }] }))).expect("saved");
    let refused = ci.request_build(BuildRequest::new("sh", "teste")).expect_err("refused");
    assert!(refused.to_string().contains("falta o plugin shell"), "{refused}");
    assert!(ci.queue().expect("queue").is_empty(), "nothing went to the queue");
}

#[test]
fn every_step_gets_the_variables_of_its_build_and_of_its_pipeline() {
    let Setup { ci, .. } = setup();
    let pipeline = json!({
        "environment": { "MODO": "rapido" },
        "steps": [{ "show": "MODO" }, { "show": "BUILD_NUMBER" }, { "show": "BRANCH_NAME" }, { "show": "E2E" }],
    });
    ci.save_job(job("repo/PR-7", pipeline)).expect("saved");
    let n = ci
        .request_build(BuildRequest::new("repo/PR-7", "teste").parameter("E2E", json!(true)))
        .expect("asked");
    ci.work_until_idle("e1").expect("runs");
    let log = texts(&ci, "repo/PR-7", n.number);
    for wanted in ["MODO=rapido", "BUILD_NUMBER=1", "BRANCH_NAME=PR-7", "E2E=true"] {
        assert!(log.contains(&wanted.to_owned()), "{wanted} in {log:?}");
    }
}

#[test]
fn a_rerun_does_what_the_first_build_did_around_its_steps() {
    let Setup { ci, .. } = setup();
    ci.save_job(job(
        "de-novo",
        json!({ "steps": [{ "lines": ["x"] }], "post": { "always": [{ "lines": ["post"] }] } }),
    ))
    .expect("saved");
    let n = ci.request_build(BuildRequest::new("de-novo", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let again = ci.rebuild("de-novo", n, None).expect("again");
    ci.work_until_idle("e1").expect("runs");
    assert!(texts(&ci, "de-novo", again).contains(&"post".to_owned()));
}

#[test]
fn a_hook_refuses_a_build_before_its_code_and_hears_how_the_others_ended() {
    let Setup { ci, .. } = setup();
    ci.save_job(job("cheio", json!({ "steps": [{ "file": "nunca.txt" }] })).settings(json!({
        "fake-hook": { "refuse": "o disco tem 1 GB livre, menos que os 5 GB pedidos" },
        "scm": { "id": "fake-scm", "url": "x" },
    })))
    .expect("saved");
    let n = ci.request_build(BuildRequest::new("cheio", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let build = ci.build("cheio", n).expect("build");
    assert_eq!(build.status, BuildStatus::Failed);
    assert!(build.reason.as_deref().unwrap_or_default().contains("menos que os 5 GB"), "{build:?}");
    assert!(!texts(&ci, "cheio", n).iter().any(|l| l.starts_with("buscando")), "the code was not fetched");
}

#[test]
fn the_folders_above_a_build_left_empty_go_with_it() {
    let Setup { ci, .. } = setup();
    ci.save_job(job("apps/web/site", json!({ "steps": [{ "file": "a.txt" }] }))).expect("saved");
    let n = ci.request_build(BuildRequest::new("apps/web/site", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let workspace = ci.workspace("apps/web/site", n);
    let root = workspace.ancestors().nth(4).expect("the folder of every build").to_path_buf();
    assert!(!root.join("apps").exists(), "{} was left", root.join("apps").display());
}

#[test]
fn the_notices_and_the_status_of_the_commit_link_to_the_page_of_the_build() {
    let Setup { ci, statuses, events } = setup();
    ci.store().set_setting("url", "http://ci.exemplo:8090/").expect("the address");
    let settings = json!({
        "scm": { "id": "fake-scm", "url": "https://exemplo/repo.git" },
        "status": [{ "id": "fake-status", "repository": "dono/repo" }],
        "notify": [{ "id": "fake-notifier" }],
    });
    ci.save_job(job("repo/main", json!({ "steps": [{ "show": "GIT_COMMIT" }] })).settings(settings))
        .expect("saved");
    let n = ci.request_build(BuildRequest::new("repo/main", "push")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let page = format!("http://ci.exemplo:8090/painel/#/build/repo%2Fmain/{n}");
    assert_eq!(ci.build_url("repo/main", n).as_deref(), Some(page.as_str()));
    let linked: Vec<Option<String>> = statuses.statuses().iter().map(|s| s.url.clone()).collect();
    assert_eq!(linked, [Some(page.clone()), Some(page.clone())], "pending and the end");
    let told = events.events();
    assert!(!told.is_empty());
    for event in told {
        match event {
            BuildEvent::Started(info) | BuildEvent::Ended(info) => {
                assert_eq!(info.url.as_deref(), Some(page.as_str()));
            }
            _ => {}
        }
    }
}
