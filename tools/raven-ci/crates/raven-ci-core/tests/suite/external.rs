use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use raven_ci_core::external::{ExternalPlugin, read_manifest};
use raven_ci_core::{Memory, MemorySecrets, Origin, PluginState};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CiPlugin, Permission, Point};

use crate::common::{OnDisk, example_plugins, python, test_plugins};

fn secrets() -> MemorySecrets {
    MemorySecrets(BTreeMap::from([("token".to_owned(), "s3cr3t-do-teste".to_owned())]))
}

#[test]
fn a_build_with_the_hello_step_runs_python_and_gets_its_answer() {
    if !python() {
        return;
    }
    let disk = OnDisk::new("hello");
    let ci = disk.open(secrets());
    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.discover_plugins(&example_plugins());
    ci.apply_plugins().expect("applies");
    assert_eq!(ci.registry().info("hello").expect("found").state, PluginState::On);

    // The fake format makes fake-echo steps; this pipeline is a flow with the hello step instead.
    let catalog = ci.registry().catalog();
    assert!(catalog.get("hello", 1).is_some(), "the step of the plugin is in the catalog");
    let owners = ci.registry().step_owners();
    assert_eq!(owners.get("hello").map(String::as_str), Some("hello"));

    let hello = ExternalPlugin::open(&example_plugins().join("hello")).expect("opens");
    hello
        .start(Arc::new(raven_ci_plugin_api::testing::FakeHost::new("hello", &[Permission::Secrets])))
        .expect("on");
    let answer = hello
        .call(
            "step/execute",
            serde_json::json!({ "kind": "hello", "parameters": { "who": "Ana" }, "inputs": { "main": [{ "n": 1 }] } }),
        )
        .expect("answers");
    assert_eq!(answer["outputs"]["main"][0]["hello"], "Olá, Ana!");
    assert_eq!(answer["outputs"]["main"][0]["n"], 1);
    hello.stop();
}

#[test]
fn the_hello_command_writes_what_python_says() {
    if !python() {
        return;
    }
    let disk = OnDisk::new("hello-cli");
    let ci = disk.open(secrets());
    ci.discover_plugins(&example_plugins());
    ci.apply_plugins().expect("applies");
    let command = ci
        .registry()
        .on()
        .iter()
        .flat_map(|plugin| plugin.cli_commands())
        .find(|command| command.name() == "hello")
        .expect("the command");
    let mut out = Vec::new();
    assert_eq!(command.run(&["Ana".into(), "e".into(), "Bia".into()], &mut out).expect("runs"), 0);
    assert_eq!(String::from_utf8_lossy(&out), "Olá, Ana e Bia!\n");
    let view = ci.registry().info("hello").expect("found");
    assert!(matches!(view.origin, Origin::External(_)));
    assert!(
        matches!(view.memory, Memory::Bytes(_) | Memory::Unknown),
        "its program is up: {:?}",
        view.memory
    );
    assert_eq!(view.declared, [Permission::Secrets]);
}

#[test]
fn a_build_runs_the_external_step_and_a_secret_it_reads_is_on_the_audit() {
    if !python() {
        return;
    }
    let disk = OnDisk::new("hello-build");
    let ci = disk.open(secrets());
    ci.add_plugin(Arc::new(HelloFormat));
    ci.discover_plugins(&example_plugins());
    ci.apply_plugins().expect("applies");
    ci.save_job(raven_ci_core::JobSpec::new("ola", "hello-format", "Ana")).expect("saved");
    let number = ci.request_build(BuildRequest::new("ola", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let build = ci.build("ola", number).expect("kept");
    assert_eq!(build.status, BuildStatus::Succeeded, "{:?}", build.reason);
    let audit = ci.store().audit_of("hello").expect("audit");
    assert_eq!(audit.last().map(|a| (a.what.as_str(), a.granted)), Some(("ler o segredo token", true)));
    assert!(!ci.log().lines().iter().any(|line| line.contains("s3cr3t-do-teste")), "the secret never shows");
}

#[test]
fn a_plugin_that_does_not_answer_in_time_is_stopped_and_the_next_call_starts_it_again() {
    if !python() {
        return;
    }
    let rebel = ExternalPlugin::open(&test_plugins().join("rebelde")).expect("opens");
    rebel.start(Arc::new(raven_ci_plugin_api::testing::FakeHost::new("rebelde", &[]))).expect("on");
    let slow = serde_json::json!({ "kind": "rebelde", "parameters": { "sleep": 5 }, "inputs": {} });
    let began = Instant::now();
    let error = rebel.call("step/execute", slow).expect_err("too slow");
    assert_eq!(error.to_string(), "o plugin rebelde não respondeu a step/execute em 1500 ms");
    assert!(began.elapsed() < Duration::from_secs(4));
    assert_eq!(rebel.pid(), None, "its program was stopped");
    let fine = serde_json::json!({ "kind": "rebelde", "parameters": {}, "inputs": {} });
    assert_eq!(rebel.call("step/execute", fine).expect("answers")["outputs"]["main"][0]["ok"], true);
    assert_eq!(rebel.starts(), 2, "a new program answered");
    rebel.stop();
}

#[test]
fn a_plugin_that_dies_in_a_call_fails_the_call_and_nothing_else() {
    if !python() {
        return;
    }
    let rebel = ExternalPlugin::open(&test_plugins().join("rebelde")).expect("opens");
    rebel.start(Arc::new(raven_ci_plugin_api::testing::FakeHost::new("rebelde", &[]))).expect("on");
    let crash = serde_json::json!({ "kind": "rebelde", "parameters": { "crash": true }, "inputs": {} });
    let error = rebel.call("step/execute", crash).expect_err("it died");
    assert_eq!(error.to_string(), "o plugin rebelde terminou no meio de step/execute, com o código 3");
    let fine = serde_json::json!({ "kind": "rebelde", "parameters": {}, "inputs": {} });
    assert!(rebel.call("step/execute", fine).is_ok(), "it starts again");
    rebel.stop();
}

#[test]
fn a_secret_asked_without_the_permission_is_denied_and_audited() {
    if !python() {
        return;
    }
    let host =
        Arc::new(raven_ci_plugin_api::testing::FakeHost::new("rebelde", &[]).with_secret("token", "x"));
    let rebel = ExternalPlugin::open(&test_plugins().join("rebelde")).expect("opens");
    rebel.start(host.clone()).expect("on");
    let ask = serde_json::json!({ "kind": "rebelde", "parameters": { "secret": true }, "inputs": {} });
    let error = rebel.call("step/execute", ask).expect_err("denied");
    assert!(error.to_string().contains("não tem a permissão ci.secrets"), "{error}");
    assert_eq!(host.audit(), [(Permission::Secrets, "ler o segredo token".to_owned(), false)]);
    rebel.stop();
}

#[test]
fn a_plugin_with_no_call_for_its_idle_time_is_stopped() {
    if !python() {
        return;
    }
    let rebel = ExternalPlugin::open(&test_plugins().join("rebelde")).expect("opens");
    rebel.start(Arc::new(raven_ci_plugin_api::testing::FakeHost::new("rebelde", &[]))).expect("on");
    let fine = serde_json::json!({ "kind": "rebelde", "parameters": {}, "inputs": {} });
    rebel.call("step/execute", fine).expect("answers");
    let pid = rebel.pid().expect("running");
    let began = Instant::now();
    while rebel.pid().is_some() {
        assert!(began.elapsed() < Duration::from_secs(5), "stopped after its 400 ms idle");
        std::thread::sleep(Duration::from_millis(50));
    }
    // A process just ended may still show for a moment, as a zombie holding no memory.
    let gone = || ravenrs_metrics::rss_bytes_of(pid).is_none_or(|bytes| bytes == 0);
    let began = Instant::now();
    while !gone() && !cfg!(windows) && began.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(gone() || cfg!(windows), "the process is gone");
    rebel.stop();
}

#[test]
fn a_manifest_with_problems_says_each_and_the_others_go_on() {
    let dir = std::env::temp_dir().join(format!("raven-ci-manifesto-{}", std::process::id()));
    let broken = dir.join("quebrado");
    std::fs::create_dir_all(&broken).expect("folder");
    std::fs::write(
        broken.join("plugin.toml"),
        "name = \"Quebrado\"\nversion = \"x\"\ncommand = \"nada\"\npermissions = [\"root\"]\n",
    )
    .expect("written");
    let said = read_manifest(&broken).expect_err("wrong");
    assert!(said.contains("o nome \"Quebrado\""), "{said}");
    assert!(said.contains("a versão"), "{said}");
    assert!(said.contains("a permissão \"root\" não existe"), "{said}");
    assert!(said.contains("não oferece nenhum passo"), "{said}");
    let ci = raven_ci_core::Ci::in_memory().expect("a core");
    ci.discover_plugins(&dir);
    ci.discover_plugins(&example_plugins());
    assert_eq!(ci.registry().problems().len(), 1);
    assert!(ci.registry().info("hello").is_ok(), "the good one is there");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A format that makes one `hello` step greeting the text of the job, reading the secret.
struct HelloFormat;

impl ravenrs_app::Module for HelloFormat {
    fn name(&self) -> &str {
        "hello-format"
    }
}

impl CiPlugin for HelloFormat {
    fn pipeline_formats(&self) -> Vec<Arc<dyn raven_ci_plugin_api::PipelineFormat>> {
        vec![Arc::new(HelloFormat)]
    }
}

impl raven_ci_plugin_api::PipelineFormat for HelloFormat {
    fn id(&self) -> &str {
        "hello-format"
    }

    fn file_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn read(
        &self,
        job: &str,
        text: &str,
    ) -> Result<raven_ci_plugin_api::Pipeline, Vec<raven_ci_plugin_api::FormatError>> {
        use ravenrs_flows::{FlowDefinition, NodeDef};
        let flow = FlowDefinition::new(job, job)
            .node(NodeDef::new("start", "manual"))
            .node(
                NodeDef::new("ola", "hello")
                    .parameter(raven_ci_plugin_api::STAGE_KEY, serde_json::json!("Saudação"))
                    .parameter("who", serde_json::json!(text))
                    .parameter("secret", serde_json::json!("token")),
            )
            .connect("start", "main", "ola", "main");
        Ok(raven_ci_plugin_api::Pipeline::new(flow, "start"))
    }
}

/// The builds a trigger asked for, kept for the test to read.
#[derive(Default)]
struct Asked(std::sync::Mutex<Vec<BuildRequest>>);

impl raven_ci_plugin_api::Builds for Asked {
    fn request(&self, request: BuildRequest) -> Result<u64, raven_ci_plugin_api::CiError> {
        let mut asked = self.0.lock().expect("lock");
        asked.push(request);
        Ok(asked.len() as u64)
    }
}

impl Asked {
    fn count(&self) -> usize {
        self.0.lock().expect("lock").len()
    }

    fn wait_for(&self, count: usize) {
        let began = Instant::now();
        while self.count() < count {
            assert!(began.elapsed() < Duration::from_secs(10), "{} of {count} builds asked", self.count());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn pontos(host: &Arc<raven_ci_plugin_api::testing::FakeHost>) -> ExternalPlugin {
    let plugin = ExternalPlugin::open(&test_plugins().join("pontos")).expect("opens");
    plugin.start(host.clone()).expect("on");
    plugin
}

#[test]
fn every_point_the_plugin_toml_declares_is_offered_and_listed() {
    let plugin = ExternalPlugin::open(&test_plugins().join("pontos")).expect("opens");
    assert_eq!(
        plugin.declared_points(),
        [
            Point::Step,
            Point::Trigger,
            Point::Publisher,
            Point::CredentialKind,
            Point::Routes,
            Point::BuildHook
        ]
    );
    assert_eq!(plugin.triggers()[0].id(), "tique");
    assert_eq!(plugin.publishers()[0].id(), "anexos");
    assert_eq!(plugin.build_hooks()[0].id(), "porteiro");
    assert_eq!(plugin.routes()[0].path, "/api/pontos/eco");
    let kind = &plugin.credential_kinds()[0];
    assert_eq!(kind.caption(), "Token dos pontos");
    let problems =
        raven_ci_plugin_api::check_settings(&kind.fields(), &serde_json::json!({ "usuario": "ana" }))
            .expect_err("the token is required");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(plugin.pid(), None, "declaring points starts nothing");
}

#[test]
fn a_trigger_asks_for_builds_outside_a_call_and_watches_again_when_its_program_falls() {
    if !python() {
        return;
    }
    let host = Arc::new(raven_ci_plugin_api::testing::FakeHost::new("pontos", &[]));
    let plugin = pontos(&host);
    let asked = Arc::new(Asked::default());
    let trigger = plugin.triggers().remove(0);
    trigger.watch("site", &serde_json::json!({ "vezes": 2 }), asked.clone()).expect("watches");
    asked.wait_for(2);
    assert_eq!(asked.0.lock().expect("lock")[0].cause, "tique 0");
    std::thread::sleep(Duration::from_millis(600));
    assert!(plugin.pid().is_some(), "a plugin that watches a job is not stopped for its 300 ms idle");

    let crash = serde_json::json!({ "kind": "diz", "parameters": { "cair": true }, "inputs": {} });
    let error = plugin.call("step/execute", crash).expect_err("it fell");
    assert!(error.to_string().contains("terminou no meio de step/execute, com o código 3"), "{error}");
    asked.wait_for(4);
    assert_eq!(plugin.starts(), 2, "started again on its own");
    assert!(
        host.lines().iter().any(|line| line.contains("o gatilho tique volta a vigiar site")),
        "{:?}",
        host.lines()
    );

    trigger.unwatch("site");
    assert!(plugin.watched().is_empty());
    plugin.stop();
    assert_eq!(plugin.pid(), None);
}

#[test]
fn a_publisher_hands_its_attachments_inline_and_by_a_file_of_the_build() {
    if !python() {
        return;
    }
    let host = Arc::new(raven_ci_plugin_api::testing::FakeHost::new("pontos", &[]));
    let plugin = pontos(&host);
    let workspace = std::env::temp_dir().join(format!("raven-ci-pontos-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("folder");
    let build = raven_ci_plugin_api::BuildInfo { job: "site".into(), number: 7, ..Default::default() };
    let attachments =
        plugin.publishers()[0].publish(&build, &serde_json::json!({}), &workspace).expect("publishes");
    assert_eq!(attachments[0].bytes, b"build 7");
    assert_eq!(attachments[0].summary["n"], 1);
    assert_eq!(attachments[1].bytes.len(), 300_000);
    assert!(!workspace.join(".raven-ci-anexos").exists(), "the file it left is taken and removed");
    plugin.stop();
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn a_hook_route_and_credential_answer_as_the_traits_say() {
    if !python() {
        return;
    }
    let host = Arc::new(raven_ci_plugin_api::testing::FakeHost::new("pontos", &[]));
    let plugin = pontos(&host);
    let build = raven_ci_plugin_api::BuildContext {
        job: "site".into(),
        number: 1,
        workspace: std::env::temp_dir(),
        run: "r".into(),
    };
    let hook = plugin.build_hooks().remove(0);
    assert!(hook.before(&build, &serde_json::json!({})).is_ok());
    let refused = hook.before(&build, &serde_json::json!({ "recusar": true })).expect_err("refused");
    assert_eq!(
        refused,
        raven_ci_plugin_api::CiError::settings("porteiro", "sem espaço"),
        "the same error, whole"
    );

    let route = plugin.routes().remove(0);
    let request = raven_ci_plugin_api::HttpRequest {
        method: "POST".into(),
        path: "/api/pontos/eco".into(),
        body: b"oi".to_vec(),
        actor: Some("ana".into()),
        ..Default::default()
    };
    let response = (route.handler)(&request);
    assert_eq!((response.status, response.body), (201, b"eco de ana: oi".to_vec()));

    let kind = plugin.credential_kinds().remove(0);
    let credential = serde_json::json!({ "usuario": "ana", "token": "tk-9" });
    assert_eq!(kind.secret(&credential), "ana:tk-9");
    assert_eq!(kind.hidden(&credential), ["usuario:tk-9", "tk-9"], "the secret fields are hidden as well");
    plugin.stop();
}

#[test]
fn what_a_step_writes_for_its_build_goes_to_the_log_of_the_build_hidden() {
    if !python() {
        return;
    }
    use raven_ci_plugin_api::{BuildContext, BuildScope, MemoryLog, enter};
    use ravenrs_flows::{Mode, NodeContext, NodeDef, Ports};
    let host = Arc::new(raven_ci_plugin_api::testing::FakeHost::new("pontos", &[]));
    let plugin = pontos(&host);
    let log = Arc::new(MemoryLog::new());
    let context = BuildContext {
        job: "site".into(),
        number: 3,
        workspace: std::env::temp_dir(),
        run: "run-pontos".into(),
    };
    let _guard = enter(BuildScope::new(context, log.clone(), BTreeMap::new()));
    let mut catalog = ravenrs_flows::Catalog::new();
    plugin.steps(&mut catalog);
    let node = NodeDef::new("falar", "diz");
    let inputs =
        Ports::from([("main".to_owned(), vec![ravenrs_flows::Item::new(serde_json::json!({ "n": 1 }))])]);
    let cx = NodeContext::new(&node, &inputs, 0, Mode::Active).in_run(Some(Arc::from("run-pontos")));
    let ports = catalog.get("diz", 1).expect("the step").execute(&cx).expect("runs");
    assert_eq!(ports["main"][0].json["n"], 1);
    assert_eq!(log.lines(), [("falar".to_owned(), "do plugin: ****".to_owned())]);
    plugin.stop();
}
