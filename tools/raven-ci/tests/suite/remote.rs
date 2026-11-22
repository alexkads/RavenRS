//! The commands that speak to a server (#882), run on the binary against `raven-ci servir` on a free
//! port: a build asked with a parameter and followed to its end, one that fails, its status, the
//! summary, its log followed, the evidence downloaded, and what each says when the token is missing
//! or the server is not there.

use std::path::Path;
use std::process::Output;

use raven_ci_core::{Ci, JobSpec};
use serde_json::{Value, json};

use crate::server::{Running, raven, start_with, token};

/// `demo` passes and leaves `relatorio.txt`; `quebra` fails. Both take requests by the API.
fn jobs(dir: &Path) {
    let ci = Ci::open(dir).expect("a core");
    let settings = json!({ "triggers": [{ "id": "api" }] });
    let demo = "artifacts = [\"*.txt\"]\n\n[[parameters]]\nname = \"ALVO\"\ntype = \"text\"\ndefault = \"\"\n\n[[stages]]\nname = \"Gera\"\nsteps = [\"echo relatorio> relatorio.txt\", \"echo feito\"]\n";
    ci.save_job(JobSpec::new("apps/demo", "toml", demo).settings(settings.clone())).expect("demo");
    let quebra = "[[stages]]\nname = \"Quebra\"\nsteps = [\"exit 1\"]\n";
    ci.save_job(JobSpec::new("quebra", "toml", quebra).settings(settings)).expect("quebra");
}

fn remote(running: &Running, token: Option<&str>, args: &[&str]) -> Output {
    let mut command = raven(&running.dir);
    command.args(args).args(["--servidor", &format!("http://{}", running.address)]);
    command.env_remove("RAVEN_CI_SERVIDOR");
    match token {
        Some(token) => command.env("RAVEN_CI_TOKEN", token),
        None => command.env_remove("RAVEN_CI_TOKEN"),
    };
    command.output().expect("runs")
}

fn said(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_build_is_asked_followed_read_summed_up_and_its_evidence_taken() {
    let running = start_with("remoto", jobs);
    let key = token(&running, "ana");

    let ran = remote(&running, Some(&key), &["rodar", "apps/demo", "--param", "ALVO=x", "--esperar"]);
    let text = said(&ran);
    assert_eq!(ran.status.code(), Some(0), "{text}");
    assert!(text.contains("feito") && text.contains("apps/demo #1: passou"), "{text}");

    let broke = remote(&running, Some(&key), &["rodar", "quebra", "--esperar"]);
    assert_eq!(broke.status.code(), Some(1), "a build that fails ends with 1: {}", said(&broke));

    let status = remote(&running, Some(&key), &["status", "apps/demo", "--format", "json"]);
    let build: Value = serde_json::from_slice(&status.stdout).expect("json");
    assert_eq!(build["build"]["status"], "succeeded");
    assert_eq!(build["build"]["parameters"]["ALVO"], "x");
    assert!(said(&remote(&running, Some(&key), &["status", "quebra", "1"])).contains("quebra #1: falhou"));

    let summary = said(&remote(&running, Some(&key), &["resumo"]));
    assert!(summary.contains("apps/demo") && summary.contains("passou"), "{summary}");
    assert!(summary.contains("quebra") && summary.contains("falhou"), "{summary}");
    assert!(summary.contains("na fila: 0"), "{summary}");

    let followed = remote(&running, Some(&key), &["log", "apps/demo", "1", "--seguir", "--format", "json"]);
    let lines: Vec<Value> =
        said(&followed).lines().map(|line| serde_json::from_str(line).expect("a line of json")).collect();
    assert!(lines.iter().any(|line| line["text"] == "feito"), "{lines:?}");

    let folder = running.dir.join("evidencia");
    let taken = remote(&running, Some(&key), &["evidencia", "apps/demo", "1", &folder.display().to_string()]);
    assert_eq!(taken.status.code(), Some(0), "{}", said(&taken));
    let report = std::fs::read_to_string(folder.join("relatorio.txt")).expect("the artifact");
    assert_eq!(report.trim(), "relatorio");
    assert!(std::fs::read_to_string(folder.join("log.txt")).expect("the log").contains("feito"));
}

#[test]
fn no_token_and_no_server_say_what_to_do() {
    let running = start_with("remoto-sem", jobs);
    let refused = remote(&running, None, &["resumo"]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(said(&refused).contains("RAVEN_CI_TOKEN"), "{}", said(&refused));
    let nowhere = raven(&running.dir)
        .args(["status", "apps/demo", "--servidor", "http://127.0.0.1:9"])
        .output()
        .expect("runs");
    assert!(said(&nowhere).contains("http://127.0.0.1:9"), "{}", said(&nowhere));
    let wrong = raven(&running.dir).args(["rodar"]).output().expect("runs");
    assert_eq!(wrong.status.code(), Some(2), "a command with its words missing shows the usage");
}
