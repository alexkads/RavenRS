//! `raven-ci plugins listar|ligar|desligar … --servidor URL`: the plugins of a server that is
//! running, through its API (#946, part of #929).
//!
//! Without a server named, `plugins ligar` writes the folder of data (`crate::cli`), and a server
//! that is running only sees it after it restarts: the 0.1.7 told a person to switch the plugin
//! `explain` on that way, and nothing happened. With `--servidor` (or `RAVEN_CI_SERVIDOR`), the
//! command goes to `POST /api/ci/plugins/enable|disable`, which only Administradores may use, and
//! the server switches it at once.

use std::io::Write;

use raven_ci_plugin_api::CiError;
use serde_json::{Value, json};

use crate::cliente::Server;
use crate::saida::{Refused, Said, refuse};

/// Runs `words` (from `plugins`) on a server when they name one; `None` for the folder of data.
pub fn run(words: &[String], json: bool, out: &mut dyn Write) -> Option<Result<i32, CiError>> {
    let verb = words.get(1).map(String::as_str);
    if words.first().map(String::as_str) != Some("plugins")
        || !matches!(verb, Some("listar" | "ligar" | "desligar"))
        || !crate::apply::to_server(words)
    {
        return None;
    }
    let mut server = None;
    let mut json = json;
    let mut rest = Vec::new();
    let mut given = words[1..].iter();
    while let Some(word) = given.next() {
        match word.as_str() {
            "--servidor" => server = given.next().cloned(),
            "--format" => json = given.next().is_some_and(|f| f == "json"),
            _ => rest.push(word.as_str()),
        }
    }
    let server = Server::new(server);
    let said = match rest.as_slice() {
        ["listar"] => list(&server, json, out),
        [verb @ ("ligar" | "desligar"), name] => switch(&server, verb == &"ligar", name, json, out),
        _ => Err(Refused::usage("no servidor: plugins listar | plugins ligar NOME | plugins desligar NOME")),
    };
    Some(match said {
        Ok(code) => Ok(code),
        Err(refused) => refuse(out, &refused, json),
    })
}

fn list(server: &Server, json: bool, out: &mut dyn Write) -> Said {
    let plugins = server.get("/api/ci/plugins", &[])?;
    if json {
        writeln!(out, "{plugins:#}")?;
        return Ok(0);
    }
    for plugin in plugins.as_array().into_iter().flatten() {
        let text = |field: &str| plugin.get(field).and_then(Value::as_str).unwrap_or_default().to_owned();
        let on =
            if plugin.get("enabled").and_then(Value::as_bool) == Some(true) { "ligado" } else { "desligado" };
        writeln!(out, "{:<22}{:<11}{}", text("name"), on, text("summary"))?;
    }
    Ok(0)
}

fn switch(server: &Server, on: bool, name: &str, json: bool, out: &mut dyn Write) -> Said {
    let path = if on { "/api/ci/plugins/enable" } else { "/api/ci/plugins/disable" };
    let answer = server.post(path, &json!({ "name": name }))?;
    if json {
        writeln!(out, "{answer:#}")?;
        return Ok(0);
    }
    let changed: Vec<&str> = answer
        .get("changed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if changed.is_empty() {
        writeln!(out, "{name}: já estava {}", if on { "ligado" } else { "desligado" })?;
    }
    for change in changed {
        writeln!(out, "{change}")?;
    }
    Ok(0)
}
