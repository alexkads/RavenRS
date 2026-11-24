//! `raven-ci agentes …`: the build agents the server knows, the tokens to let one in, and the
//! templates that bring agents up when a build needs one (see `raven_ci_core::agents`).

use std::io::Write;

use raven_ci_core::Ci;
use raven_ci_core::agents::AgentTemplate;
use raven_ci_plugin_api::CiError;
use serde_json::Value;

/// What `raven-ci agentes` says when it is asked wrong.
pub const USAGE: &str = "uso: raven-ci agentes <comando>
  listar                  os agentes, com rótulos, executores e se estão conectados
  token NOME [--rotulos a,b] [--executores N]
                          um token de uso único (10 min) para o agente NOME entrar
  remover NOME            o agente deixa de entrar: a chave dele para de funcionar
  modelos                 os modelos que sobem agentes quando um build pede
  modelo NOME --lancador ID --rotulos a,b [--executores N] [--maximo N] [--ocioso MIN]
         [--config JSON]  guarda o modelo; a configuração é a do lançador
  modelo-apagar NOME      apaga o modelo";

fn write(out: &mut dyn Write, text: &str) -> Result<(), CiError> {
    out.write_all(text.as_bytes()).map_err(|e| CiError::failed(e.to_string()))
}

fn flag<'a>(args: &[&'a str], name: &str) -> Result<Option<&'a str>, CiError> {
    match args.iter().position(|a| *a == name) {
        Some(at) => {
            args.get(at + 1).copied().map(Some).ok_or_else(|| CiError::settings(name, "precisa de um valor"))
        }
        None => Ok(None),
    }
}

fn number(args: &[&str], name: &str, default: u64) -> Result<u64, CiError> {
    flag(args, name)?
        .map_or(Ok(default), |n| n.parse().map_err(|_| CiError::settings(name, "tem que ser um número")))
}

fn labels(args: &[&str]) -> Result<Vec<String>, CiError> {
    Ok(flag(args, "--rotulos")?
        .map(|all| all.split(',').map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default())
}

/// Runs `raven-ci agentes ARGS`; the exit code.
pub fn command(ci: &Ci, args: &[&str], json: bool, out: &mut dyn Write) -> Result<i32, CiError> {
    let done = |out: &mut dyn Write, result: Result<String, CiError>| match result {
        Ok(text) => write(out, &text).map(|()| 0),
        Err(why) => write(out, &format!("{why}\n")).map(|()| 2),
    };
    match args {
        ["listar"] => {
            let agents = ci.agents()?;
            if json {
                return done(
                    out,
                    Ok(format!("{}\n", serde_json::to_string_pretty(&agents).unwrap_or_default())),
                );
            }
            let mut text = String::new();
            if agents.is_empty() {
                text.push_str("nenhum agente; crie um com: raven-ci agentes token NOME --rotulos linux\n");
            } else {
                // The state of a connection lives in the memory of the server running (#929).
                text.push_str("(lido da pasta de dados: conectado ou não, só o servidor no ar sabe; use --servidor URL)\n");
            }
            for agent in agents {
                let state = if agent.connected { "conectado" } else { "desconectado" };
                let running = agent.hello.as_ref().map_or(0, |hello| hello.running.len());
                text.push_str(&format!(
                    "{}  {state}  rótulos: {}  executores: {} ({} rodando){}\n",
                    agent.name,
                    agent.labels.join(", "),
                    agent.executors,
                    running,
                    agent.launched_by.map(|by| format!("  subiu por {by}")).unwrap_or_default()
                ));
            }
            done(out, Ok(text))
        }
        ["token", name, rest @ ..] => {
            let executors = u32::try_from(number(rest, "--executores", 1)?).unwrap_or(1);
            done(
                out,
                ci.agent_token(name, &labels(rest)?, executors).map(|token| {
                    format!(
                        "{token}\nvale uma vez, por 10 minutos. No agente:\n  RAVEN_CI_AGENT_TOKEN={token} raven-ci-agent --servidor URL --nome {name}\n"
                    )
                }),
            )
        }
        ["remover", name] => done(out, ci.remove_agent(name).map(|()| format!("{name}: removido\n"))),
        ["modelos"] => {
            let templates = ci.agent_templates()?;
            if json {
                return done(
                    out,
                    Ok(format!("{}\n", serde_json::to_string_pretty(&templates).unwrap_or_default())),
                );
            }
            let text: String = templates
                .iter()
                .map(|t| {
                    format!(
                        "{}  {}  rótulos: {}  executores: {}  até {} agente(s)  ocioso: {} min\n",
                        t.name,
                        t.launcher,
                        t.labels.join(", "),
                        t.executors,
                        t.max_agents,
                        t.idle_ms / 60_000
                    )
                })
                .collect();
            done(out, Ok(if text.is_empty() { "nenhum modelo\n".to_owned() } else { text }))
        }
        ["modelo", name, rest @ ..] => {
            let launcher =
                flag(rest, "--lancador")?.ok_or_else(|| CiError::settings("--lancador", "falta"))?;
            let settings: Value = match flag(rest, "--config")? {
                Some(text) => {
                    serde_json::from_str(text).map_err(|e| CiError::settings("--config", e.to_string()))?
                }
                None => Value::Object(serde_json::Map::new()),
            };
            let template = AgentTemplate {
                name: (*name).to_owned(),
                labels: labels(rest)?,
                launcher: launcher.to_owned(),
                settings,
                executors: u32::try_from(number(rest, "--executores", 1)?).unwrap_or(1),
                max_agents: u32::try_from(number(rest, "--maximo", 1)?).unwrap_or(1),
                idle_ms: number(rest, "--ocioso", 10)? * 60_000,
            };
            done(out, ci.save_agent_template(&template).map(|()| format!("{name}: modelo guardado\n")))
        }
        ["modelo-apagar", name] => {
            done(out, ci.delete_agent_template(name).map(|()| format!("{name}: apagado\n")))
        }
        _ => {
            write(out, &format!("{USAGE}\n"))?;
            Ok(2)
        }
    }
}
