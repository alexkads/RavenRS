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

/// Where the server is unless told.
pub const DEFAULT_SERVER: &str = "http://127.0.0.1:8080";
/// How often a command that follows asks again.
const FOLLOW_EVERY: Duration = Duration::from_millis(500);

/// The names of the commands of this module.
pub const COMMANDS: [&str; 5] = ["rodar", "status", "log", "resumo", "evidencia"];

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// What the words after a command say: the plain words, and the flags.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
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
                    "text" => args.json = false,
                    other => return Err(format!("--format aceita json ou text, não {other}")),
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
    let mut args = match Args::parse(words) {
        Ok(args) => args,
        Err(why) => {
            writeln!(out, "{why}").map_err(failed)?;
            return Ok(2);
        }
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
        ("aplicar", [file]) => aplicar(&server, file, &args, out),
        ("credencial", ["criar", name]) => {
            let mut text = String::new();
            match std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
                Ok(_) => credencial_criar(&server, name, &text, &args, out),
                Err(e) => Err(Refused(format!("a entrada: {e}"))),
            }
        }
        ("credencial", ["listar"]) => credencial_listar(&server, &args, out),
        ("credencial", ["apagar", name]) => credencial_apagar(&server, name, &args, out),
        ("evidencia", [job, number, folder]) => {
            evidencia(&server, job, number, Path::new(folder), &args, out)
        }
        _ => {
            writeln!(out, "{}", crate::cli::USAGE).map_err(failed)?;
            return Ok(2);
        }
    };
    match said {
        Ok(code) => Ok(code),
        Err(Refused(why)) => {
            writeln!(out, "{why}").map_err(failed)?;
            Ok(1)
        }
    }
}

/// A refusal to tell whoever ran the command, in words.
struct Refused(String);

impl From<CiError> for Refused {
    fn from(e: CiError) -> Self {
        Self(e.to_string())
    }
}

impl From<std::io::Error> for Refused {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}

type Said = Result<i32, Refused>;

/// A server of Raven CI, and how to reach it.
struct Server {
    base: String,
    token: Option<String>,
    agent: ureq::Agent,
}

/// `text` as a value of a query: what is not a letter, a digit or `-._~` goes as `%XX`.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

impl Server {
    fn new(server: Option<String>) -> Self {
        let base = server
            .or_else(|| std::env::var("RAVEN_CI_SERVIDOR").ok().filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| DEFAULT_SERVER.to_owned())
            .trim_end_matches('/')
            .to_owned();
        let token = std::env::var("RAVEN_CI_TOKEN").ok().filter(|t| !t.trim().is_empty());
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .new_agent();
        Self { base, token, agent }
    }

    /// `GET` of `path` with `query`: the status, the type and the bytes of the answer.
    fn fetch(&self, path: &str, query: &[(&str, String)]) -> Result<(u16, String, Vec<u8>), Refused> {
        let mut url = format!("{}{path}", self.base);
        for (i, (name, value)) in query.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(&format!("{name}={}", encode(value)));
        }
        let mut request = self.agent.get(&url);
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let answer = request.call().map_err(|e| self.unreachable(&e))?;
        self.read(answer)
    }

    fn unreachable(&self, e: &ureq::Error) -> Refused {
        Refused(format!(
            "o servidor em {} não respondeu ({e}); ele está de pé? (raven-ci servir, ou --servidor URL)",
            self.base
        ))
    }

    fn read(&self, mut answer: ureq::http::Response<ureq::Body>) -> Result<(u16, String, Vec<u8>), Refused> {
        let status = answer.status().as_u16();
        let kind = answer
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let bytes = answer
            .body_mut()
            .with_config()
            .limit(u64::MAX)
            .read_to_vec()
            .map_err(|e| Refused(format!("a resposta de {}: {e}", self.base)))?;
        if status == 401 {
            let why = if self.token.is_some() {
                "o servidor não aceitou o RAVEN_CI_TOKEN; confira a chave"
            } else {
                "o servidor pede login: ponha uma chave em RAVEN_CI_TOKEN (crie com POST /api/auth/keys)"
            };
            return Err(Refused(why.to_owned()));
        }
        if !(200..300).contains(&status) {
            let message = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|value| value.get("message").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).chars().take(200).collect());
            return Err(Refused(format!("o servidor respondeu {status}: {message}")));
        }
        Ok((status, kind, bytes))
    }

    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Refused> {
        let (_, _, bytes) = self.fetch(path, query)?;
        serde_json::from_slice(&bytes).map_err(|e| Refused(format!("a resposta de {path} não é JSON: {e}")))
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, Refused> {
        let mut request =
            self.agent.post(&format!("{}{path}", self.base)).header("Content-Type", "application/json");
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let answer = request.send(body.to_string()).map_err(|e| self.unreachable(&e))?;
        let (_, _, bytes) = self.read(answer)?;
        serde_json::from_slice(&bytes).map_err(|e| Refused(format!("a resposta de {path} não é JSON: {e}")))
    }

    fn delete(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Refused> {
        let mut url = format!("{}{path}", self.base);
        for (i, (name, value)) in query.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(&format!("{name}={}", encode(value)));
        }
        let mut request = self.agent.delete(&url);
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let answer = request.call().map_err(|e| self.unreachable(&e))?;
        let (_, _, bytes) = self.read(answer)?;
        serde_json::from_slice(&bytes).map_err(|e| Refused(format!("a resposta de {path} não é JSON: {e}")))
    }

    /// The build `number` of `job`, with its stages and attachments.
    fn build(&self, job: &str, number: u64) -> Result<Value, Refused> {
        self.get("/api/ci/build", &[("job", job.to_owned()), ("number", number.to_string())])
    }

    /// The lines of the log of the build after `after`.
    fn log(&self, job: &str, number: u64, after: u64) -> Result<Vec<Value>, Refused> {
        let lines = self.get(
            "/api/ci/log",
            &[("job", job.to_owned()), ("number", number.to_string()), ("after", after.to_string())],
        )?;
        Ok(lines.as_array().cloned().unwrap_or_default())
    }
}

/// The status of a build in its JSON (`succeeded`), in words for people.
fn words(status: &str) -> &str {
    match status {
        "queued" => "na fila",
        "running" => "rodando",
        "succeeded" => "passou",
        "failed" => "falhou",
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
    text.parse().map_err(|_| Refused(format!("{text} não é o número de um build")))
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
    Ok(if status == "succeeded" { 0 } else { 1 })
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
        .ok_or_else(|| Refused(format!("o job {job} ainda não tem builds")))
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
    let body = serde_json::to_value(&config).map_err(|e| Refused(e.to_string()))?;
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
        return Err(Refused("diga o tipo: --tipo token, user-password, ssh-key ou arquivo".to_owned()));
    };
    let fields: Value = serde_json::from_str(text).map_err(|_| {
        Refused(
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
fn evidencia(
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
    if args.json {
        write_json(out, &json!({ "artifacts": kept, "log": log_path.display().to_string() }))?;
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
