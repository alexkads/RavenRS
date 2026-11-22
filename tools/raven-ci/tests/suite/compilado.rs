//! A plugin in Rust with the standard traits, compiled and installed by `raven-ci plugins
//! compilar` and used as one compiled in (#878).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::{Duration, Instant};

use raven_ci_core::{Ci, CiOptions, JobSpec, MemorySecrets, PLUGINS_DIR, PluginState};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, Permission, Point};

/// A folder of data of its own, removed at the end.
struct Data(PathBuf);

impl Data {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("raven-ci-compilado-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_raven-ci"))
            .arg("--dados")
            .arg(&self.0)
            .args(args)
            .output()
            .expect("the binary runs")
    }
}

impl Data {
    /// `raven-ci plugins compilar PASTA --debug`, as the tests compile: in the profile dev, with at
    /// most three jobs, in one folder of build kept in the target of the tests, so the example
    /// compiles once and the next runs reuse it.
    fn compile(&self, folder: &Path) -> Output {
        let jobs = std::env::var("CARGO_BUILD_JOBS").unwrap_or_else(|_| "3".to_owned());
        Command::new(env!("CARGO_BIN_EXE_raven-ci"))
            .arg("--dados")
            .arg(&self.0)
            .args(["plugins", "compilar"])
            .arg(folder)
            .arg("--debug")
            .env("CARGO_BUILD_JOBS", jobs)
            .env("RAVEN_CI_CARGO_TARGET", Path::new(env!("CARGO_TARGET_TMPDIR")).join("exemplo-rust"))
            .output()
            .expect("the binary runs")
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn said(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins/exemplo-rust")
}

fn log_of(ci: &Ci, job: &str, number: u64) -> Vec<String> {
    ci.build_log(job, number, 0).expect("log").into_iter().map(|line| line.text).collect()
}

fn build(ci: &Ci, job: &str) -> raven_ci_core::Build {
    let number = ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    ci.build(job, number).expect("kept")
}

/// Waits until `job` has `count` builds that ended, working the queue meanwhile.
fn wait_for_builds(ci: &Ci, job: &str, count: usize) {
    let began = Instant::now();
    loop {
        ci.work_until_idle("e1").expect("runs");
        let ended = ci.builds(job).expect("builds").iter().filter(|b| b.status.ended()).count();
        if ended >= count {
            return;
        }
        assert!(began.elapsed() < Duration::from_secs(20), "{ended} of {count} builds of {job}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn a_crate_that_asks_for_another_api_is_refused_before_it_builds() {
    let data = Data::new("api-2");
    let crate_dir = data.0.join("plugin-novo");
    std::fs::create_dir_all(crate_dir.join("src")).expect("folder");
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"novo\"\nversion = \"0.1.0\"\n\n[dependencies]\nraven-ci-plugin-api = { version = \"^2\", features = [\"serve\"] }\n",
    )
    .expect("written");
    let out = data.compile(&crate_dir);
    assert_eq!(out.status.code(), Some(2), "{}", said(&out));
    let text = said(&out);
    assert!(text.contains("o plugin pede a raven-ci-plugin-api ^2"), "{text}");
    assert!(
        text.contains(&format!("este Raven CI tem a {}", raven_ci_plugin_api::protocol::API_VERSION)),
        "{text}"
    );
    assert!(!data.0.join(PLUGINS_DIR).join("novo").exists(), "nothing installed");
}

#[test]
fn a_plugin_in_rust_is_compiled_installed_and_used_as_one_compiled_in() {
    if ravenrs_process::resolve("cargo").is_none() {
        eprintln!("pulado: sem cargo nesta máquina, o plugin de exemplo em Rust não compila");
        return;
    }
    let data = Data::new("exemplo");
    let out = data.compile(&example());
    assert!(out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("pontos: step, trigger, publisher, pipeline_format"), "{}", said(&out));
    assert!(!data.0.join(".compilando").exists(), "the folder of build of the tests is kept apart");
    let home = data.0.join(PLUGINS_DIR).join("exemplo-rust");
    assert!(home.join("plugin.toml").is_file());

    // The list of plugins shows it on, with its points.
    let out = data.run(&["plugins", "listar"]);
    let line = said(&out).lines().find(|l| l.starts_with("exemplo-rust")).map(str::to_owned).expect("listed");
    assert!(line.contains("externo") && line.contains("ligado"), "{line}");
    assert!(line.contains("step, trigger, publisher, pipeline_format"), "{line}");

    // A core with the plugin, as the server opens it.
    let secrets = MemorySecrets(BTreeMap::from([("token".to_owned(), "s3cr3t-compilado".to_owned())]));
    let ci = Arc::new(
        Ci::new(CiOptions {
            data_dir: Some(data.0.clone()),
            secrets: Arc::new(secrets),
            ..CiOptions::default()
        })
        .expect("a core"),
    );
    ci.discover_plugins(&data.0.join(PLUGINS_DIR));
    ci.apply_plugins().expect("applies");
    let view = ci.registry().info("exemplo-rust").expect("found");
    assert_eq!(view.state, PluginState::On);
    assert_eq!(view.points, [Point::Step, Point::Trigger, Point::Publisher, Point::PipelineFormat]);

    // Its step runs in a build, reads a secret through the host, and its publisher keeps the file.
    ci.save_job(JobSpec::new("ola", "exemplo-linhas", "saudar quem=Ana segredo=token\npublicar\n"))
        .expect("saved");
    let done = build(&ci, "ola");
    assert_eq!(
        done.status,
        BuildStatus::Succeeded,
        "{:?} {:?}",
        done.reason,
        log_of(&ci, "ola", done.number)
    );
    let log = log_of(&ci, "ola", done.number);
    assert!(log.contains(&"Olá, Ana!".to_owned()), "{log:?}");
    assert!(log.contains(&"o segredo token é ****, com 16 letras".to_owned()), "{log:?}");
    assert!(!log.iter().any(|line| line.contains("s3cr3t-compilado")), "the secret never shows");
    let attachments = ci.attachments("ola", done.number).expect("attachments");
    assert_eq!(attachments.len(), 1, "{attachments:?}");
    assert_eq!(attachments[0].name, "saudacao.txt");
    assert_eq!(ci.attachment_bytes(attachments[0].id).expect("bytes"), b"Ol\xc3\xa1, Ana!\n");
    let audit = ci.store().audit_of("exemplo-rust").expect("audit");
    assert!(audit.iter().any(|a| a.what == "ler o segredo token" && a.granted), "{audit:?}");

    // With the permission taken away, the host refuses it, as it would a plugin compiled in.
    ci.registry().revoke("exemplo-rust", Permission::Secrets, true).expect("revoked");
    let refused = build(&ci, "ola");
    assert_eq!(refused.status, BuildStatus::Failed);
    let why = format!("{:?} {:?}", refused.reason, log_of(&ci, "ola", refused.number));
    assert!(why.contains("não tem a permissão ci.secrets"), "{why}");
    ci.registry().revoke("exemplo-rust", Permission::Secrets, false).expect("given back");

    // Its trigger asks for builds through the host, outside any call of the core.
    ci.save_job(JobSpec::new("tique", "exemplo-linhas", "saudar quem=Bia\n").settings(serde_json::json!({
        "triggers": [{ "id": "exemplo-intervalo", "a_cada_ms": 300, "vezes": 200 }]
    })))
    .expect("saved");
    assert!(ci.watch_triggers().is_empty());
    wait_for_builds(&ci, "tique", 1);

    // A plugin that falls in the middle of a call fails that build, and the core goes on: the
    // program is started again, its trigger watches again and the next builds pass.
    ci.save_job(JobSpec::new("cai", "exemplo-linhas", "saudar cair=sim\n")).expect("saved");
    let fell = build(&ci, "cai");
    assert_eq!(fell.status, BuildStatus::Failed);
    let why = format!("{:?} {:?}", fell.reason, log_of(&ci, "cai", fell.number));
    assert!(why.contains("terminou no meio de step/execute, com o código 3"), "{why}");
    let before = ci.builds("tique").expect("builds").len();
    wait_for_builds(&ci, "tique", before + 1);
    assert!(
        ci.log().lines().iter().any(|line| line.contains("o gatilho exemplo-intervalo volta a vigiar tique")),
        "{:?}",
        ci.log().lines()
    );
    assert_eq!(build(&ci, "ola").status, BuildStatus::Succeeded, "the core goes on");
    ci.unwatch_triggers();
    assert!(Path::new(&home).is_dir());
}
