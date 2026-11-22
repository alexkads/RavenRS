use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use raven_ci_plugin_api::testing::{ECHO, Fake, FakeHost, FakeLines};
use raven_ci_plugin_api::{
    BuildEvent, BuildInfo, BuildRequest, BuildStatus, Builds, CheckoutRequest, CiError, CiPlugin,
    CommitState, CommitStatus, HttpRequest, LaunchRequest, PipelineFormat, Point, Reference, STAGE_KEY,
    check_settings, points_of,
};
use ravenrs_flows::{Catalog, Executor, Item, Plan};
use serde_json::json;

fn started(point: Point) -> Fake {
    let fake = Fake::new(point);
    fake.start(Arc::new(FakeHost::new("fake", &[]))).expect("starts");
    fake
}

#[test]
fn each_fake_offers_its_point_only_once_it_started() {
    for fake in Fake::all() {
        assert!(points_of(&fake).is_empty(), "{} offers nothing before it starts", fake.summary());
        fake.start(Arc::new(FakeHost::new("fake", &[]))).expect("starts");
        let offered = points_of(&fake);
        assert_eq!(offered.len(), 1, "{}", fake.summary());
        assert!(fake.summary().ends_with(&format!("{}.", offered[0])));
        fake.stop();
        assert!(points_of(&fake).is_empty());
    }
}

#[test]
fn a_step_is_a_node_kind_a_pipeline_runs() {
    let fake = started(Point::Step);
    let mut catalog = Catalog::builtin();
    fake.steps(&mut catalog);
    assert!(catalog.get(ECHO, 1).is_some());
    let pipeline = FakeLines.read("site", "Build: compila\nTeste: testa").expect("reads");
    let plan = Plan::compile(&pipeline.flow, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).start_with(&pipeline.start, vec![Item::new(json!({}))]).run();
    assert!(run.succeeded());
    assert_eq!(run.output("step-2", "main")[0].json["said"], "testa");
}

#[test]
fn a_format_says_every_error_with_its_line_and_puts_the_stage_on_each_step() {
    let wrong = FakeLines.read("site", "Build: ok\nsem dois pontos\n\nde novo sem").expect_err("wrong");
    let said: Vec<String> = wrong.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["linha 2: falta o ':' entre o estágio e o passo", "linha 4: falta o ':' entre o estágio e o passo"]
    );
    let pipeline = FakeLines.read("site", "# um comentário\nBuild: compila").expect("reads");
    assert_eq!(pipeline.flow.find("step-2").expect("the step").parameters[STAGE_KEY], "Build");
    assert_eq!(started(Point::PipelineFormat).pipeline_formats()[0].file_names(), ["pipeline.txt"]);
}

#[test]
fn an_scm_puts_code_in_the_workspace_and_logs_what_it_did() {
    let scm = started(Point::Scm).scms()[0].clone();
    let said = Mutex::new(Vec::new());
    let request = CheckoutRequest {
        url: "https://example.com/site.git".into(),
        reference: Reference::PullRequest { number: 7, target: "main".into() },
        workspace: PathBuf::from("/tmp/ws"),
        depth: Some(1),
        credential: None,
        merge_onto_target: true,
    };
    let checkout =
        scm.checkout(&request, &|line| said.lock().expect("lock").push(line.to_owned())).expect("checks out");
    assert_eq!(checkout.commit, "f00d");
    assert_eq!(*said.lock().expect("lock"), ["buscando https://example.com/site.git"]);
}

#[test]
fn a_branch_source_leaves_forks_out_unless_told() {
    let source = started(Point::BranchSource).branch_sources()[0].clone();
    let names = |settings| -> Vec<String> {
        source.discover(&settings).expect("finds").into_iter().map(|head| head.name).collect()
    };
    assert_eq!(names(json!({})), ["main", "PR-1"]);
    assert_eq!(names(json!({ "forks": true })), ["main", "PR-1", "PR-2"]);
}

struct Counter(Mutex<Vec<BuildRequest>>);

impl Builds for Counter {
    fn request(&self, request: BuildRequest) -> Result<u64, CiError> {
        let mut asked = self.0.lock().expect("lock");
        asked.push(request);
        Ok(asked.len() as u64)
    }
}

#[test]
fn a_trigger_asks_for_builds_of_the_jobs_it_watches_until_unwatched() {
    let fake = started(Point::Trigger);
    let probe = fake.probe();
    let trigger = fake.triggers()[0].clone();
    let builds = Arc::new(Counter(Mutex::default()));
    assert!(matches!(
        trigger.watch("site", &json!({ "broken": 1 }), builds.clone()),
        Err(CiError::Settings { .. })
    ));
    trigger.watch("site", &json!({}), builds.clone()).expect("watches");
    assert_eq!(probe.fire("site").expect("asks"), 1);
    assert_eq!(builds.0.lock().expect("lock")[0].cause, "disparado pelo gatilho falso");
    trigger.unwatch("site");
    assert!(probe.fire("site").is_err());
}

#[test]
fn a_publisher_returns_attachments_with_a_summary() {
    let publisher = started(Point::Publisher).publishers()[0].clone();
    let build =
        BuildInfo { job: "site".into(), number: 3, status: BuildStatus::Failed, ..BuildInfo::default() };
    let attachments = publisher.publish(&build, &json!({}), &PathBuf::from(".")).expect("publishes");
    assert_eq!(attachments[0].bytes, b"site #3 falhou");
    assert_eq!(attachments[0].summary, json!({ "status": "failed" }));
}

#[test]
fn a_reporter_and_a_notifier_hear_about_the_build() {
    let reporter = started(Point::StatusReporter);
    let status = CommitStatus {
        repository: "dono/site".into(),
        commit: "a1".into(),
        context: "raven-ci/build".into(),
        state: CommitState::Pending,
        description: "rodando".into(),
        url: None,
    };
    reporter.status_reporters()[0].report(&status, &json!({})).expect("reports");
    assert_eq!(reporter.probe().statuses(), [status]);

    let notifier = started(Point::Notifier);
    let event = BuildEvent::Ended(BuildInfo { job: "site".into(), number: 1, ..BuildInfo::default() });
    notifier.notifiers()[0].notify(&event, &json!({})).expect("tells");
    assert_eq!(notifier.probe().events(), [event]);
}

#[test]
fn a_credential_kind_describes_its_form_and_what_a_log_hides() {
    let kind = started(Point::CredentialKind).credential_kinds()[0].clone();
    let fields = kind.fields();
    assert!(fields.fields[0].secret, "no screen shows the token");
    assert!(check_settings(&fields, &json!({})).is_err(), "the token is required");
    assert_eq!(kind.hidden(&json!({ "token": "ghp_x" })), ["ghp_x"]);
}

#[test]
fn an_agent_launcher_brings_an_agent_up_and_down() {
    let launcher = started(Point::AgentLauncher).agent_launchers()[0].clone();
    let request = LaunchRequest {
        build: Some("site#4".into()),
        labels: vec!["linux".into()],
        settings: json!({}),
        server: "http://localhost:8080".into(),
        token: "uma-vez".into(),
    };
    let agent = launcher.launch(&request).expect("launches");
    assert_eq!(agent.name(), "falso-site#4");
    assert!(agent.alive());
    agent.stop();
    assert!(!agent.alive());
}

#[test]
fn a_route_answers_with_who_asked_and_a_command_writes_to_its_output() {
    let routes = started(Point::Routes).routes();
    assert!(routes[0].matches("GET", "/api/fake/hello"));
    let request = HttpRequest { actor: Some("ana".into()), ..HttpRequest::default() };
    assert_eq!((routes[0].handler)(&request).body, br#"{"hello":"ana"}"#);

    let command = started(Point::CliCommand).cli_commands()[0].clone();
    let mut out = Vec::new();
    assert_eq!(command.run(&["a".into(), "b".into()], &mut out).expect("runs"), 0);
    assert_eq!(out, b"fake: a b\n");
}

#[test]
fn a_hook_sees_the_build_before_and_after_and_refuses_what_its_settings_say() {
    use raven_ci_plugin_api::{BuildContext, BuildStatus};
    let fake = started(Point::BuildHook);
    let probe = fake.probe();
    let hook = fake.build_hooks()[0].clone();
    let build = BuildContext { job: "site".into(), number: 2, workspace: ".".into(), run: "r".into() };
    hook.before(&build, &json!({})).expect("goes on");
    assert!(hook.before(&build, &json!({ "refuse": "sem disco" })).is_err());
    hook.after(&build, BuildStatus::Succeeded, &serde_json::Value::Null).expect("after");
    assert_eq!(probe.hooked(), ["antes site#2", "antes site#2", "depois site#2 passou"]);
}

#[test]
fn the_fake_of_screens_puts_a_view_in_the_model_once_it_started() {
    let off = Fake::new(Point::Screens);
    assert!(!raven_ci_plugin_api::has_screens(&off), "made, it adds nothing");
    let on = started(Point::Screens);
    assert!(raven_ci_plugin_api::has_screens(&on));
    assert_eq!(points_of(&on), [Point::Screens]);
    let mut model = ravenrs_app::ModelNode::new("Application");
    ravenrs_app::Module::extend_model(&on, &mut model);
    let views = model.children.iter().find(|node| node.id == "Views").expect("its views");
    assert_eq!(views.children[0].id, "fake-screens_View");
}
