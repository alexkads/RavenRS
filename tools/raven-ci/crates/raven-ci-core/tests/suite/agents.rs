//! Build agents on the server's side (#893, #895): getting in with a token used once, who may use
//! the routes, which build goes to which agent, the log sent twice, attachments in parts, the
//! lease of a long step, and the build of an agent that went quiet.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use raven_ci_core::agents::{AgentLine, End, Hello, Join, LogBatch, ROLE_AGENT};
use raven_ci_core::{Ci, CiOptions, Executors, JobSpec, api};
use raven_ci_plugin_api::{
    AgentLauncher, Attachment, BuildRequest, BuildStatus, CiPlugin, FormatError, LaunchRequest, LaunchedAgent,
    PLUGIN_KEY, Pipeline, PipelineFormat, ROLE_ADMIN, ROLE_READ, STAGE_KEY, Use,
};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_app_server::ApiRequest;
use ravenrs_flows::{
    CancelToken, Catalog, FlowDefinition, ManualClock, NodeContext, NodeDef, NodeError, NodeKind, NodeSpec,
    Ports, RunOutcome, SystemClock,
};
use serde_json::{Value, json};

use crate::common::OnDisk;

const PASSWORD: &str = "uma-senha-de-teste";

/// How many times `conta` ran, by its parameter `marca`.
fn counted() -> &'static Mutex<HashMap<String, u32>> {
    static COUNTED: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();
    COUNTED.get_or_init(Mutex::default)
}

/// The step `conta`: counts its runs by its `marca` and waits `sleep_ms`.
struct Conta;

impl NodeKind for Conta {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("conta", 1).input("main").output("main")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let mark = cx.parameter("marca").and_then(Value::as_str).unwrap_or_default().to_owned();
        *counted().lock().expect("lock").entry(mark).or_default() += 1;
        if let Some(ms) = cx.parameter("sleep_ms").and_then(Value::as_u64) {
            std::thread::sleep(Duration::from_millis(ms));
        }
        Ok(Ports::from([("main".to_owned(), cx.input("main").to_vec())]))
    }
}

/// `{"agent": "linux", "steps": [{"sleep_ms": 10}]}`: one `conta` per step.
struct Format;

impl PipelineFormat for Format {
    fn id(&self) -> &str {
        "agentes"
    }

    fn file_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
        let value: Value = serde_json::from_str(text).map_err(|e| vec![FormatError::whole(e.to_string())])?;
        let mut flow = FlowDefinition::new(job, job).node(NodeDef::new("start", "manual"));
        let mut previous = "start".to_owned();
        for (n, step) in value["steps"].as_array().into_iter().flatten().enumerate() {
            let id = format!("s{}", n + 1);
            let mut node = NodeDef::new(&id, "conta")
                .parameter(STAGE_KEY, json!("Build"))
                .parameter(PLUGIN_KEY, json!("agentes"));
            for (key, value) in step.as_object().into_iter().flatten() {
                node = node.parameter(key, value.clone());
            }
            flow = flow.node(node).connect(&previous, "main", &id, "main");
            previous = id;
        }
        let mut pipeline = Pipeline::new(flow, "start");
        if let Some(label) = value["agent"].as_str() {
            pipeline.agent =
                Some(Use { id: "label".into(), plugin: None, settings: json!({ "label": label }) });
        }
        if let Some(launcher) = value["launcher"].as_str() {
            pipeline.agent = Some(Use { id: launcher.into(), plugin: None, settings: value["with"].clone() });
        }
        Ok(pipeline)
    }
}

struct Plugin;

impl Module for Plugin {
    fn name(&self) -> &str {
        "agentes"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version("1.0.0").unwrap_or_default()
    }
}

impl CiPlugin for Plugin {
    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Conta);
    }

    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        vec![Arc::new(Format)]
    }

    fn agent_launchers(&self) -> Vec<Arc<dyn AgentLauncher>> {
        vec![Arc::new(Dying)]
    }
}

/// The launcher `morre`: its agent is never alive, and says why, as an image where the agent
/// does not run (#934).
struct Dying;

struct Dead(String);

impl LaunchedAgent for Dead {
    fn name(&self) -> String {
        self.0.clone()
    }
    fn alive(&self) -> bool {
        false
    }
    fn stop(&self) {}
    fn last_words(&self) -> Option<String> {
        Some("o contêiner saiu com o código 1: version 'GLIBC_2.44' not found".to_owned())
    }
}

impl AgentLauncher for Dying {
    fn id(&self) -> &str {
        "morre"
    }
    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, raven_ci_plugin_api::CiError> {
        let name = request.settings["name"].as_str().unwrap_or("morto").to_owned();
        Ok(Box::new(Dead(name)))
    }
}

/// A core on disk with the plugin of this test, an administrator and a reader.
fn core(disk: &OnDisk, clock: Option<&ManualClock>) -> Ci {
    let mut options = CiOptions { data_dir: Some(disk.dir.clone()), ..CiOptions::default() };
    if let Some(clock) = clock {
        options.clock = Arc::new(clock.clone());
    }
    let ci = Ci::new(options).expect("a core");
    ci.add_plugin(Arc::new(Plugin));
    ci.apply_plugins().expect("on");
    let users = api::users_file(&disk.dir);
    api::create_user(&users, "ana", PASSWORD, ROLE_ADMIN).expect("ana");
    api::create_user(&users, "bia", PASSWORD, ROLE_READ).expect("bia");
    ci
}

fn hello(session: &str, labels: &[&str]) -> Hello {
    Hello {
        labels: labels.iter().map(|l| (*l).to_owned()).collect(),
        executors: 1,
        free: 1,
        session: session.to_owned(),
        ..Hello::default()
    }
}

/// Lets the agent `name` in; its key.
fn join(ci: &Ci, name: &str, labels: &[&str]) -> String {
    let labels_owned: Vec<String> = labels.iter().map(|l| (*l).to_owned()).collect();
    let token = ci.agent_token(name, &labels_owned, 1).expect("a token");
    ci.agent_join(&Join { name: name.into(), token, hello: hello("s1", labels) }).expect("in").key
}

fn job(ci: &Ci, name: &str, pipeline: Value) -> u64 {
    ci.save_job(JobSpec::new(name, "agentes", pipeline.to_string())).expect("the job");
    ci.request_build(BuildRequest::new(name, "teste")).expect("asked").number
}

#[test]
fn a_token_works_once_for_its_name_and_while_it_lasts() {
    let disk = OnDisk::new("agentes-token");
    let clock = ManualClock::at(1_000_000);
    let ci = core(&disk, Some(&clock));
    let token = ci.agent_token("linux-1", &["linux".into()], 1).expect("a token");
    let other = Join { name: "linux-2".into(), token: token.clone(), hello: hello("s", &[]) };
    assert!(ci.agent_join(&other).is_err(), "a token of another name");
    let again = Join { name: "linux-1".into(), token, hello: hello("s", &[]) };
    assert!(ci.agent_join(&again).is_err(), "the token went with the first try, even a wrong one");

    let token = ci.agent_token("linux-1", &["linux".into()], 1).expect("a token");
    clock.advance(raven_ci_core::agents::TOKEN_MS + 1);
    assert!(
        ci.agent_join(&Join { name: "linux-1".into(), token, hello: hello("s", &[]) }).is_err(),
        "run out"
    );

    let key = join(&ci, "linux-1", &["linux"]);
    assert!(key.starts_with("rvk_"));
    let agents = ci.agents().expect("agents");
    assert_eq!(agents.len(), 1);
    assert!(agents[0].connected);
    assert_eq!(agents[0].labels, ["linux"]);
}

#[test]
fn the_routes_of_agents_are_only_for_agents_and_an_agent_reaches_nothing_else() {
    let disk = OnDisk::new("agentes-rotas");
    let ci = Arc::new(core(&disk, None));
    let mut service = api::service(ci.clone(), &api::users_file(&disk.dir)).expect("the service");
    let key = join(&ci, "linux-1", &["linux"]);

    let oi = |bearer: &str| {
        ApiRequest::new("POST", "/api/ci/agentes/oi").bearer(bearer).json(&json!(hello("s1", &["linux"])))
    };
    assert_eq!(service.handle(oi(&key)).status, 200);
    assert_eq!(service.handle(ApiRequest::new("POST", "/api/ci/agentes/oi").json(&json!({}))).status, 401);
    let signed = service.handle(
        ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": "bia", "password": PASSWORD })),
    );
    let bia = signed.body["token"].as_str().expect("a token").to_owned();
    assert_eq!(service.handle(oi(&bia)).status, 403, "a person is not an agent");
    assert_eq!(service.handle(ApiRequest::new("GET", "/api/ci/agents").bearer(&bia)).status, 200);
    let credentials = service.handle(ApiRequest::new("GET", "/api/ci/credentials").bearer(&key));
    assert_eq!(credentials.status, 403, "an agent is no administrator");
    let bad = service.handle(
        ApiRequest::new("POST", "/api/ci/agentes/entrar")
            .json(&json!({ "name": "linux-1", "token": "rva_nada", "hello": hello("s", &[]) })),
    );
    assert_eq!(bad.status, 401);

    ci.remove_agent("linux-1").expect("removed");
    let mut service = api::service(ci.clone(), &api::users_file(&disk.dir)).expect("the service");
    assert_eq!(service.handle(oi(&key)).status, 401, "a removed agent's key opens nothing");
    assert_eq!(ROLE_AGENT, "Agentes");
}

#[test]
fn a_build_goes_only_to_an_agent_with_its_label_and_never_to_the_servers_executors() {
    let disk = OnDisk::new("agentes-rotulos");
    let ci = core(&disk, None);
    join(&ci, "mac-1", &["mac"]);
    join(&ci, "linux-1", &["linux"]);
    let number = job(&ci, "apps/site", json!({ "agent": "linux", "steps": [{}] }));
    assert_eq!(ci.work_once("executor-1").expect("looked"), None, "the server's executor leaves it");
    assert!(ci.agent_take("mac-1").expect("looked").is_none(), "the wrong label");
    let task = ci.agent_take("linux-1").expect("looked").expect("a build");
    assert_eq!((task.order.job.as_str(), task.order.number), ("apps/site", number));
    assert_eq!(task.plugins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["agentes"]);
    assert!(ci.agent_take("linux-1").expect("looked").is_none(), "taken once");
    assert_eq!(ci.build("apps/site", number).expect("build").status, BuildStatus::Running);

    // A build with no agent goes to either.
    let free = job(&ci, "apps/livre", json!({ "steps": [{}] }));
    assert_eq!(ci.work_once("executor-1").expect("ran"), Some(("apps/livre".to_owned(), free)));
}

#[test]
fn a_build_run_on_an_agent_ends_on_the_server_with_its_log_once_and_its_attachment_whole() {
    let disk = OnDisk::new("agentes-fim");
    let ci = core(&disk, None);
    join(&ci, "linux-1", &["linux"]);
    let number = job(&ci, "apps/site", json!({ "agent": "linux", "steps": [{}, {}] }));
    let task = ci.agent_take("linux-1").expect("looked").expect("a build");
    let run = task.lease.id.clone();

    let catalog = Catalog::builtin().with(Conta);
    let mut records = Vec::new();
    let done = task
        .lease
        .run(&catalog, Arc::new(SystemClock), CancelToken::new(), |step| records.push(step))
        .expect("compiles");
    assert!(ci.agent_steps("linux-1", &run, &records).expect("written").held);

    let lines =
        |texts: &[&str]| texts.iter().map(|t| AgentLine { step: "s1".into(), text: (*t).into() }).collect();
    let batch =
        |from, texts: &[&str]| LogBatch { run: run.clone(), session: "s1".into(), from, lines: lines(texts) };
    assert_eq!(ci.agent_log("linux-1", &batch(0, &["um", "dois"])).expect("kept"), 2);
    // Sent again after the network fell, with one more line.
    assert_eq!(ci.agent_log("linux-1", &batch(0, &["um", "dois", "três"])).expect("kept"), 3);
    let texts: Vec<String> =
        ci.build_log("apps/site", number, 0).expect("log").into_iter().map(|l| l.text).collect();
    assert_eq!(texts, ["um", "dois", "três"]);
    assert!(ci.agent_log("mac-1", &batch(3, &["x"])).is_err(), "only the agent that holds it writes");

    let part = |bytes: &[u8]| Attachment {
        kind: "artifact".into(),
        name: "saida.bin".into(),
        mime: "application/octet-stream".into(),
        bytes: bytes.to_vec(),
        summary: Value::Null,
    };
    assert_eq!(ci.agent_attach("linux-1", &run, &part(b"abc"), false).expect("a part"), None);
    let id = ci.agent_attach("linux-1", &run, &part(b"def"), true).expect("the last").expect("kept");
    assert_eq!(ci.attachment_bytes(id).expect("bytes"), b"abcdef");

    assert_eq!(ci.agent_outcome("linux-1", &run, &done.outcome).expect("ended"), BuildStatus::Succeeded);
    ci.agent_end("linux-1", &End { run: run.clone(), status: BuildStatus::Succeeded, commit: None })
        .expect("end");
    let stages = ci.stages("apps/site", number).expect("stages");
    assert_eq!(stages[0].steps.len(), 2);
    assert!(!ci.agent_steps("linux-1", &run, &[]).expect("asked").held, "the run is over");
}

#[test]
fn a_step_longer_than_the_lease_runs_once_with_two_executors() {
    let disk = OnDisk::new("agentes-lease");
    let ci = Arc::new(core(&disk, None));
    ci.set_lease(Duration::from_millis(300));
    let number = job(&ci, "apps/longo", json!({ "steps": [{ "sleep_ms": 1500, "marca": "longo" }] }));
    let executors = Executors::start(ci.clone(), 2, Duration::from_millis(20));
    let until = std::time::Instant::now() + Duration::from_secs(20);
    while !ci.build("apps/longo", number).expect("build").status.ended() && std::time::Instant::now() < until
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    executors.stop();
    assert_eq!(ci.build("apps/longo", number).expect("build").status, BuildStatus::Succeeded);
    assert_eq!(counted().lock().expect("lock")["longo"], 1, "the second executor never took it");
}

#[test]
fn an_agent_that_falls_gets_its_build_again_and_one_that_does_not_come_back_leaves_it_interrupted() {
    let disk = OnDisk::new("agentes-cai");
    let clock = ManualClock::at(1_000_000);
    let ci = core(&disk, Some(&clock));
    ci.set_agent_grace(Duration::from_secs(60));
    join(&ci, "linux-1", &["linux"]);
    let number = job(&ci, "apps/site", json!({ "agent": "linux", "steps": [{}] }));
    let task = ci.agent_take("linux-1").expect("looked").expect("a build");
    let run = task.lease.id.clone();

    // The process died and came back: a new session gets the same build, from the queue.
    ci.agent_hello("linux-1", &hello("s2", &["linux"])).expect("hello");
    let again = ci.agent_take("linux-1").expect("looked").expect("its build again");
    assert_eq!(again.lease.id, run);

    // Quiet within the grace: the lease is renewed and no one takes the build.
    clock.advance(59_000);
    ci.tick_agents();
    assert_eq!(ci.build("apps/site", number).expect("build").status, BuildStatus::Running);
    join(&ci, "linux-2", &["linux"]);
    assert!(ci.agent_take("linux-2").expect("looked").is_none(), "another agent waits for it");

    // Past the grace: interrupted, and the agent that comes back later forgets it.
    clock.advance(62_000);
    ci.tick_agents();
    let build = ci.build("apps/site", number).expect("build");
    assert_eq!(build.status, BuildStatus::Interrupted, "{build:?}");
    assert!(build.reason.as_deref().is_some_and(|r| r.starts_with("interrompido:")), "{build:?}");
    let texts: Vec<String> =
        ci.build_log("apps/site", number, 0).expect("log").into_iter().map(|l| l.text).collect();
    assert!(texts.iter().any(|t| t.contains("o agente linux-1 sumiu")), "{texts:?}");
    let mut back = hello("s3", &["linux"]);
    back.running = vec![run.clone()];
    let answer = ci.agent_hello("linux-1", &back).expect("hello");
    assert_eq!(answer.forget, vec![run.clone()]);
    assert!(ci.agent_outcome("linux-1", &run, &RunOutcome::Finished).is_err(), "too late to end it");
}

#[test]
fn a_build_in_the_queue_says_what_it_waits_for() {
    let disk = OnDisk::new("agentes-espera");
    let ci = core(&disk, None);
    let free = job(&ci, "apps/livre", json!({ "steps": [{}] }));
    assert_eq!(ci.waiting_for("apps/livre", free).expect("why").why, "espera um executor livre");

    let number = job(&ci, "apps/imagens", json!({ "agent": "docker", "steps": [{}] }));
    let why = ci.waiting_for("apps/imagens", number).expect("why");
    assert_eq!(why.why, "pede o rótulo docker, e nenhum agente conectado o tem");
    assert_eq!(why.plugin, None);

    // A template brings it up, while its launcher's plugin is on.
    let template = raven_ci_core::agents::AgentTemplate {
        name: "docker".into(),
        labels: vec!["docker".into()],
        launcher: "agentes".into(),
        settings: json!({}),
        executors: 1,
        max_agents: 1,
        idle_ms: 60_000,
    };
    ci.save_agent_template(&template).expect("saved");
    assert_eq!(
        ci.waiting_for("apps/imagens", number).expect("why").why,
        "pede o rótulo docker · o modelo docker sobe um agente"
    );
    ci.disable_plugin("agentes").expect("off");
    let why = ci.waiting_for("apps/imagens", number).expect("why");
    assert_eq!(why.why, "pede o rótulo docker · o plugin agentes está desligado");
    assert_eq!(why.plugin.as_deref(), Some("agentes"), "the panel offers to switch it on");

    // An agent with the label, connected, takes it when free.
    ci.enable_plugin("agentes").expect("on");
    join(&ci, "docker-1", &["docker"]);
    assert_eq!(
        ci.waiting_for("apps/imagens", number).expect("why").why,
        "espera um agente com o rótulo docker ficar livre"
    );
}

/// An agent brought up for a build that dies before coming in says why in the build's log, and
/// after three of them the build fails with the reason instead of waiting forever; none of them
/// stays in the list of agents (#934).
#[test]
fn a_build_whose_agents_die_before_coming_in_fails_after_three_tries_saying_why() {
    let disk = OnDisk::new("agentes-morrem");
    let clock = ManualClock::at(1_000_000);
    let ci = core(&disk, Some(&clock));
    let number = job(&ci, "apps/imagem", json!({ "launcher": "morre", "steps": [{}] }));
    for _ in 0..6 {
        ci.tick_agents();
        clock.advance(31_000);
    }
    let build = ci.build("apps/imagem", number).expect("the build");
    let log: Vec<String> =
        ci.build_log("apps/imagem", number, 0).expect("log").into_iter().map(|l| l.text).collect();
    assert_eq!(build.status, BuildStatus::Failed, "{log:#?}");
    assert!(log.iter().any(|l| l.contains("(tentativa 1 de 3)") && l.contains("GLIBC_2.44")), "{log:#?}");
    assert!(log.iter().any(|l| l.contains("nenhum agente subiu em 3 tentativas")), "{log:#?}");
    assert!(!log.iter().any(|l| l.contains("tentativa 4")), "{log:#?}");
    assert!(ci.agents().expect("agents").is_empty(), "the agents of the build left the list");

    // The launcher's settings may give more tries.
    let more = job(&ci, "apps/paciente", json!({ "launcher": "morre", "with": { "tentativas": 5 }, "steps": [{}] }));
    for _ in 0..4 {
        ci.tick_agents();
        clock.advance(31_000);
    }
    assert_eq!(ci.build("apps/paciente", more).expect("the build").status, BuildStatus::Queued);
}
