//! The commands that speak to a server of Raven CI over its HTTP API (#882): `rodar`, `status`,
//! `log`, `resumo` and `evidencia`; and `aplicar` and `credencial` when a server is named (#922).
//!
//! # Its role
//!
//! The commands of [`cli`](crate::cli) open the core on this machine's disk; these do not. They
//! are for whoever operates a server from a terminal, a script or a session of an AI: ask for a
//! build with its parameters, follow its log, see where everything is and take its evidence, with
//! no screen and no `curl` put together by hand.
//!
//! # How it works
//!
//! - **The server** is `--servidor URL`, else `RAVEN_CI_SERVIDOR`, else `http://127.0.0.1:8080`
//!   (where `raven-ci servir` listens unless told). **The token** is `RAVEN_CI_TOKEN`: a key of a
//!   program (`POST /api/auth/keys`) or the token of a session (`POST /api/auth/token`), sent as
//!   `Authorization: Bearer`.
//! - `rodar` is `POST /api/ci/run` (the plugin `api-trigger`); `status`, `log` and `resumo` read
//!   `/api/ci/build`, `/api/ci/builds`, `/api/ci/log?after=`, `/api/ci/jobs` and `/api/ci/queue`;
//!   `evidencia` lists `/api/ci/artifacts` (the plugin `artifacts`) and downloads each with
//!   `/api/ci/artifact?id=`.
//! - **Following** (`log --seguir`, `rodar --esperar`) asks for the lines after the last one seen
//!   every half second until the build ends, then once more, so no line is lost.
//! - **`--format json`** gives what the server answered; `log --seguir` gives one line of JSON per
//!   line of the log.
//! - **What goes wrong** says what to do: no token, a server that does not answer (and where it
//!   was looked for), or what the server said.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use raven_ci_plugin_api::CiError;
use serde_json::{Map, Value, json};

use crate::cliente::Server;
use crate::saida::refuse;
pub(crate) use crate::saida::{Exit, Refused, Said};

/// How often a command that follows asks again.
const FOLLOW_EVERY: Duration = Duration::from_millis(500);

/// The names of the commands of this module.
pub const COMMANDS: [&str; 6] = ["rodar", "status", "log", "resumo", "evidencia", "explicar"];

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// What the words after a command say: the plain words, and the flags.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Args {
    words: Vec<String>,
    server: Option<String>,
    json: bool,
    params: Vec<(String, String)>,
    priority: Option<i64>,
    wait: bool,
    follow: bool,
    remove: bool,
    kind: Option<String>,
    description: Option<String>,
}

impl Args {
    fn parse(words: &[String]) -> Result<Self, String> {
        let mut args = Self::default();
        let mut rest = words.iter();
        while let Some(word) = rest.next() {
            let mut value =
                |flag: &str| rest.next().cloned().ok_or_else(|| format!("{flag} precisa de um valor"));
            match word.as_str() {
                "--servidor" => args.server = Some(value("--servidor")?),
                "--format" => match value("--format")?.as_str() {
                    "json" => args.json = true,
                    "text" | "md" => args.json = false,
                    other => return Err(format!("--format aceita json, text ou md, não {other}")),
                },
                "--param" => {
                    let pair = value("--param")?;
                    let (name, value) =
                        pair.split_once('=').ok_or_else(|| format!("--param é NOME=VALOR, não {pair}"))?;
                    args.params.push((name.to_owned(), value.to_owned()));
                }
                "--prioridade" => {
                    args.priority =
                        Some(value("--prioridade")?.parse().map_err(|_| "--prioridade é um número")?);
                }
                "--esperar" => args.wait = true,
                "--seguir" => args.follow = true,
                "--remover" => args.remove = true,
                "--tipo" => args.kind = Some(value("--tipo")?),
                "--descricao" => args.description = Some(value("--descricao")?),
                flag if flag.starts_with("--") => return Err(format!("opção desconhecida: {flag}")),
                _ => args.words.push(word.clone()),
            }
        }
        Ok(args)
    }
}

/// Runs `words` when they are one of these commands, writing to `out`; the exit code, or `None`
/// for words that are not these commands. `json` is the `--format json` given before the command.
pub fn run(words: &[String], json: bool, out: &mut dyn Write) -> Option<Result<i32, CiError>> {
    let command = words.first()?;
    // `aplicar` and `credencial` are of the folder of data unless a server is named
    // (`crate::apply`).
    let to_server = matches!(command.as_str(), "aplicar" | "credencial") && crate::apply::to_server(words);
    if !COMMANDS.contains(&command.as_str()) && !to_server {
        return None;
    }
    Some(run_command(command, &words[1..], json, out))
}

fn run_command(command: &str, words: &[String], json: bool, out: &mut dyn Write) -> Result<i32, CiError> {
    let wants_json = json || words.windows(2).any(|pair| pair[0] == "--format" && pair[1] == "json");
    let mut args = match Args::parse(words) {
        Ok(args) => args,
        Err(why) => return refuse(out, &Refused::usage(why), wants_json),
    };
    args.json |= json;
    let server = Server::new(args.server.clone());
    let words: Vec<&str> = args.words.iter().map(String::as_str).collect();
    let said = match (command, words.as_slice()) {
        ("rodar", [job]) => rodar(&server, job, &args, out),
        ("status", [job]) => status(&server, job, None, &args, out),
        ("status", [job, number]) => status(&server, job, Some(number), &args, out),
        ("log", [job, number]) => log(&server, job, number, &args, out),
        ("resumo", []) => resumo(&server, &args, out),
        ("resumo", [job]) => resumo_do_build(&server, job, None, &args, out),
        ("resumo", [job, number]) => resumo_do_build(&server, job, Some(number), &args, out),
        ("explicar", [job]) => explicar(&server, job, None, &args, out),
        ("explicar", [job, number]) => explicar(&server, job, Some(number), &args, out),
        ("aplicar", [file]) => aplicar(&server, file, &args, out),
        ("credencial", ["criar", name]) => {
            let mut text = String::new();
            match std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
                Ok(_) => credencial_criar(&server, name, &text, &args, out),
                Err(e) => Err(Refused::local(format!("a entrada: {e}"))),
            }
        }
        ("credencial", ["listar"]) => credencial_listar(&server, &args, out),
        ("credencial", ["apagar", name]) => credencial_apagar(&server, name, &args, out),
        ("evidencia", [job, number, folder]) => {
            evidencia(&server, job, number, Path::new(folder), &args, out)
        }
        _ => {
            if args.json {
                return refuse(
                    out,
                    &Refused::usage(format!("{command}: palavras que ele não aceita; veja raven-ci ajuda")),
                    true,
                );
            }
            writeln!(out, "{}", crate::cli::USAGE).map_err(failed)?;
            return Ok(Exit::Usage.code());
        }
    };
    match said {
        Ok(code) => Ok(code),
        Err(refused) => refuse(out, &refused, args.json),
    }
}

/// The status of a build in its JSON (`succeeded`), in words for people.
fn words(status: &str) -> &str {
    match status {
        "queued" => "na fila",
        "running" => "rodando",
        "succeeded" => "passou",
        "failed" => "falhou",
        "unstable" => "instável",
        "cancelled" => "cancelado",
        "timed_out" => "passou do prazo",
        // The agent of the build went away (#803, phase 3): it ended, neither failed nor cancelled.
        "interrupted" => "interrompido",
        other => other,
    }
}

fn ended(status: &str) -> bool {
    !matches!(status, "queued" | "running" | "")
}

fn number_of(text: &str) -> Result<u64, Refused> {
    text.parse().map_err(|_| Refused::usage(format!("{text} não é o número de um build")))
}

fn write_json(out: &mut dyn Write, value: &Value) -> Result<(), Refused> {
    writeln!(out, "{value:#}")?;
    Ok(())
}

/// `rodar JOB [--param NOME=VALOR]... [--prioridade N] [--esperar]`.
fn rodar(server: &Server, job: &str, args: &Args, out: &mut dyn Write) -> Said {
    let parameters: Map<String, Value> =
        args.params.iter().map(|(name, value)| (name.clone(), Value::String(value.clone()))).collect();
    let mut body = json!({ "job": job, "parameters": parameters });
    if let Some(priority) = args.priority {
        body["priority"] = json!(priority);
    }
    let answer = server.post("/api/ci/run", &body)?;
    let number = answer.get("number").and_then(Value::as_u64).unwrap_or_default();
    if !args.wait {
        if args.json {
            write_json(out, &answer)?;
        } else {
            writeln!(out, "{job} #{number}: pedido")?;
        }
        return Ok(0);
    }
    if !args.json {
        writeln!(out, "{job} #{number}: pedido; seguindo o log")?;
    }
    let status = follow(server, job, number, args.json, out)?;
    if args.json {
        write_json(out, &server.build(job, number)?)?;
    } else {
        writeln!(out, "{job} #{number}: {}", words(&status))?;
    }
    Ok(exit_of(&status))
}

/// Writes the lines of the log of the build as they come, until it ends; its status.
fn follow(
    server: &Server,
    job: &str,
    number: u64,
    json: bool,
    out: &mut dyn Write,
) -> Result<String, Refused> {
    let mut after = 0;
    loop {
        // The status first: the lines read after it ended are all of them.
        let build = server.build(job, number)?;
        let status = build.pointer("/build/status").and_then(Value::as_str).unwrap_or_default().to_owned();
        for line in server.log(job, number, after)? {
            after = line.get("seq").and_then(Value::as_u64).unwrap_or(after).max(after);
            write_line(out, &line, json)?;
        }
        out.flush()?;
        if ended(&status) {
            return Ok(status);
        }
        std::thread::sleep(FOLLOW_EVERY);
    }
}

fn write_line(out: &mut dyn Write, line: &Value, json: bool) -> Result<(), Refused> {
    if json {
        writeln!(out, "{line}")?;
    } else {
        let step = line.get("step").and_then(Value::as_str).unwrap_or_default();
        let text = line.get("text").and_then(Value::as_str).unwrap_or_default();
        writeln!(out, "[{step}] {text}")?;
    }
    Ok(())
}

/// The number of the last build of `job`.
fn last(server: &Server, job: &str) -> Result<u64, Refused> {
    let builds = server.get("/api/ci/builds", &[("job", job.to_owned())])?;
    builds
        .as_array()
        .and_then(|list| list.iter().filter_map(|b| b.get("number").and_then(Value::as_u64)).max())
        .ok_or_else(|| Refused::new(format!("o job {job} ainda não tem builds")))
}

/// `status JOB [NUMERO]`.
fn status(server: &Server, job: &str, number: Option<&&str>, args: &Args, out: &mut dyn Write) -> Said {
    let number = match number {
        Some(text) => number_of(text)?,
        None => last(server, job)?,
    };
    let answer = server.build(job, number)?;
    if args.json {
        write_json(out, &answer)?;
        return Ok(0);
    }
    let build = &answer["build"];
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    writeln!(out, "{job} #{number}: {}", words(&text(build, "status")))?;
    writeln!(out, "causa:       {}", text(build, "cause"))?;
    if let Some(actor) = build.get("actor").and_then(Value::as_str) {
        writeln!(out, "pedido por:  {actor}")?;
    }
    if let Some(reason) = build.get("reason").and_then(Value::as_str) {
        writeln!(out, "motivo:      {reason}")?;
    }
    if let (Some(start), Some(end)) =
        (build.get("created_ms").and_then(Value::as_i64), build.get("ended_ms").and_then(Value::as_i64))
    {
        writeln!(out, "duração:     {:.1} s", (end - start) as f64 / 1000.0)?;
    }
    if let Some(parameters) = build.get("parameters").and_then(Value::as_object).filter(|p| !p.is_empty()) {
        let list: Vec<String> = parameters.iter().map(|(name, value)| format!("{name}={value}")).collect();
        writeln!(out, "parâmetros:  {}", list.join(", "))?;
    }
    for stage in answer["stages"].as_array().into_iter().flatten() {
        writeln!(out, "  {:<28}{}", text(stage, "name"), text(stage, "status"))?;
    }
    for attachment in answer["attachments"].as_array().into_iter().flatten() {
        let size = attachment.get("size").and_then(Value::as_u64).unwrap_or_default();
        writeln!(out, "  anexo {} {} ({size} bytes)", text(attachment, "kind"), text(attachment, "name"))?;
    }
    Ok(0)
}

/// `log JOB NUMERO [--seguir]`.
fn log(server: &Server, job: &str, number: &str, args: &Args, out: &mut dyn Write) -> Said {
    let number = number_of(number)?;
    if args.follow {
        let status = follow(server, job, number, args.json, out)?;
        if !args.json {
            writeln!(out, "{job} #{number}: {}", words(&status))?;
        }
        return Ok(0);
    }
    let lines = server.log(job, number, 0)?;
    if args.json {
        write_json(out, &Value::Array(lines))?;
    } else {
        for line in &lines {
            write_line(out, line, false)?;
        }
    }
    Ok(0)
}

/// `resumo`.
/// `resumo JOB [NUMERO]`: the summary of a build for programs and sessions of AI (#941), in
/// Markdown, or in JSON with `--format json`. It ends with 1 when the build failed.
fn resumo_do_build(
    server: &Server,
    job: &str,
    number: Option<&str>,
    args: &Args,
    out: &mut dyn Write,
) -> Said {
    let mut query = vec![("job", job.to_owned())];
    if let Some(number) = number {
        query.push(("number", number_of(number)?.to_string()));
    }
    let summary = server.get("/api/ci/build/summary", &query)?;
    if args.json {
        write_json(out, &summary)?;
    } else {
        query.push(("format", "md".to_owned()));
        let (_, _, bytes) = server.fetch("/api/ci/build/summary", &query)?;
        out.write_all(&bytes)?;
    }
    let status = summary.get("status").and_then(Value::as_str).unwrap_or_default();
    Ok(exit_of(status))
}

/// How a command about a build ends by its status: 0 passed or not ended, 7 unstable (#947), 1
/// any other end.
fn exit_of(status: &str) -> i32 {
    match status {
        "unstable" => Exit::Unstable.code(),
        _ if ended(status) && status != "succeeded" => Exit::BuildFailed.code(),
        _ => Exit::Done.code(),
    }
}

/// What `evidencia` downloads, as JSON: for the MCP's `evidencia`.
pub(crate) fn evidencia_json(
    server: &Server,
    job: &str,
    number: u64,
    folder: &Path,
) -> Result<Value, Refused> {
    let args = Args { json: true, ..Args::default() };
    let mut said = Vec::new();
    evidencia(server, job, &number.to_string(), folder, &args, &mut said)?;
    serde_json::from_slice(&said).map_err(|e| Refused::local(e.to_string()))
}

/// Every job with its last build, and the queue: what `resumo` shows and the MCP's
/// `resumo_geral` gives.
pub(crate) fn overview(server: &Server) -> Result<Value, Refused> {
    let jobs = server.get("/api/ci/jobs", &[])?;
    let queue = server.get("/api/ci/queue", &[])?;
    let mut rows = Vec::new();
    for job in jobs.as_array().into_iter().flatten() {
        let name = job.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let builds = server.get("/api/ci/builds", &[("job", name.clone())])?;
        let latest = builds
            .as_array()
            .and_then(|list| list.iter().max_by_key(|b| b.get("number").and_then(Value::as_u64)))
            .cloned();
        rows.push(json!({
            "job": name,
            "last": latest.as_ref().and_then(|b| b.get("number")).cloned().unwrap_or(Value::Null),
            "status": latest.as_ref().and_then(|b| b.get("status")).cloned().unwrap_or(Value::Null),
        }));
    }
    let queue: Vec<Value> = queue
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| json!({ "job": b.get("job"), "number": b.get("number"), "status": b.get("status") }))
        .collect();
    Ok(json!({ "jobs": rows, "queue": queue }))
}

/// `explicar JOB [NUMERO]`: why the build failed, written by the local AI of the plugin `explain`
/// (#942), kept with the build so it is written once.
fn explicar(server: &Server, job: &str, number: Option<&str>, args: &Args, out: &mut dyn Write) -> Said {
    let number = match number {
        Some(number) => number_of(number)?,
        None => last(server, job)?,
    };
    let path = format!("/api/ci/build/explain?job={}&number={number}", crate::cliente::encode(job));
    let answer = server.post(&path, &json!({}))?;
    if args.json {
        write_json(out, &answer)?;
    } else {
        writeln!(out, "{}", answer.get("texto").and_then(Value::as_str).unwrap_or_default().trim())?;
        let model = answer.get("modelo").and_then(Value::as_str).unwrap_or_default();
        let kept = if answer.get("guardada") == Some(&json!(true)) { ", guardada antes" } else { "" };
        writeln!(out, "({model}{kept})")?;
    }
    Ok(Exit::Done.code())
}

fn resumo(server: &Server, args: &Args, out: &mut dyn Write) -> Said {
    let jobs = server.get("/api/ci/jobs", &[])?;
    let queue = server.get("/api/ci/queue", &[])?;
    let mut rows = Vec::new();
    for job in jobs.as_array().into_iter().flatten() {
        let name = job.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let builds = server.get("/api/ci/builds", &[("job", name.clone())])?;
        let latest = builds
            .as_array()
            .and_then(|list| list.iter().max_by_key(|b| b.get("number").and_then(Value::as_u64)))
            .cloned();
        rows.push(json!({
            "job": name,
            "last": latest.as_ref().and_then(|b| b.get("number")).cloned().unwrap_or(Value::Null),
            "status": latest.as_ref().and_then(|b| b.get("status")).cloned().unwrap_or(Value::Null),
        }));
    }
    if args.json {
        write_json(out, &json!({ "jobs": rows, "queue": queue }))?;
        return Ok(0);
    }
    if rows.is_empty() {
        writeln!(out, "nenhum job")?;
    }
    // The widest name and two spaces: the job of a branch (`repo/raven-ci-teste-mundo-real`)
    // passes 32 letters and ran into its number.
    let width =
        rows.iter().map(|row| row["job"].as_str().unwrap_or_default().chars().count()).max().unwrap_or(0) + 2;
    for row in &rows {
        let name = row["job"].as_str().unwrap_or_default();
        match row["last"].as_u64() {
            Some(number) => {
                writeln!(
                    out,
                    "{name:<width$}#{number:<6}{}",
                    words(row["status"].as_str().unwrap_or_default())
                )?;
            }
            None => writeln!(out, "{name:<width$}sem builds")?,
        }
    }
    let waiting = queue.as_array().map_or(0, Vec::len);
    writeln!(out, "na fila: {waiting}")?;
    Ok(0)
}

/// `aplicar ARQUIVO [--remover] --servidor URL`: the file read here, the `file` of its jobs with
/// it, and the whole sent to `POST /api/ci/config`.
fn aplicar(server: &Server, file: &str, args: &Args, out: &mut dyn Write) -> Said {
    let config = raven_ci_core::config::ServerConfig::read(Path::new(file))?;
    let body = serde_json::to_value(&config).map_err(|e| Refused::local(e.to_string()))?;
    let path = if args.remove { "/api/ci/config?remover=1" } else { "/api/ci/config" };
    let answer = server.post(path, &body)?;
    let changes: Vec<String> = answer["changes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    crate::apply::write_changes(out, &changes, args.json)?;
    Ok(0)
}

/// `credencial criar NOME --tipo TIPO [--descricao TEXTO]`, with the fields in JSON in `text` (the
/// standard input): never in the words of the command, which anyone on the machine reads in `ps`.
fn credencial_criar(server: &Server, name: &str, text: &str, args: &Args, out: &mut dyn Write) -> Said {
    let Some(kind) = &args.kind else {
        return Err(Refused::usage(
            "diga o tipo: --tipo token, user-password, ssh-key ou arquivo".to_owned(),
        ));
    };
    let fields: Value = serde_json::from_str(text).map_err(|_| {
        Refused::usage(
            "os campos vêm em JSON na entrada: echo '{\"token\": \"...\"}' | raven-ci credencial criar NOME --tipo token --servidor URL"
                .to_owned(),
        )
    })?;
    let mut body = json!({ "name": name, "kind": kind, "fields": fields });
    if let Some(description) = &args.description {
        body["description"] = json!(description);
    }
    let answer = server.post("/api/ci/credentials", &body)?;
    if args.json {
        write_json(out, &answer)?;
    } else {
        let what = if answer["created"] == json!(true) { "guardada" } else { "girada" };
        writeln!(out, "{name}: {what} no cofre de {}, do tipo {kind}", server.base)?;
    }
    Ok(0)
}

/// `credencial listar`: the credentials of the server, with no value.
fn credencial_listar(server: &Server, args: &Args, out: &mut dyn Write) -> Said {
    let list = server.get("/api/ci/credentials", &[])?;
    if args.json {
        write_json(out, &list)?;
        return Ok(0);
    }
    let all = list.as_array().cloned().unwrap_or_default();
    if all.is_empty() {
        writeln!(out, "nenhuma credencial")?;
    }
    for credential in all {
        let text = |field: &str| credential.get(field).and_then(Value::as_str).unwrap_or_default().to_owned();
        let used: Vec<String> = credential["used_by"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let used = if used.is_empty() { String::new() } else { format!("  usada por: {}", used.join(", ")) };
        writeln!(out, "{:<24}{:<16}{}{used}", text("name"), text("kind"), text("description"))?;
    }
    Ok(0)
}

/// `credencial apagar NOME`: revoked on the server.
fn credencial_apagar(server: &Server, name: &str, args: &Args, out: &mut dyn Write) -> Said {
    let answer = server.delete("/api/ci/credentials", &[("name", name.to_owned())])?;
    if args.json {
        write_json(out, &answer)?;
    } else {
        writeln!(out, "{name}: revogada e apagada em {}", server.base)?;
    }
    Ok(0)
}

/// Where the artifact `name` goes in `folder`: refused when it would leave it.
fn inside(folder: &Path, name: &str) -> Option<PathBuf> {
    let relative = Path::new(name);
    let safe = relative.components().all(|part| matches!(part, Component::Normal(_)));
    (safe && !name.is_empty()).then(|| folder.join(relative))
}

/// `evidencia JOB NUMERO PASTA`.
pub(crate) fn evidencia(
    server: &Server,
    job: &str,
    number: &str,
    folder: &Path,
    args: &Args,
    out: &mut dyn Write,
) -> Said {
    let number = number_of(number)?;
    let list = server.get("/api/ci/artifacts", &[("job", job.to_owned()), ("number", number.to_string())])?;
    std::fs::create_dir_all(folder)?;
    let mut kept = Vec::new();
    for artifact in list.as_array().into_iter().flatten() {
        let name = artifact.get("name").and_then(Value::as_str).unwrap_or_default();
        let id = artifact.get("id").and_then(Value::as_i64).unwrap_or_default();
        let Some(path) = inside(folder, name) else {
            writeln!(out, "pulado: {name} sairia da pasta")?;
            continue;
        };
        let (_, _, bytes) = server.fetch("/api/ci/artifact", &[("id", id.to_string())])?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &bytes)?;
        kept.push(json!({ "name": name, "path": path.display().to_string(), "size": bytes.len() }));
    }
    let lines = server.log(job, number, 0)?;
    let text: String = lines
        .iter()
        .map(|line| {
            format!(
                "[{}] {}\n",
                line.get("step").and_then(Value::as_str).unwrap_or_default(),
                line.get("text").and_then(Value::as_str).unwrap_or_default()
            )
        })
        .collect();
    let log_path = folder.join("log.txt");
    std::fs::write(&log_path, text)?;
    // The summary beside them: what goes with a pull request (#941).
    let summary_path = folder.join("resumo.md");
    let query = [("job", job.to_owned()), ("number", number.to_string()), ("format", "md".to_owned())];
    let (_, _, summary) = server.fetch("/api/ci/build/summary", &query)?;
    std::fs::write(&summary_path, summary)?;
    if args.json {
        write_json(
            out,
            &json!({ "artifacts": kept, "log": log_path.display().to_string(), "resumo": summary_path.display().to_string() }),
        )?;
    } else {
        for item in &kept {
            writeln!(out, "{} ({} bytes)", item["path"].as_str().unwrap_or_default(), item["size"])?;
        }
        writeln!(out, "{} ({} linhas)", log_path.display(), lines.len())?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cliente::encode;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn the_flags_go_anywhere_after_the_command() {
        let args = Args::parse(&words(&[
            "demo",
            "--param",
            "ALVO=x=y",
            "--servidor",
            "http://h:1",
            "--format",
            "json",
            "--esperar",
        ]))
        .expect("reads");
        assert_eq!(args.words, ["demo"]);
        assert_eq!(args.params, [("ALVO".to_owned(), "x=y".to_owned())]);
        assert_eq!(args.server.as_deref(), Some("http://h:1"));
        assert!(args.json && args.wait);
        assert!(Args::parse(&words(&["--param", "sem-igual"])).is_err());
        assert!(Args::parse(&words(&["--inventado"])).is_err());
        assert!(run(&words(&["plugins", "listar"]), false, &mut Vec::new()).is_none(), "not one of these");
    }

    #[test]
    fn a_job_with_a_slash_and_a_name_that_would_leave_the_folder() {
        assert_eq!(encode("apps/site ç"), "apps%2Fsite%20%C3%A7");
        let folder = Path::new("/ev");
        assert_eq!(inside(folder, "saida/a.txt"), Some(PathBuf::from("/ev/saida/a.txt")));
        assert_eq!(inside(folder, "../fora.txt"), None);
        assert_eq!(inside(folder, "/etc/passwd"), None);
        assert_eq!(inside(folder, ""), None);
    }
}
