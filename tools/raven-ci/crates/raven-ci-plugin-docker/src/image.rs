//! The steps `docker-build` and `docker-push`: images built from the build's folder and sent to
//! a registry with a credential of the vault.

use std::sync::Arc;
use std::time::Duration;

use raven_ci_plugin_api::BuildScope;
use raven_ci_plugin_shell::{ended, watch};
use ravenrs_flows::{NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::End;
use serde_json::Value;

use crate::engine::Engine;
use crate::registry::{Config, Login, config_for, server_of};
use crate::{State, texts};

/// The kind of the step that builds an image.
pub const DOCKER_BUILD: &str = "docker-build";
/// The kind of the step that sends images to their registry.
pub const DOCKER_PUSH: &str = "docker-push";

/// The arguments of `docker build` for `tags`, from `context`, with `file`, the names of
/// `build_args` (their values go in the command's environment), `pull` and the labels of the
/// build.
pub fn build_args(
    tags: &[String],
    context: &str,
    file: Option<&str>,
    arg_names: &[String],
    pull: bool,
    labels: &[String],
) -> Vec<String> {
    let mut args = vec!["build".to_owned()];
    for tag in tags {
        args.extend(["-t".to_owned(), tag.clone()]);
    }
    if let Some(file) = file {
        args.extend(["-f".to_owned(), file.to_owned()]);
    }
    for name in arg_names {
        args.extend(["--build-arg".to_owned(), name.clone()]);
    }
    for label in labels {
        args.extend(["--label".to_owned(), label.clone()]);
    }
    if pull {
        args.push("--pull".to_owned());
    }
    args.push(context.to_owned());
    args
}

/// The login of the credential the step names, for the registry of `image`, in a configuration
/// of its own; `None` when it names none.
fn login(
    state: &State,
    cx: &NodeContext<'_>,
    build: &BuildScope,
    image: &str,
) -> Result<Option<Config>, NodeError> {
    let Some(name) = cx.parameter("credential").and_then(Value::as_str).filter(|n| !n.is_empty()) else {
        return Ok(None);
    };
    let secret = state.secret(name)?;
    let login = Login::read(&secret)?;
    let server = if login.server.is_empty() { server_of(image) } else { login.server.clone() };
    build.log(&cx.node().id, &format!("login no registro {server} com a credencial {name}"));
    config_for(build, &login, &server).map(Some)
}

/// Runs `args` of `engine`, with `config` when there is one, for the step of `cx`.
fn run(
    cx: &NodeContext<'_>,
    build: &BuildScope,
    engine: &Engine,
    config: Option<&Config>,
    args: Vec<String>,
    env: &[(String, String)],
) -> Result<(), NodeError> {
    let step = cx.node().id.clone();
    let log = |line: &str| build.log(&step, line);
    let global = config.map(|c| c.global_args(engine)).unwrap_or_default();
    let mut spec = engine.command(global.into_iter().chain(args)).dir(&build.context().workspace);
    if let Some(config) = config {
        spec = config.apply(engine, spec);
    }
    for (name, value) in env {
        spec = spec.env(name, value);
    }
    if let Some(ms) = cx.node().timeout_ms {
        spec = spec.timeout(Duration::from_millis(ms));
    }
    let watched = watch(cx, spec, &log, &|| {})?;
    match watched.end {
        End::Code(0) => Ok(()),
        End::Code(_) if !watched.last_error().is_empty() => {
            Err(NodeError::new(format!("o {} falhou: {}", engine.name(), watched.last_error())))
        }
        end => ended(end, cx.node().retry.is_some()),
    }
}

/// Sends each of `images` to its registry.
fn push(
    cx: &NodeContext<'_>,
    build: &BuildScope,
    engine: &Engine,
    config: Option<&Config>,
    images: &[String],
) -> Result<(), NodeError> {
    for image in images {
        build.log(&cx.node().id, &format!("enviando {image}"));
        run(cx, build, engine, config, vec!["push".to_owned(), image.clone()], &[])?;
    }
    Ok(())
}

/// The step `docker-build`: `tag` (one or a list), `context` (`.`), `file`, `build_args` (a
/// table), `pull`, and `push` with `credential`.
#[derive(Debug)]
pub struct DockerBuild(pub Arc<State>);

impl NodeKind for DockerBuild {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(DOCKER_BUILD, 1).title("Construir imagem").input("main").output("main").requires("tag")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let engine = self.0.engine()?;
        let tags = texts(cx.parameter("tag"));
        let first =
            tags.first().ok_or_else(|| NodeError::new("o passo docker-build precisa de tag"))?.clone();
        let mut names = Vec::new();
        let mut env = Vec::new();
        if let Some(Value::Object(table)) = cx.parameter("build_args") {
            for (name, value) in table {
                names.push(name.clone());
                env.push((name.clone(), value.as_str().map_or_else(|| value.to_string(), str::to_owned)));
            }
        }
        let context = build.context();
        let labels =
            vec![format!("raven-ci.job={}", context.job), format!("raven-ci.number={}", context.number)];
        let args = build_args(
            &tags,
            cx.parameter("context").and_then(Value::as_str).unwrap_or("."),
            cx.parameter("file").and_then(Value::as_str),
            &names,
            cx.parameter("pull").and_then(Value::as_bool).unwrap_or(false),
            &labels,
        );
        let config = login(&self.0, cx, &build, &first)?;
        run(cx, &build, &engine, config.as_ref(), args, &env)?;
        if cx.parameter("push").and_then(Value::as_bool).unwrap_or(false) {
            push(cx, &build, &engine, config.as_ref(), &tags)?;
        }
        Ok(BuildScope::pass_on(cx))
    }
}

/// The step `docker-push`: `image` (one or a list), with `credential`.
#[derive(Debug)]
pub struct DockerPush(pub Arc<State>);

impl NodeKind for DockerPush {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(DOCKER_PUSH, 1).title("Enviar imagem").input("main").output("main").requires("image")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let engine = self.0.engine()?;
        let images = texts(cx.parameter("image"));
        let first =
            images.first().ok_or_else(|| NodeError::new("o passo docker-push precisa de image"))?.clone();
        let config = login(&self.0, cx, &build, &first)?;
        push(cx, &build, &engine, config.as_ref(), &images)?;
        Ok(BuildScope::pass_on(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_names_its_tags_file_args_and_labels_and_ends_with_the_context() {
        let args = build_args(
            &["a:1".into(), "a:latest".into()],
            "app",
            Some("app/Dockerfile"),
            &["VERSAO".into()],
            true,
            &["raven-ci.job=site".into()],
        );
        assert_eq!(
            args,
            [
                "build",
                "-t",
                "a:1",
                "-t",
                "a:latest",
                "-f",
                "app/Dockerfile",
                "--build-arg",
                "VERSAO",
                "--label",
                "raven-ci.job=site",
                "--pull",
                "app"
            ]
        );
    }
}
