//! Raven CI for sessions of AI (#941), on the binary: the MCP server spoken to over its standard
//! input as Claude Code speaks to it, against `raven-ci servir` on a free port, with the key of an
//! administrator and of a reader; and the schema, the check of a file and the installation, with
//! no server.

use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout, Stdio};

use raven_ci_core::{Ci, JobSpec};
use serde_json::{Value, json};

use crate::server::{Running, create_user, raven, start_with, token};

/// A session of MCP with `raven-ci mcp`.
struct Session {
    child: std::process::Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Session {
    fn start(running: &Running, key: &str) -> Self {
        let mut child = raven(&running.dir)
            .arg("mcp")
            .env("RAVEN_CI_SERVIDOR", format!("http://{}", running.address))
            .env("RAVEN_CI_TOKEN", key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the MCP starts");
        let input = child.stdin.take().expect("stdin");
        let output = BufReader::new(child.stdout.take().expect("stdout"));
        let mut session = Self { child, input, output, next: 1 };
        let hello = session.ask(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "teste", "version": "1" } }),
        );
        assert_eq!(hello["serverInfo"]["name"], "raven-ci", "{hello}");
        session.say("notifications/initialized", json!({}));
        session
    }

    fn say(&mut self, method: &str, params: Value) {
        let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        writeln!(self.input, "{message}").expect("written");
        self.input.flush().expect("flushed");
    }

    fn ask(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.input, "{message}").expect("written");
        self.input.flush().expect("flushed");
        loop {
            let mut line = String::new();
            assert!(self.output.read_line(&mut line).expect("read") > 0, "the MCP closed");
            let answer: Value = serde_json::from_str(&line).expect("json");
            if answer["id"] == id {
                return answer.get("result").cloned().unwrap_or(answer);
            }
        }
    }

    fn tools(&mut self) -> Vec<String> {
        let listed = self.ask("tools/list", json!({}));
        listed["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|t| t["name"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    fn call(&mut self, tool: &str, arguments: Value) -> (bool, Value) {
        let result = self.ask("tools/call", json!({ "name": tool, "arguments": arguments }));
        let error = result["isError"] == true;
        let content = result.get("structuredContent").cloned().unwrap_or(result);
        (error, content)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn jobs(dir: &std::path::Path) {
    let ci = Ci::open(dir).expect("a core");
    let settings = json!({ "triggers": [{ "id": "api" }] });
    ci.save_job(
        JobSpec::new("passa", "toml", "[[stages]]\nname = \"Ok\"\nsteps = [\"echo tudo-certo\"]\n")
            .settings(settings.clone()),
    )
    .expect("passa");
    ci.save_job(
        JobSpec::new("quebra", "toml", "[[stages]]\nname = \"Testa\"\nsteps = [\"echo antes\", \"echo o-erro-que-importa >&2; exit 4\"]\n")
            .settings(settings),
    )
    .expect("quebra");
}

#[test]
fn a_session_of_ai_runs_waits_and_reads_why_a_build_failed_through_mcp() {
    let running = start_with("mcp", jobs);
    let key = token(&running, "ana");
    let mut session = Session::start(&running, &key);
    let tools = session.tools();
    for tool in [
        "resumo_geral",
        "build",
        "log",
        "rodar",
        "cancelar",
        "repetir",
        "explicar",
        "validar_pipeline",
        "converter_jenkinsfile",
        "esquema_pipeline",
        "evidencia",
        "fila_e_agentes",
        "historico",
    ] {
        assert!(tools.iter().any(|t| t == tool), "{tool} in {tools:?}");
    }

    let (error, passed) = session.call("rodar", json!({ "job": "passa", "esperar": true }));
    assert!(!error, "{passed}");
    assert_eq!(passed["status"], "succeeded", "{passed}");

    let (_, failed) = session.call("rodar", json!({ "job": "quebra", "esperar": true }));
    assert_eq!(failed["status"], "failed", "{failed}");
    assert_eq!(failed["failed"]["stage"], "Testa");
    assert!(failed["failed"]["tail"].to_string().contains("o-erro-que-importa"), "{failed}");
    let step = failed["failed"]["step"].as_str().expect("the step").to_owned();

    let (_, again) = session.call("build", json!({ "job": "quebra" }));
    assert_eq!(again["number"], 1, "the last build when no number");
    let (_, lines) = session.call("log", json!({ "job": "quebra", "numero": 1, "passo": step, "limite": 5 }));
    let texts = lines["lines"].to_string();
    assert!(texts.contains("o-erro-que-importa") && !texts.contains("antes"), "only that step: {texts}");

    let (_, overview) = session.call("resumo_geral", json!({}));
    assert_eq!(overview["jobs"].as_array().map(Vec::len), Some(2), "{overview}");

    let (_, checked) =
        session.call("validar_pipeline", json!({ "texto": "[[stages]]\nname = \"a\"\nstep = [\"x\"]\n" }));
    assert_eq!(checked["valido"], false, "{checked}");
    assert!(checked["erros"].to_string().contains("step"), "{checked}");
    let (_, schema) = session.call("esquema_pipeline", json!({}));
    assert!(schema["properties"]["stages"].is_object(), "{schema}");
    let (_, converted) = session.call(
        "converter_jenkinsfile",
        json!({ "texto": "pipeline {\n  agent any\n  stages {\n    stage('A') { steps { sh 'make' } }\n  }\n}\n" }),
    );
    assert_eq!(converted["completo"], true, "{converted}");

    let folder = running.dir.join("evidencia-mcp");
    let (error, evidence) = session
        .call("evidencia", json!({ "job": "quebra", "numero": 1, "pasta": folder.display().to_string() }));
    assert!(!error, "{evidence}");
    let summary = std::fs::read_to_string(folder.join("resumo.md")).expect("the resumo.md");
    assert!(summary.starts_with("# quebra #1: falhou"), "{summary}");

    // The plugin explain is off from the start: the tool says how to switch it on (#942).
    let (error, off) = session.call("explicar", json!({ "job": "quebra" }));
    assert!(error, "{off}");
    assert!(off["mensagem"].as_str().unwrap_or_default().contains("plugins ligar explain"), "{off}");

    let (error, missing) = session.call("build", json!({ "job": "nao/existe", "numero": 1 }));
    assert!(error, "{missing}");
    assert_eq!(missing["codigo"], "recusado", "{missing}");
}

#[test]
fn a_key_of_a_reader_sees_no_tool_that_acts_and_cannot_run_one() {
    let running = start_with("mcp-leitor", jobs);
    create_user(&running.dir, "caio", "Leitores");
    let key = token(&running, "caio");
    let mut session = Session::start(&running, &key);
    let tools = session.tools();
    for acting in ["rodar", "cancelar", "repetir", "explicar"] {
        assert!(!tools.iter().any(|t| t == acting), "{acting} in {tools:?}");
    }
    assert!(tools.iter().any(|t| t == "build"), "{tools:?}");
    let refused = session.ask("tools/call", json!({ "name": "rodar", "arguments": { "job": "passa" } }));
    let said = refused.to_string();
    assert!(refused.get("error").is_some() || refused["isError"] == true, "a reader ran a build: {said}");
}

#[test]
fn the_schema_the_check_and_the_installation_need_no_server() {
    let dir = std::env::temp_dir().join(format!("raven-ci-ia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    let schema = raven(&dir).arg("esquema").output().expect("runs");
    assert!(schema.status.success());
    let schema: Value = serde_json::from_slice(&schema.stdout).expect("json");
    assert_eq!(schema["title"], ".raven-ci.toml");

    let good = dir.join(".raven-ci.toml");
    std::fs::write(&good, "[[stages]]\nname = \"a\"\nsteps = [\"echo x\", { cargo = \"test\" }]\n")
        .expect("written");
    let checked = raven(&dir)
        .args(["validar", &good.display().to_string(), "--format", "json"])
        .output()
        .expect("runs");
    assert_eq!(checked.status.code(), Some(0), "{}", String::from_utf8_lossy(&checked.stdout));
    let bad = dir.join("ruim.raven-ci.toml");
    std::fs::write(
        &bad,
        "[[stages]]\nname = \"a\"\nsteps = [{ uses = \"nada/passo\" }]\ntimeout = \"logo\"\n",
    )
    .expect("written");
    let checked =
        raven(&dir).args(["validar", &bad.display().to_string(), "--format", "json"]).output().expect("runs");
    assert_eq!(checked.status.code(), Some(1));
    let said: Value = serde_json::from_slice(&checked.stdout).expect("json");
    assert_eq!(said["valido"], false);
    assert!(said["erros"].to_string().contains("logo"), "{said}");
    let missing = raven(&dir).args(["validar", "/nao/existe.toml"]).output().expect("runs");
    assert_eq!(missing.status.code(), Some(6));

    let install = raven(&dir)
        .args(["ia", "instalar", "--servidor", "http://ci:8090", "--so-mostrar"])
        .env_remove("RAVEN_CI_TOKEN")
        .output()
        .expect("runs");
    let text = String::from_utf8_lossy(&install.stdout);
    assert_eq!(install.status.code(), Some(0), "{text}");
    assert!(
        text.contains("claude mcp add raven-ci --scope user -e RAVEN_CI_SERVIDOR=http://ci:8090 --"),
        "{text}"
    );
    assert!(text.contains("\"RAVEN_CI_SERVIDOR\": \"http://ci:8090\""), "{text}");
    let guide = raven(&dir).arg("ia").output().expect("runs");
    assert!(String::from_utf8_lossy(&guide.stdout).contains("3 sem_servidor"));
    let _ = std::fs::remove_dir_all(&dir);
}
