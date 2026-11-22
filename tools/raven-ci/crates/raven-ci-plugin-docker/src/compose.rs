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

/// A service of a compose project, as `inspect` says it.
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

/// How [`services`] asks `inspect` for each container of a project: its service, state, exit
/// code and health, the last empty when the service has no healthcheck.
pub const INSPECT_FORMAT: &str = "{{index .Config.Labels \"com.docker.compose.service\"}}|{{.State.Status}}|{{.State.ExitCode}}|{{if .State.Health}}{{.State.Health.Status}}{{end}}";

/// The services in the lines `inspect --format` [`INSPECT_FORMAT`] wrote, one per container.
/// `inspect` rather than `compose ps`: only it tells a service with no healthcheck from one
/// whose first check has not come yet.
pub fn services(lines: &[String]) -> Vec<Service> {
    lines
        .iter()
        .filter_map(|line| {
            let mut parts = line.trim().splitn(4, '|');
            let name = parts.next().filter(|n| !n.is_empty())?.to_owned();
            let state = parts.next().unwrap_or_default().to_lowercase();
            let exit_code = parts.next().and_then(|c| c.trim().parse().ok()).unwrap_or(0);
            let health = parts.next().unwrap_or_default().trim().to_lowercase();
            Some(Service { name, state, health, exit_code })
        })
        .collect()
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
                        crate::container::said(&watched.errors)
                    )));
                }
                if cx.parameter("wait").and_then(Value::as_bool).unwrap_or(true) {
                    wait(cx, &engine, &command, limit, &log)?;
                }
                Ok(BuildScope::pass_on(cx))
            }
            other => Err(NodeError::new(format!("compose: action {other:?} não existe; use up ou down"))),
        }
    }
}

/// Waits until the services are ready twice in a row (a service that falls as it starts is
/// caught), at most `limit`, saying in the log which it waits for when that changes.
fn wait(
    cx: &NodeContext<'_>,
    engine: &crate::engine::Engine,
    command: &dyn Fn(&[String]) -> ravenrs_process::Spec,
    limit: Duration,
    log: &dyn Fn(&str),
) -> Result<(), NodeError> {
    let until = Instant::now() + limit;
    let ids = ["ps", "--all", "-q"].map(str::to_owned);
    let mut said = Vec::new();
    let mut ready_before = false;
    loop {
        cx.check()?;
        let listed = command(&ids)
            .timeout(crate::engine::QUICK)
            .output()
            .map_err(|e| NodeError::new(format!("compose ps não rodou: {e}")))?;
        let containers: Vec<String> =
            listed.stdout.iter().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect();
        let found = if containers.is_empty() {
            Vec::new()
        } else {
            let mut args = vec!["inspect".to_owned(), "--format".to_owned(), INSPECT_FORMAT.to_owned()];
            args.extend(containers);
            services(&engine.lines(args))
        };
        match health(&found) {
            Health::Ready if ready_before => {
                log("serviços prontos");
                return Ok(());
            }
            Health::Ready => ready_before = true,
            Health::Broken(why) => return Err(NodeError::new(why)),
            Health::Waiting(names) => {
                ready_before = false;
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

    fn read(lines: &[&str]) -> Vec<Service> {
        services(&lines.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn services_are_read_from_inspect_one_per_container() {
        let both = read(&["db|running|0|healthy", "fila|running|0|"]);
        assert_eq!(both.len(), 2);
        assert_eq!(health(&both), Health::Ready);
        assert_eq!(health(&read(&["db|running|0|starting"])), Health::Waiting(vec!["db (starting)".into()]));
    }

    #[test]
    fn an_unhealthy_service_or_one_that_stopped_badly_breaks_the_wait() {
        assert_eq!(
            health(&read(&["db|running|0|unhealthy"])),
            Health::Broken("o serviço db ficou unhealthy".into())
        );
        assert_eq!(
            health(&read(&["web|exited|127|"])),
            Health::Broken("o serviço web parou com o código 127".into())
        );
        assert_eq!(
            health(&read(&["migra|exited|0|"])),
            Health::Ready,
            "a job of setup that ended well is ready"
        );
    }

    #[test]
    fn nothing_listed_yet_is_waiting() {
        assert!(matches!(health(&read(&[])), Health::Waiting(_)));
        assert!(read(&["", "|running|0|"]).is_empty(), "a line with no service is not one");
    }
}
