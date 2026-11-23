//! The plugin explain (#942) on a core on disk, with a fake model: a program that reads the line
//! of JSON and answers a fixed text. It is off from the start; on, it explains a build that failed,
//! keeps the explanation and gives it back without running the program again; it refuses a build
//! that passed, a reader, and a program that is not there.
#![cfg(unix)]
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, CiPlugin, HttpRequest, HttpResponse, ROLE_READ, ROLE_RUN};
use raven_ci_plugin_explain::{ExplainPlugin, PATH};
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use serde_json::{Value, json};

struct Disk(PathBuf);

impl Drop for Disk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fake model: it counts its runs in `vezes`, checks it got the summary, and answers.
fn fake_model(dir: &Path) -> PathBuf {
    let program = dir.join("explicador-falso");
    let script = format!(
        "#!/bin/sh\nread linha\necho x >> {count}\ncase \"$linha\" in *o-erro-que-importa*) ;; *) echo '{{\"erro\":\"sem o resumo\"}}'; exit 0;; esac\necho '{{\"texto\":\"O passo Testa falhou com o código 4: o-erro-que-importa.\",\"modelo\":\"falso\",\"ms\":12}}'\n",
        count = dir.join("vezes").display()
    );
    std::fs::write(&program, script).expect("the fake");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("runnable");
    program
}

fn core(name: &str) -> (Disk, Arc<Ci>, Arc<ExplainPlugin>) {
    let dir = std::env::temp_dir().join(format!("raven-ci-explica-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    let ci = Arc::new(Ci::open(&dir.join("dados")).expect("a core"));
    ci.share();
    let plugin = Arc::new(ExplainPlugin::new());
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(plugin.clone());
    ci.apply_plugins().expect("applied");
    (Disk(dir), ci, plugin)
}

fn ask(plugin: &ExplainPlugin, method: &str, job: &str, number: u64, role: &str) -> (u16, Value) {
    let route = plugin.routes().into_iter().find(|r| r.method == method && r.path == PATH).expect("the route");
    let request = HttpRequest {
        method: method.to_owned(),
        path: PATH.to_owned(),
        query: [("job".to_owned(), job.to_owned()), ("number".to_owned(), number.to_string())].into(),
        actor: Some("ana".to_owned()),
        roles: vec![role.to_owned()],
        ..HttpRequest::default()
    };
    let HttpResponse { status, body, .. } = (route.handler)(&request);
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

fn build(ci: &Ci, job: &str, steps: &str) -> u64 {
    ci.save_job(JobSpec::new(job, "toml", format!("[[stages]]\nname = \"Testa\"\nsteps = {steps}\n"))).expect("job");
    let n = ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    n
}

#[test]
fn it_is_off_from_the_start_and_explains_a_failure_once_when_on() {
    let (disk, ci, plugin) = core("explica");
    let listed = ci.registry().list().expect("plugins");
    let explain = listed.iter().find(|p| p.name == "explain").expect("listed");
    assert!(!explain.enabled, "off from the start");
    // Off, the route says how to switch it on.
    let (status, body) = ask(&plugin, "POST", "x", 1, ROLE_RUN);
    assert_eq!(status, 503, "{body}");
    assert!(body["message"].as_str().unwrap_or_default().contains("plugins ligar explain"), "{body}");

    let program = fake_model(&disk.0);
    ci.configure_plugin("explain", json!({ "programa": program.display().to_string(), "socket": disk.0.join("sem-socket").display().to_string() }))
        .expect("configured");
    assert!(!ci.registry().list().expect("plugins").iter().any(|p| p.name == "explain" && p.enabled), "configuring does not switch it on");
    ci.enable_plugin("explain").expect("on");

    let failed = build(&ci, "quebra", "['echo antes', 'echo o-erro-que-importa >&2; exit 4']");
    let passed = build(&ci, "passa", "['echo tudo-certo']");

    // GET before: none yet.
    assert_eq!(ask(&plugin, "GET", "quebra", failed, ROLE_READ).0, 404);
    // A reader may not ask for one.
    assert_eq!(ask(&plugin, "POST", "quebra", failed, ROLE_READ).0, 403);
    // A build that passed has nothing to explain.
    let (status, body) = ask(&plugin, "POST", "passa", passed, ROLE_RUN);
    assert_eq!(status, 400, "{body}");
    assert!(body["message"].as_str().unwrap_or_default().contains("não falhou"), "{body}");

    let (status, made) = ask(&plugin, "POST", "quebra", failed, ROLE_RUN);
    assert_eq!(status, 200, "{made}");
    assert_eq!(made["guardada"], false);
    assert_eq!(made["modelo"], "falso");
    assert!(made["texto"].as_str().unwrap_or_default().contains("o-erro-que-importa"), "{made}");

    // Again: the one kept, and the program did not run again.
    let (status, kept) = ask(&plugin, "POST", "quebra", failed, ROLE_RUN);
    assert_eq!((status, kept["guardada"].clone()), (200, json!(true)), "{kept}");
    assert_eq!(kept["texto"], made["texto"]);
    let runs = std::fs::read_to_string(disk.0.join("vezes")).expect("the count");
    assert_eq!(runs.lines().count(), 1, "the model ran once");
    // A reader reads the one kept.
    let (status, read) = ask(&plugin, "GET", "quebra", failed, ROLE_READ);
    assert_eq!((status, read["guardada"].clone()), (200, json!(true)));
    // It is an attachment of the build.
    let attachments = ci.attachments("quebra", failed).expect("attachments");
    assert!(attachments.iter().any(|a| a.kind == "explicacao" && a.name == "explicacao.md"), "{attachments:?}");
}

#[test]
fn a_program_that_is_not_there_says_what_to_install() {
    let (disk, ci, plugin) = core("sem-programa");
    ci.configure_plugin("explain", json!({ "programa": disk.0.join("nao-existe").display().to_string(), "socket": disk.0.join("sem-socket").display().to_string() }))
        .expect("configured");
    ci.enable_plugin("explain").expect("on");
    let failed = build(&ci, "quebra", "['exit 1']");
    let (status, body) = ask(&plugin, "POST", "quebra", failed, ROLE_RUN);
    assert_eq!(status, 404, "{body}");
    assert!(body["message"].as_str().unwrap_or_default().contains("instale o raven-ci-explicador"), "{body}");
}
