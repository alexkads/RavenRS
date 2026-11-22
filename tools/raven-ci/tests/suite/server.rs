//! `raven-ci servir` (#873), run as a service manager runs it: people made on the command line, the
//! server on a free port, `/health` open, everything else behind a token, a build asked through
//! its job and its log read over HTTP, and the server stopping when told.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const PASSWORD: &str = "senha-de-teste-do-servidor";

pub(crate) struct Running {
    pub(crate) child: Child,
    pub(crate) address: String,
    pub(crate) dir: PathBuf,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(crate) fn raven(dir: &PathBuf) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_raven-ci"));
    command.arg("--dados").arg(dir);
    command
}

pub(crate) fn create_user(dir: &PathBuf, name: &str, role: &str) -> String {
    let mut child = raven(dir)
        .args(["usuario", "criar", name, "--papel", role])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runs");
    child.stdin.take().expect("stdin").write_all(format!("{PASSWORD}\n").as_bytes()).expect("written");
    let out = child.wait_with_output().expect("ends");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn start(name: &str) -> Running {
    start_with(name, |_| {})
}

/// The server on a free port, with `prepare` run on its folder of data before it starts (to keep
/// jobs there, as the configuration as code of O-02 will).
pub(crate) fn start_with(name: &str, prepare: impl FnOnce(&std::path::Path)) -> Running {
    let dir = std::env::temp_dir().join(format!("raven-ci-servir-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    prepare(&dir);
    assert!(create_user(&dir, "ana", "Leitores").contains("Administradores"), "the first is an admin");
    let mut child = raven(&dir)
        .args(["servir", "--endereco", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("the server starts");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout")).read_line(&mut line).expect("it says where");
    let address = line
        .split_whitespace()
        .find_map(|word| word.strip_prefix("http://"))
        .map(|at| at.trim_end_matches(',').to_owned())
        .unwrap_or_else(|| panic!("no address in {line:?}"));
    Running { child, address, dir }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).build().new_agent()
}

fn get(running: &Running, path: &str, token: Option<&str>) -> (u16, Value) {
    let mut request = agent().get(&format!("http://{}{path}", running.address));
    if let Some(token) = token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let mut answer = request.call().expect("answered");
    let status = answer.status().as_u16();
    let text = answer.body_mut().read_to_string().unwrap_or_default();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

pub(crate) fn token(running: &Running, name: &str) -> String {
    let mut answer = agent()
        .post(&format!("http://{}/api/auth/token", running.address))
        .header("Content-Type", "application/json")
        .send(json!({ "name": name, "password": PASSWORD }).to_string())
        .expect("answered");
    let body: Value = serde_json::from_str(&answer.body_mut().read_to_string().expect("body")).expect("json");
    body["token"].as_str().unwrap_or_else(|| panic!("no token: {body}")).to_owned()
}

#[test]
fn the_server_answers_health_to_anyone_and_the_rest_only_with_a_token() {
    let running = start("login");
    assert_eq!(get(&running, "/health", None).0, 200);
    assert_eq!(get(&running, "/api/ci/jobs", None).0, 401);
    let token = token(&running, "ana");
    let (status, jobs) = get(&running, "/api/ci/jobs", Some(&token));
    assert_eq!((status, jobs), (200, json!([])));
}

#[test]
fn a_second_person_gets_the_role_asked_and_a_wrong_role_is_refused() {
    let running = start("papeis");
    assert!(create_user(&running.dir, "bia", "Executores").contains("Executores"));
    let refused = raven(&running.dir)
        .args(["usuario", "criar", "caio", "--papel", "Chefes"])
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    assert_eq!(refused.status.code(), Some(2));
    let token = token(&running, "bia");
    assert_eq!(get(&running, "/api/me", Some(&token)).1["roles"], json!(["Executores"]));
}

#[test]
#[cfg(unix)]
fn the_server_stops_when_a_service_manager_tells_it_to() {
    let mut running = start("parar");
    let pid = running.child.id().to_string();
    let killed = Command::new("kill").args(["-TERM", &pid]).status().expect("kill");
    assert!(killed.success());
    let began = Instant::now();
    loop {
        if let Some(status) = running.child.try_wait().expect("waits") {
            assert!(status.success(), "{status:?}");
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(10), "it did not stop");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_credential_is_made_on_the_command_line_listed_only_to_administrators_and_never_with_its_value() {
    let running = start("credenciais");
    assert!(create_user(&running.dir, "bia", "Executores").contains("Executores"));
    let mut child = raven(&running.dir)
        .args(["credencial", "criar", "github", "--tipo", "token"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runs");
    child.stdin.take().expect("stdin").write_all(br#"{"token": "ghp_do_teste_123456"}"#).expect("written");
    let made = child.wait_with_output().expect("ends");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stdout));
    let listed = raven(&running.dir).args(["credencial", "listar"]).output().expect("runs");
    assert!(String::from_utf8_lossy(&listed.stdout).contains("github"));
    let ana = token(&running, "ana");
    let (status, list) = get(&running, "/api/ci/credentials", Some(&ana));
    assert_eq!(status, 200);
    assert_eq!(list[0]["name"], "github");
    assert!(!list.to_string().contains("ghp_do_teste"), "{list}");
    let bia = token(&running, "bia");
    assert_eq!(get(&running, "/api/ci/credentials", Some(&bia)).0, 403);
}
