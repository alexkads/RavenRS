//! `raven-ci agentes|plugins|usuario … --servidor URL`: what is administered on a server that is
//! running, through its API (#929, #946, #952).
//!
//! # Its role
//!
//! Without a server named, these commands open the folder of data on this disk. That is right for
//! a server that is stopped, and wrong for one that runs: the state of the agents lives in its
//! memory (an agent running a build read from the disk says "desconectado"), a plugin switched on
//! the disk is only seen after a restart, and on a machine where the folder of the server is not
//! the user's own the command makes a second folder of data that nobody reads (#952).
//!
//! # How it works
//!
//! With `--servidor URL` (or `RAVEN_CI_SERVIDOR`) and an administrator's key in `RAVEN_CI_TOKEN`,
//! the words go to the API:
//!
//! | Words | Route |
//! |---|---|
//! | `agentes listar` | `GET /api/ci/agents`, with the state the server knows |
//! | `agentes token NOME [--rotulos a,b] [--executores N]` | `POST /api/ci/agents/token` |
//! | `agentes remover NOME` | `DELETE /api/ci/agents?name=` |
//! | `agentes modelos`, `modelo NOME …`, `modelo-apagar NOME` | `GET`, `POST`, `DELETE /api/ci/agents/templates` |
//! | `plugins listar`, `ligar NOME`, `desligar NOME` | `GET /api/ci/plugins`, `POST /api/ci/plugins/enable|disable` |
//! | `usuario criar NOME [--papel P]` | `POST /api/ci/users`, the password from the input, never an argument |
//!
//! Each says what it did in words, or the answer of the server with `--format json`, and ends with
//! the codes of [`crate::saida::Exit`].

use std::io::{BufRead, Write};

use raven_ci_plugin_api::CiError;
use serde_json::{Value, json};

use crate::cliente::Server;
use crate::saida::{Refused, Said, refuse};

/// Runs `words` on a server when they name one; `None` for the folder of data.
pub fn run(words: &[String], json: bool, out: &mut dyn Write) -> Option<Result<i32, CiError>> {
    let first = words.first().map(String::as_str)?;
    if !matches!(first, "agentes" | "plugins" | "usuario") || !crate::apply::to_server(words) {
        return None;
    }
    // `plugins info` and `plugins compilar` read and write this disk: they stay local.
    if first == "plugins"
        && !matches!(words.get(1).map(String::as_str), Some("listar" | "ligar" | "desligar"))
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
    let said = match first {
        "agentes" => agents(&server, &rest, json, out),
        "plugins" => plugins(&server, &rest, json, out),
        _ => {
            let stdin = std::io::stdin();
            user(&server, &rest, &mut stdin.lock(), json, out)
        }
    };
    Some(match said {
        Ok(code) => Ok(code),
        Err(refused) => refuse(out, &refused, json),
    })
}

fn flag<'a>(args: &[&'a str], name: &str) -> Result<Option<&'a str>, Refused> {
    match args.iter().position(|a| *a == name) {
        Some(at) => args
            .get(at + 1)
            .copied()
            .map(Some)
            .ok_or_else(|| Refused::usage(format!("{name} precisa de um valor"))),
        None => Ok(None),
    }
}

fn number(args: &[&str], name: &str, default: u64) -> Result<u64, Refused> {
    flag(args, name)?.map_or(Ok(default), |n| {
        n.parse().map_err(|_| Refused::usage(format!("{name} tem que ser um número")))
    })
}

fn labels(args: &[&str]) -> Result<Vec<String>, Refused> {
    Ok(flag(args, "--rotulos")?
        .map(|all| all.split(',').map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default())
}

fn said(out: &mut dyn Write, answer: &Value, json: bool, words: &str) -> Said {
    if json {
        writeln!(out, "{answer:#}")?;
    } else {
        writeln!(out, "{words}")?;
    }
    Ok(0)
}

fn agents(server: &Server, args: &[&str], json: bool, out: &mut dyn Write) -> Said {
    match args {
        ["listar"] => {
            let agents = server.get("/api/ci/agents", &[])?;
            if json {
                writeln!(out, "{agents:#}")?;
                return Ok(0);
            }
            let list = agents.as_array().cloned().unwrap_or_default();
            if list.is_empty() {
                writeln!(
                    out,
                    "nenhum agente; crie um com: raven-ci agentes token NOME --rotulos linux --servidor URL"
                )?;
            }
            for agent in list {
                let text =
                    |field: &str| agent.get(field).and_then(Value::as_str).unwrap_or_default().to_owned();
                let state = if agent.get("connected").and_then(Value::as_bool) == Some(true) {
                    "conectado"
                } else {
                    "desconectado"
                };
                let labels: Vec<&str> =
                    agent["labels"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                let running = agent.pointer("/hello/running").and_then(Value::as_array).map_or(0, Vec::len);
                let by =
                    agent.get("launched_by").and_then(Value::as_str).map(|by| format!("  subiu por {by}"));
                writeln!(
                    out,
                    "{}  {state}  rótulos: {}  executores: {} ({running} rodando){}",
                    text("name"),
                    labels.join(", "),
                    agent.get("executors").and_then(Value::as_u64).unwrap_or(0),
                    by.unwrap_or_default()
                )?;
            }
            Ok(0)
        }
        ["token", name, rest @ ..] => {
            let body = json!({
                "name": name,
                "labels": labels(rest)?,
                "executors": number(rest, "--executores", 1)?,
            });
            let answer = server.post("/api/ci/agents/token", &body)?;
            let token = answer.get("token").and_then(Value::as_str).unwrap_or_default();
            said(
                out,
                &answer,
                json,
                &format!(
                    "{token}\nvale uma vez, por 10 minutos. No agente, pela entrada:\n  echo TOKEN | raven-ci-agent --servidor {} --nome {name} --token -",
                    server.base
                ),
            )
        }
        ["remover", name] => {
            let answer = server.delete("/api/ci/agents", &[("name", (*name).to_owned())])?;
            said(out, &answer, json, &format!("{name}: removido"))
        }
        ["modelos"] => {
            let templates = server.get("/api/ci/agents/templates", &[])?;
            if json {
                writeln!(out, "{templates:#}")?;
                return Ok(0);
            }
            let list = templates.as_array().cloned().unwrap_or_default();
            if list.is_empty() {
                writeln!(out, "nenhum modelo")?;
            }
            for t in list {
                let text = |field: &str| t.get(field).and_then(Value::as_str).unwrap_or_default().to_owned();
                let whole = |field: &str| t.get(field).and_then(Value::as_u64).unwrap_or(0);
                let labels: Vec<&str> =
                    t["labels"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                writeln!(
                    out,
                    "{}  {}  rótulos: {}  executores: {}  até {} agente(s)  ocioso: {} min",
                    text("name"),
                    text("launcher"),
                    labels.join(", "),
                    whole("executors"),
                    whole("max_agents"),
                    whole("idle_ms") / 60_000
                )?;
            }
            Ok(0)
        }
        ["modelo", name, rest @ ..] => {
            let launcher = flag(rest, "--lancador")?.ok_or_else(|| Refused::usage("--lancador: falta"))?;
            let settings: Value = match flag(rest, "--config")? {
                Some(text) => {
                    serde_json::from_str(text).map_err(|e| Refused::usage(format!("--config: {e}")))?
                }
                None => json!({}),
            };
            let body = json!({
                "name": name,
                "labels": labels(rest)?,
                "launcher": launcher,
                "settings": settings,
                "executors": number(rest, "--executores", 1)?,
                "max_agents": number(rest, "--maximo", 1)?,
                "idle_ms": number(rest, "--ocioso", 10)? * 60_000,
            });
            let answer = server.post("/api/ci/agents/templates", &body)?;
            said(out, &answer, json, &format!("{name}: modelo guardado"))
        }
        ["modelo-apagar", name] => {
            let answer = server.delete("/api/ci/agents/templates", &[("name", (*name).to_owned())])?;
            said(out, &answer, json, &format!("{name}: apagado"))
        }
        _ => Err(Refused::usage(crate::agentes::USAGE)),
    }
}

fn plugins(server: &Server, args: &[&str], json: bool, out: &mut dyn Write) -> Said {
    match args {
        ["listar"] => {
            let plugins = server.get("/api/ci/plugins", &[])?;
            if json {
                writeln!(out, "{plugins:#}")?;
                return Ok(0);
            }
            for plugin in plugins.as_array().into_iter().flatten() {
                let text =
                    |field: &str| plugin.get(field).and_then(Value::as_str).unwrap_or_default().to_owned();
                let on = if plugin.get("enabled").and_then(Value::as_bool) == Some(true) {
                    "ligado"
                } else {
                    "desligado"
                };
                writeln!(out, "{:<22}{:<11}{}", text("name"), on, text("summary"))?;
            }
            Ok(0)
        }
        [verb @ ("ligar" | "desligar"), name] => {
            let on = *verb == "ligar";
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
        _ => Err(Refused::usage("no servidor: plugins listar | plugins ligar NOME | plugins desligar NOME")),
    }
}

fn user(server: &Server, args: &[&str], input: &mut dyn BufRead, json: bool, out: &mut dyn Write) -> Said {
    let ["criar", name, rest @ ..] = args else {
        return Err(Refused::usage(
            "no servidor: usuario criar NOME [--papel PAPEL], com a senha na entrada",
        ));
    };
    let mut password = String::new();
    input.read_line(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        return Err(Refused::usage(format!(
            "a senha vem na primeira linha da entrada: echo SENHA | raven-ci usuario criar {name} --servidor URL"
        )));
    }
    let mut body = json!({ "name": name, "password": password });
    if let Some(role) = flag(rest, "--papel")? {
        body["role"] = json!(role);
    }
    let answer = server.post("/api/ci/users", &body)?;
    let role = answer.get("role").and_then(Value::as_str).unwrap_or_default();
    said(out, &answer, json, &format!("{name}: criado, com o papel {role}"))
}
