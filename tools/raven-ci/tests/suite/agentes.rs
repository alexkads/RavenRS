//! `raven-ci agentes` (#893, #894): a token for an agent installed by hand, the templates that
//! bring agents up, and the list.

use std::process::Command;

fn raven(dir: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_raven-ci"))
        .arg("--dados")
        .arg(dir)
        .args(args)
        .output()
        .expect("runs");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn the_command_of_agents_makes_tokens_and_keeps_templates() {
    let dir = std::env::temp_dir().join(format!("raven-ci-agentes-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (ok, said) = raven(&dir, &["agentes", "token", "linux-1", "--rotulos", "linux,x64"]);
    assert!(ok, "{said}");
    assert!(said.starts_with("rva_"), "{said}");
    assert!(said.contains("raven-ci-agent --servidor URL --nome linux-1"), "{said}");
    let (ok, said) = raven(&dir, &["agentes", "token", "um agente"]);
    assert!(!ok, "a name with a space: {said}");

    let (ok, said) = raven(
        &dir,
        &[
            "agentes",
            "modelo",
            "local",
            "--lancador",
            "agent-local",
            "--rotulos",
            "local",
            "--executores",
            "2",
            "--ocioso",
            "5",
        ],
    );
    assert!(ok, "{said}");
    let (_, said) = raven(&dir, &["agentes", "modelos"]);
    assert!(
        said.contains("local  agent-local  rótulos: local  executores: 2  até 1 agente(s)  ocioso: 5 min"),
        "{said}"
    );
    let (_, said) = raven(&dir, &["agentes", "listar"]);
    assert!(said.contains("nenhum agente"), "{said}");
    let (ok, _) = raven(&dir, &["agentes", "modelo-apagar", "local"]);
    assert!(ok);
    let (ok, said) = raven(&dir, &["agentes"]);
    assert!(!ok && said.contains("uso: raven-ci agentes"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}
