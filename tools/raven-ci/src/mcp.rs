//! `raven-ci mcp`: the MCP server for sessions of AI (#941), over standard input and output.
//!
//! # Its role
//!
//! A session of AI (Claude Code, Codex) that uses the CI through the command line reads the help,
//! puts the commands together and reads whole logs. Through MCP it calls tools that say what they
//! take and give back the little it needs: the summary of a build instead of its log, a build
//! asked for and waited for in one call, a pipeline checked before it is applied.
//!
//! # How it works
//!
//! - **The server** is `RAVEN_CI_SERVIDOR` (or `http://127.0.0.1:8080`), and **the key**
//!   `RAVEN_CI_TOKEN` or the one `raven-ci ia instalar --chave -` saved: the same as the remote
//!   commands ([`crate::remote`]), whose client every tool uses.
//! - **The tools** (see [`RavenCiServer`]): to read, `resumo_geral`, `build` (the summary of
//!   [`raven_ci_core::summary`]), `log` (only a step, or the lines after one, with a ceiling),
//!   `fila_e_agentes` and `historico`; to act, `rodar` (with `esperar`, which answers the summary
//!   at the end), `cancelar` and `repetir`; for pipelines, with no server, `validar_pipeline`,
//!   `converter_jenkinsfile` and `esquema_pipeline`; and `evidencia`, which downloads what a build
//!   kept, its log and its `resumo.md` to a folder.
//! - **The role of the key decides**: when the server says it is of a reader, the tools that act
//!   are switched off, so they are neither listed nor run. The server refuses them anyway.
//! - **No secret leaves**: the log comes hidden by the server, and there is no tool of credentials.
//! - **A failure** is an answer with `{codigo, saida, mensagem}`, the codes of the command line.
//! - Each call runs the blocking HTTP on a thread of its own, so a long `rodar` with `esperar`
//!   does not stop the server answering.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use raven_ci_plugin_api::CiError;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::remote::{Refused, Server};

/// The tools that act on builds: off for a key of a reader.
pub const ACTING: [&str; 3] = ["rodar", "cancelar", "repetir"];
/// The most lines `log` gives in one answer.
const MOST_LINES: usize = 400;
/// How long `rodar` with `esperar` waits unless told: half an hour.
const WAIT_S: u64 = 30 * 60;

const INSTRUCTIONS: &str = "Raven CI, o servidor de CI/CD. Para saber por que um build falhou, use build (o resumo: \
o passo que falhou com as últimas linhas, os testes quebrados e os diagnósticos do cargo) e só abra log de um passo \
quando o resumo não bastar. Para rodar e esperar, use rodar com esperar = true: a resposta é o resumo do fim. Antes \
de aplicar um .raven-ci.toml, confira com validar_pipeline; esquema_pipeline diz cada campo. evidencia baixa os \
artefatos, o log e o resumo.md de um build para anexar a um PR.";

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct Nothing {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BuildOf {
    /// O job, pelo caminho: apps/site, repo/main.
    job: String,
    /// O número do build; o último, quando não dito.
    numero: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LogOf {
    /// O job.
    job: String,
    /// O número do build.
    numero: u64,
    /// Só as linhas deste passo, pelo nó (o campo failed.step do resumo); todas quando não dito.
    passo: Option<String>,
    /// Só as linhas depois desta (o next_after da resposta anterior), para seguir um build.
    depois: Option<u64>,
    /// Quantas das últimas linhas, até 400; 100 quando não dito.
    limite: Option<usize>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct HistoryOf {
    /// Quantos builds, os mais novos primeiro; 20 quando não dito.
    limite: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RunOf {
    /// O job.
    job: String,
    /// Os valores dos parâmetros, pelo nome.
    parametros: Option<Map<String, Value>>,
    /// Espera o build acabar e responde o resumo dele.
    esperar: Option<bool>,
    /// Quanto esperar, em segundos; 1800 quando não dito.
    prazo_s: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ActOn {
    /// O job.
    job: String,
    /// O número do build.
    numero: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PipelineText {
    /// O texto do pipeline.
    texto: String,
    /// O nome do arquivo, que diz o formato: Jenkinsfile, ou .raven-ci.toml quando não dito.
    arquivo: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct JenkinsfileText {
    /// O texto do Jenkinsfile declarativo.
    texto: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EvidenceOf {
    /// O job.
    job: String,
    /// O número do build.
    numero: u64,
    /// A pasta onde gravar: os artefatos, log.txt e resumo.md.
    pasta: String,
}

/// The MCP server of Raven CI. See the [module](self).
pub struct RavenCiServer {
    router: ToolRouter<Self>,
}

fn answer(result: Result<Value, Refused>) -> CallToolResult {
    match result {
        Ok(value) => CallToolResult::structured(value),
        Err(refused) => CallToolResult::structured_error(refused.to_json()),
    }
}

/// Runs `work`, which speaks HTTP and may wait, on a thread of its own.
async fn blocking(work: impl FnOnce(&Server) -> Result<Value, Refused> + Send + 'static) -> CallToolResult {
    let done = tokio::task::spawn_blocking(move || work(&Server::new(None))).await;
    match done {
        Ok(result) => answer(result),
        Err(e) => CallToolResult::structured_error(Refused::local_json(e.to_string())),
    }
}

/// The last build of `job`.
fn last(server: &Server, job: &str) -> Result<u64, Refused> {
    let builds = server.get("/api/ci/builds", &[("job", job.to_owned())])?;
    let newest =
        builds.as_array().and_then(|list| list.iter().filter_map(|b| b.get("number")?.as_u64()).max());
    newest.ok_or_else(|| Refused::local(format!("o job {job} ainda não tem build")))
}

fn summary(server: &Server, job: &str, number: u64) -> Result<Value, Refused> {
    server.get("/api/ci/build/summary", &[("job", job.to_owned()), ("number", number.to_string())])
}

#[tool_router]
impl RavenCiServer {
    /// The server with every tool, those that act off when `reader` (a key of the role Leitores).
    pub fn new(reader: bool) -> Self {
        let mut router = Self::tool_router();
        if reader {
            for name in ACTING {
                router.disable_route(name);
            }
        }
        Self { router }
    }

    #[tool(
        description = "Cada job com o número e o estado do último build, e os builds na fila.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn resumo_geral(&self, Parameters(_): Parameters<Nothing>) -> CallToolResult {
        blocking(crate::remote::overview).await
    }

    #[tool(
        description = "O resumo de um build, feito para ler pouco: estado, causa, commit, duração, o estágio e o passo que falharam com as últimas 30 linhas, os testes que falharam com a mensagem, os diagnósticos do cargo com arquivo e linha, e o que o build guardou. Menos de 8 KB.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn build(&self, Parameters(input): Parameters<BuildOf>) -> CallToolResult {
        blocking(move |server| {
            let number = match input.numero {
                Some(number) => number,
                None => last(server, &input.job)?,
            };
            summary(server, &input.job, number)
        })
        .await
    }

    #[tool(
        description = "As linhas do log de um build, com os segredos escondidos: de um passo só (o failed.step do resumo), ou depois de uma linha, só as últimas (100, até 400). Use só quando o resumo de build não bastar.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn log(&self, Parameters(input): Parameters<LogOf>) -> CallToolResult {
        blocking(move |server| {
            let lines = server.log(&input.job, input.numero, input.depois.unwrap_or(0))?;
            let wanted: Vec<&Value> = lines
                .iter()
                .filter(|line| {
                    input.passo.as_ref().is_none_or(|step| line.get("step").and_then(Value::as_str) == Some(step))
                })
                .collect();
            let most = input.limite.unwrap_or(100).clamp(1, MOST_LINES);
            let from = wanted.len().saturating_sub(most);
            let next_after = lines.iter().filter_map(|l| l.get("seq")?.as_u64()).max();
            let kept: Vec<Value> = wanted[from..]
                .iter()
                .map(|line| json!({ "seq": line.get("seq"), "step": line.get("step"), "text": line.get("text") }))
                .collect();
            Ok(json!({ "lines": kept, "omitted": from, "next_after": next_after }))
        })
        .await
    }

    #[tool(
        description = "Os builds na fila, com o motivo de cada espera, e os agentes conectados com os rótulos.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn fila_e_agentes(&self, Parameters(_): Parameters<Nothing>) -> CallToolResult {
        blocking(|server| {
            let queue = server.get("/api/ci/queue", &[])?;
            let agents = server.get("/api/ci/agents", &[])?;
            Ok(json!({ "queue": queue, "agents": agents }))
        })
        .await
    }

    #[tool(
        description = "Os últimos builds de todos os jobs, os mais novos primeiro.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn historico(&self, Parameters(input): Parameters<HistoryOf>) -> CallToolResult {
        blocking(move |server| {
            let limit = input.limite.unwrap_or(20).clamp(1, 200);
            server.get("/api/ci/history", &[("limit", limit.to_string())])
        })
        .await
    }

    #[tool(
        description = "Pede um build de um job, com parâmetros. Com esperar = true, espera ele acabar (até prazo_s) e responde o resumo do build; sem, responde o número.",
        annotations(read_only_hint = false, destructive_hint = false, open_world_hint = false)
    )]
    async fn rodar(&self, Parameters(input): Parameters<RunOf>) -> CallToolResult {
        blocking(move |server| {
            let body = json!({ "job": input.job, "parameters": input.parametros.unwrap_or_default() });
            let asked = server.post("/api/ci/run", &body)?;
            let number = asked.get("number").and_then(Value::as_u64).unwrap_or_default();
            if input.esperar != Some(true) {
                return Ok(asked);
            }
            let until = Instant::now() + Duration::from_secs(input.prazo_s.unwrap_or(WAIT_S));
            loop {
                let build = server.build(&input.job, number)?;
                let status = build.pointer("/build/status").and_then(Value::as_str).unwrap_or_default();
                if !matches!(status, "queued" | "running" | "") {
                    return summary(server, &input.job, number);
                }
                if Instant::now() > until {
                    return Ok(json!({ "job": input.job, "number": number, "status": status, "waited": "o prazo passou; o build segue: peça build depois" }));
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        })
        .await
    }

    #[tool(
        description = "Cancela um build na fila ou rodando.",
        annotations(read_only_hint = false, destructive_hint = true, open_world_hint = false)
    )]
    async fn cancelar(&self, Parameters(input): Parameters<ActOn>) -> CallToolResult {
        blocking(move |server| {
            server.post("/api/ci/cancel", &json!({ "job": input.job, "number": input.numero }))
        })
        .await
    }

    #[tool(
        description = "Roda de novo um build, com os mesmos parâmetros; responde o número do novo.",
        annotations(read_only_hint = false, destructive_hint = false, open_world_hint = false)
    )]
    async fn repetir(&self, Parameters(input): Parameters<ActOn>) -> CallToolResult {
        blocking(move |server| {
            server.post("/api/ci/rebuild", &json!({ "job": input.job, "number": input.numero }))
        })
        .await
    }

    #[tool(
        description = "Confere um pipeline (.raven-ci.toml, ou Jenkinsfile pelo nome do arquivo) sem servidor: os erros com a linha, e os passos que nenhum plugin do Raven CI oferece.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn validar_pipeline(&self, Parameters(input): Parameters<PipelineText>) -> CallToolResult {
        let name = input.arquivo.unwrap_or_else(|| raven_ci_plugin_format_toml::FILE_NAME.to_owned());
        answer(
            crate::ia::check(&name, &input.texto)
                .map(|(format, errors)| json!({ "formato": format, "valido": errors.is_empty(), "erros": errors }))
                .map_err(|e: CiError| Refused::local(e.to_string())),
        )
    }

    #[tool(
        description = "Converte um Jenkinsfile declarativo em .raven-ci.toml e lista, com a linha, o que não converteu e por quê.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn converter_jenkinsfile(&self, Parameters(input): Parameters<JenkinsfileText>) -> CallToolResult {
        let conversion = raven_ci_plugin_format_jenkinsfile::convert(&input.texto);
        let notes: Vec<Value> = conversion
            .notes
            .iter()
            .map(|note| json!({ "linha": note.line, "o_que": note.what, "por_que": note.why, "nota": note.to_string() }))
            .collect();
        CallToolResult::structured(
            json!({ "toml": conversion.toml, "completo": conversion.complete(), "notas": notes }),
        )
    }

    #[tool(
        description = "O JSON Schema do .raven-ci.toml: cada campo do arquivo, do estágio e do passo, com o tipo e o que faz.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn esquema_pipeline(&self, Parameters(_): Parameters<Nothing>) -> CallToolResult {
        CallToolResult::structured(raven_ci_plugin_format_toml::schema())
    }

    #[tool(
        description = "Baixa para uma pasta os artefatos de um build, o log inteiro (log.txt) e o resumo (resumo.md), para anexar a um PR; responde os caminhos.",
        annotations(read_only_hint = false, destructive_hint = false, open_world_hint = false)
    )]
    async fn evidencia(&self, Parameters(input): Parameters<EvidenceOf>) -> CallToolResult {
        blocking(move |server| {
            crate::remote::evidencia_json(server, &input.job, input.numero, &PathBuf::from(&input.pasta))
        })
        .await
    }
}

#[tool_handler(router = self.router)]
impl ServerHandler for RavenCiServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("raven-ci", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Whether the key the server is reached with is only of a reader: it has neither the role
/// Executores nor Administradores. A server that does not answer is not a reader: each tool says
/// why it fails.
fn is_reader() -> bool {
    match Server::new(None).get("/api/me", &[]) {
        Ok(me) => !me
            .get("roles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|role| role == raven_ci_plugin_api::ROLE_RUN || role == raven_ci_plugin_api::ROLE_ADMIN),
        Err(_) => false,
    }
}

/// Serves the tools over standard input and output until the client closes them.
pub fn serve() -> Result<i32, CiError> {
    let reader = is_reader();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| CiError::failed(e.to_string()))?;
    runtime.block_on(async move {
        let service = RavenCiServer::new(reader)
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| CiError::failed(format!("o MCP não começou: {e}")))?;
        service.waiting().await.map_err(|e| CiError::failed(e.to_string()))?;
        Ok::<(), CiError>(())
    })?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_sees_no_tool_that_acts() {
        let names = |server: &RavenCiServer| -> Vec<String> {
            server.router.list_all().into_iter().map(|tool| tool.name.to_string()).collect()
        };
        let all = names(&RavenCiServer::new(false));
        for tool in ACTING {
            assert!(all.iter().any(|name| name == tool), "{tool} in {all:?}");
        }
        assert_eq!(all.len(), 12, "{all:?}");
        let read = names(&RavenCiServer::new(true));
        assert_eq!(read.len(), 9, "{read:?}");
        assert!(!read.iter().any(|name| ACTING.contains(&name.as_str())), "{read:?}");
    }
}
