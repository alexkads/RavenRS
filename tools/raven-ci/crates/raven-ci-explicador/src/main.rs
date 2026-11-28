//! `raven-ci-explicador`: why a build of Raven CI failed, written by a model of AI that runs on
//! this machine (#942).
//!
//! # Its role
//!
//! The plugin `explain` of the server hands it the summary of a build and reads two or three
//! sentences back. It runs apart from the server because a model takes gigabytes: by the socket of
//! systemd of its RPM (one instance per request, with its own limit of memory and no network), or as
//! a child of the server on a desktop.
//!
//! # How to use it
//!
//! ```text
//! raven-ci-explicador baixar [--modelo qwen3-1.7b-q4] [--pasta DIR]   # once, with network
//! echo '{"modelo":"qwen3-1.7b-q4","resumo":{…}}' | raven-ci-explicador [--pasta DIR]
//! raven-ci-explicador --versao
//! ```
//!
//! - **The input** is one line of JSON: `modelo` (the id of a model of `ravenrs-ai`) and `resumo`
//!   (the summary of the build). **The output** is one line of JSON: `{"texto", "modelo", "ms"}`, or
//!   `{"erro"}` with what to do. It never downloads while explaining: with no model it says how
//!   to download it, so an instance with no network fails saying why.
//! - **The folder** of the models is `--pasta`, else `RAVEN_CI_EXPLICADOR_PASTA`, else
//!   `~/.raven-ci/modelos`.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use ravenrs_ai::{LocalEngine, ModelFolder, ModelState, model_by_id};
use serde_json::{Value, json};

mod prompt;

/// The model unless the request says.
const MODEL: &str = "qwen3-1.7b-q4";
/// The most tokens of an answer: three short sentences.
const MOST_TOKENS: usize = 260;

/// The value after `flag` in `args`.
fn flag(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|at| args.get(at + 1)).cloned()
}

/// Where the models are.
fn folder(args: &[String]) -> PathBuf {
    flag(args, "--pasta")
        .or_else(|| std::env::var("RAVEN_CI_EXPLICADOR_PASTA").ok().filter(|p| !p.is_empty()))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            home.join(".raven-ci").join("modelos")
        })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("--versao") => {
            println!("raven-ci-explicador {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("baixar") => download(&args),
        _ => {
            let said = explain_input(&args);
            let ok = said.get("erro").is_none();
            println!("{said}");
            let _ = std::io::stdout().flush();
            i32::from(!ok)
        }
    };
    std::process::exit(code);
}

/// `baixar`: the model, with its progress on the errors.
fn download(args: &[String]) -> i32 {
    let id = flag(args, "--modelo").unwrap_or_else(|| MODEL.to_owned());
    let Some(model) = model_by_id(&id) else {
        eprintln!("não há o modelo {id} no catálogo do ravenrs-ai");
        return 2;
    };
    let folder = ModelFolder::new(folder(args));
    let progress = |received: u64, total: u64| {
        eprint!("\r{id}: {} de {} MB", received / 1_048_576, total / 1_048_576);
    };
    match folder.download(&model, &progress, &AtomicBool::new(false)) {
        Ok(()) => {
            eprintln!("\n{id}: baixado");
            0
        }
        Err(e) => {
            eprintln!("\n{id}: não baixou: {e:?}");
            1
        }
    }
}

/// Reads the request on the input and answers it.
fn explain_input(args: &[String]) -> Value {
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() || line.trim().is_empty() {
        return json!({ "erro": "a entrada não trouxe o pedido: uma linha de JSON com modelo e resumo" });
    }
    let request: Value = match serde_json::from_str(line.trim()) {
        Ok(request) => request,
        Err(e) => return json!({ "erro": format!("o pedido não é JSON: {e}") }),
    };
    let id = request.get("modelo").and_then(Value::as_str).unwrap_or(MODEL).to_owned();
    let Some(summary) = request.get("resumo") else {
        return json!({ "erro": "o pedido não trouxe o resumo do build" });
    };
    let Some(model) = model_by_id(&id) else {
        return json!({ "erro": format!("não há o modelo {id} no catálogo do ravenrs-ai") });
    };
    let root = folder(args);
    let models = ModelFolder::new(&root);
    if models.state(&model) != ModelState::Installed {
        return json!({ "erro": format!(
            "o modelo {id} não está baixado em {}: rode raven-ci-explicador baixar --modelo {id} --pasta {}",
            root.display(),
            root.display()
        )});
    }
    let started = Instant::now();
    let mut engine = match LocalEngine::load(&models, &model) {
        Ok(engine) => engine,
        Err(e) => return json!({ "erro": format!("o modelo {id} não carregou: {e:?}") }),
    };
    let mut text = String::new();
    let user = prompt::user(summary);
    if let Err(e) = engine.generate(prompt::SYSTEM, &user, MOST_TOKENS, &mut |piece| {
        text.push_str(piece);
        true
    }) {
        return json!({ "erro": format!("o modelo {id} parou no meio: {e:?}") });
    }
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    json!({ "texto": text.trim(), "modelo": id, "ms": ms })
}
