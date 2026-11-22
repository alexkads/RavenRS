//! The step `compose`: the services of the tests (a database, a queue) brought up for the build,
//! waited for until they are healthy, and brought down at its end.

use std::sync::Arc;
use std::time::{Duration, Instant};

use raven_ci_plugin_api::BuildScope;
use raven_ci_plugin_shell::{ended, watch};
use ravenrs_flows::{NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::End;
use serde_json::Value;

use crate::cleanup::{self, Project, compose_args, project_name};
use crate::{State, texts};

/// The kind of the step.
pub const COMPOSE: &str = "compose";

/// How long `up` waits for health when the step has no `timeout`.
pub const WAIT: Duration = Duration::from_secs(120);

/// How often the wait asks the engine.
const ASK_EVERY: Duration = Duration::from_secs(1);

/// A service as `compose ps --format json` says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    /// Its name in the compose file.
    pub name: String,
    /// `running`, `exited`, `created`…
    pub state: String,
    /// `healthy`, `unhealthy`, `starting`, or empty when it has no healthcheck.
    pub health: String,
    /// The code it ended with, when it ended.
    pub exit_code: i64,
}

/// The services in the output of `compose ps --format json`: one object per line (Compose 2.21
/// on) or one array (before).
pub fn services(output: &str) -> Vec<Service> {
    let read = |value: &Value| Service {
        name: value
            .get("Service")
            .or_else(|| value.get("Name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        state: value.get("State").and_then(Value::as_str).unwrap_or_default().to_lowercase(),
        health: value.get("Health").and_then(Value::as_str).unwrap_or_default().to_lowercase(),
        exit_code: value.get("ExitCode").and_then(Value::as_i64).unwrap_or(0),
    };
    let mut found = Vec::new();
    for line in output.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(items)) => found.extend(items.iter().map(read)),
            Ok(value @ Value::Object(_)) => found.push(read(&value)),
            _ => {}
        }
    }
    found
}

/// Where the services stand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Health {
    /// All up: healthy, or running with no healthcheck, or ended well (a job of setup).
    Ready,
    /// Some still coming up, by name.
    Waiting(Vec<String>),
    /// One will not come up: what it is and why.
    Broken(String),
}

/// Where `services` stand.
pub fn health(services: &[Service]) -> Health {
    if services.is_empty() {
        return Health::Waiting(vec!["nenhum serviço apareceu ainda".to_owned()]);
    }
    let mut waiting = Vec::new();
    for service in services {
        if service.health == "unhealthy" {
            return Health::Broken(format!("o serviço {} ficou unhealthy", service.name));
        }
        match service.state.as_str() {
            "exited" | "dead" if service.exit_code != 0 => {
                return Health::Broken(format!(
                    "o serviço {} parou com o código {}",
                    service.name, service.exit_code
                ));
            }
            "exited" => {}
            "running" if service.health.is_empty() || service.health == "healthy" => {}
            other => waiting.push(format!(
                "{} ({})",
                service.name,
                if service.health.is_empty() { other } else { &service.health }
            )),
        }
    }
    if waiting.is_empty() { Health::Ready } else { Health::Waiting(waiting) }
}

/// The step `compose`: `action` `up` (the default) or `down`, `file` (one or a list),
/// `services` and `wait` (true).
#[derive(Debug)]
pub struct Compose(pub Arc<State>);

impl NodeKind for Compose {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(COMPOSE, 1).title("Serviços do teste").input("main").output("main")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let engine = self.0.engine()?;
        let step = cx.node().id.clone();
        let log = |line: &str| build.log(&step, line);
        let context = build.context();
        let project = Project {
            name: project_name(&context.run),
            files: texts(cx.parameter("file")),
            dir: context.workspace.clone(),
        };
        let base = compose_args(&project.name, &project.files);
        let action = cx.parameter("action").and_then(Value::as_str).unwrap_or("up");
        cleanup::watch(&build, &engine);
        let environment: Vec<(String, String)> =
            build.environment().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let command = |extra: &[String]| {
            let mut spec =
                engine.command(base.iter().cloned().chain(extra.iter().cloned())).dir(&project.dir);
            // Compose fills `${VAR}` of the file from its environment: the build's variables.
            for (name, value) in &environment {
                spec = spec.env(name, value);
            }
            spec
        };
        let limit = cx.node().timeout_ms.map_or(WAIT, Duration::from_millis);
        match action {
            "down" => {
                let args = ["down", "-v", "--remove-orphans"].map(str::to_owned);
                let watched = watch(cx, command(&args).timeout(limit), &log, &|| {})?;
                cleanup::brought_down(&context.run, &project.name);
                ended(watched.end, false).map(|()| BuildScope::pass_on(cx))
            }
            "up" => {
                cleanup::brought_up(&context.run, project.clone());
                let mut args: Vec<String> = ["up", "-d"].map(str::to_owned).into();
                args.extend(texts(cx.parameter("services")));
                let watched = watch(cx, command(&args).timeout(limit), &log, &|| {})?;
                if watched.end != End::Code(0) {
                    return Err(NodeError::new(format!(
                        "o compose up falhou: {}",
                        if watched.last_error().is_empty() { "sem mensagem" } else { watched.last_error() }
                    )));
                }
                if cx.parameter("wait").and_then(Value::as_bool).unwrap_or(true) {
                    wait(cx, &command, limit, &log)?;
                }
                Ok(BuildScope::pass_on(cx))
            }
            other => Err(NodeError::new(format!("compose: action {other:?} não existe; use up ou down"))),
        }
    }
}

/// Waits until the services are ready, at most `limit`, saying in the log which it waits for
/// when that changes.
fn wait(
    cx: &NodeContext<'_>,
    command: &dyn Fn(&[String]) -> ravenrs_process::Spec,
    limit: Duration,
    log: &dyn Fn(&str),
) -> Result<(), NodeError> {
    let until = Instant::now() + limit;
    let args = ["ps", "--all", "--format", "json"].map(str::to_owned);
    let mut said = Vec::new();
    loop {
        cx.check()?;
        let output = command(&args)
            .timeout(crate::engine::QUICK)
            .output()
            .map_err(|e| NodeError::new(format!("compose ps não rodou: {e}")))?;
        match health(&services(&output.stdout.join("\n"))) {
            Health::Ready => {
                log("serviços prontos");
                return Ok(());
            }
            Health::Broken(why) => return Err(NodeError::new(why)),
            Health::Waiting(names) => {
                if Instant::now() >= until {
                    return Err(NodeError::timed_out(format!(
                        "os serviços não ficaram prontos em {} s: {}",
                        limit.as_secs(),
                        names.join(", ")
                    )));
                }
                if names != said {
                    log(&format!("esperando: {}", names.join(", ")));
                    said = names;
                }
            }
        }
        std::thread::sleep(ASK_EVERY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn services_are_read_one_per_line_or_in_an_array() {
        let lines = r#"{"Service":"db","State":"running","Health":"healthy","ExitCode":0}
{"Service":"fila","State":"running","Health":"","ExitCode":0}"#;
        let array = r#"[{"Service":"db","State":"running","Health":"starting","ExitCode":0}]"#;
        assert_eq!(services(lines).len(), 2);
        assert_eq!(health(&services(lines)), Health::Ready);
        assert_eq!(health(&services(array)), Health::Waiting(vec!["db (starting)".into()]));
    }

    #[test]
    fn an_unhealthy_service_or_one_that_stopped_badly_breaks_the_wait() {
        let sick = r#"{"Service":"db","State":"running","Health":"unhealthy","ExitCode":0}"#;
        assert_eq!(health(&services(sick)), Health::Broken("o serviço db ficou unhealthy".into()));
        let fell = r#"{"Service":"migra","State":"exited","Health":"","ExitCode":2}"#;
        assert_eq!(health(&services(fell)), Health::Broken("o serviço migra parou com o código 2".into()));
        let done = r#"{"Service":"migra","State":"exited","Health":"","ExitCode":0}"#;
        assert_eq!(health(&services(done)), Health::Ready, "a job of setup that ended well is ready");
    }

    #[test]
    fn nothing_listed_yet_is_waiting() {
        assert!(matches!(health(&services("")), Health::Waiting(_)));
    }
}
