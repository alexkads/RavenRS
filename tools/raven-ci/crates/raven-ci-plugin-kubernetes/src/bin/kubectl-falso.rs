//! A `kubectl` that answers what a test tells it, for the tests of the plugin `kubernetes` on
//! machines with no cluster.
//!
//! It reads the folder in the variable `KUBECTL_FALSO`:
//!
//! - it adds a line with its arguments, joined by spaces, to `chamadas.txt`;
//! - it answers with the first item of `respostas.json` whose `se` is in its arguments:
//!   `{ "se": "rollout status", "saida": "…", "erro": "…", "codigo": 1 }`. With no item that
//!   matches, it ends with 0 and says nothing.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "it is a command")]

use std::io::Write;

use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let line = args.join(" ");
    let Some(dir) = std::env::var_os("KUBECTL_FALSO").map(std::path::PathBuf::from) else {
        eprintln!("kubectl-falso: falta KUBECTL_FALSO");
        std::process::exit(2);
    };
    if let Ok(mut calls) =
        std::fs::OpenOptions::new().create(true).append(true).open(dir.join("chamadas.txt"))
    {
        let _ = writeln!(calls, "{line}");
    }
    let answers: Vec<Value> = std::fs::read_to_string(dir.join("respostas.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let found = answers
        .iter()
        .find(|answer| answer.get("se").and_then(Value::as_str).is_some_and(|when| line.contains(when)));
    let Some(answer) = found else { return };
    if let Some(out) = answer.get("saida").and_then(Value::as_str) {
        println!("{out}");
    }
    if let Some(err) = answer.get("erro").and_then(Value::as_str) {
        eprintln!("{err}");
    }
    let code = answer.get("codigo").and_then(Value::as_i64).unwrap_or(0);
    std::process::exit(i32::try_from(code).unwrap_or(1));
}
