//! The step `container`: a script run inside an image, in the build's folder, as the agent.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use raven_ci_plugin_api::BuildScope;
use raven_ci_plugin_shell::{ended, watch};
use ravenrs_flows::{NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::End;
use serde_json::Value;

use crate::cleanup::{self, LABEL};
use crate::engine::{Engine, slug};
use crate::{State, texts};

/// The kind of the step.
pub const CONTAINER: &str = "container";

/// The names a container gets from the environment by value, on the command line, instead of by
/// name: the docker command reads its own `PATH`, `HOME` and `DOCKER_*`, so it cannot be given
/// the build's.
const BY_VALUE: &[&str] = &["PATH", "HOME"];

/// Who the container runs as: the owner of the build's folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunAs {
    /// `--user uid:gid`, for Docker.
    User(u32, u32),
    /// `--userns=keep-id`, for Podman, which maps the user who runs it.
    KeepId,
    /// As the image says: on Windows, where a Linux container has no user of the machine's.
    Image,
}

/// What a `docker run` of a step is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunPlan {
    /// The container's name.
    pub name: String,
    /// The build's run, for the label.
    pub run: String,
    /// The job and the number, for the labels people filter by.
    pub job: String,
    /// The build's number.
    pub number: u64,
    /// The image.
    pub image: String,
    /// The build's folder on this machine.
    pub workspace: String,
    /// Where it is mounted in the container.
    pub mounted: String,
    /// Who it runs as.
    pub run_as: RunAs,
    /// The variables it gets, by name (their values go in the docker command's environment).
    pub env_names: Vec<String>,
    /// The variables it gets by value.
    pub env_values: Vec<(String, String)>,
    /// The network of a compose project of the build it joins.
    pub network: Option<String>,
    /// The `args` of the pipeline's `container`, before the image.
    pub extra: Vec<String>,
    /// The `nice` of the stage.
    pub nice: Option<i64>,
    /// The script.
    pub script: String,
}

impl RunPlan {
    /// The arguments of `docker create` for this plan. The container is made first and started
    /// after (`start -a`): a build cancelled while the engine was still making it would leave it
    /// behind, made after its removal.
    pub fn args(&self) -> Vec<String> {
        let mut args: Vec<String> = ["create", "--rm", "--name", &self.name].map(str::to_owned).into();
        for label in [
            format!("{LABEL}={}", self.run),
            format!("raven-ci.job={}", self.job),
            format!("raven-ci.number={}", self.number),
        ] {
            args.extend(["--label".to_owned(), label]);
        }
        if cfg!(windows) {
            args.extend([
                "--mount".to_owned(),
                format!("type=bind,source={},target={}", self.workspace, self.mounted),
            ]);
        } else {
            args.extend(["-v".to_owned(), format!("{}:{}", self.workspace, self.mounted)]);
        }
        args.extend(["-w".to_owned(), self.mounted.clone()]);
        match self.run_as {
            RunAs::User(uid, gid) => args.extend(["--user".to_owned(), format!("{uid}:{gid}")]),
            RunAs::KeepId => args.push("--userns=keep-id".to_owned()),
            RunAs::Image => {}
        }
        // A user of the machine has no home in the image; /tmp is there and anyone writes in it.
        args.extend(["-e".to_owned(), "HOME=/tmp".to_owned()]);
        for name in &self.env_names {
            args.extend(["-e".to_owned(), name.clone()]);
        }
        for (name, value) in &self.env_values {
            args.extend(["-e".to_owned(), format!("{name}={value}")]);
        }
        if let Some(network) = &self.network {
            args.extend(["--network".to_owned(), network.clone()]);
        }
        args.extend(self.extra.iter().cloned());
        match self.nice {
            Some(level) => {
                args.extend(["--entrypoint".to_owned(), "nice".to_owned(), self.image.clone()]);
                args.extend(["-n".to_owned(), level.to_string(), "sh".to_owned()]);
            }
            None => args.extend(["--entrypoint".to_owned(), "sh".to_owned(), self.image.clone()]),
        }
        args.extend(["-e".to_owned(), "-c".to_owned(), self.script.clone()]);
        args
    }
}

/// Who owns `folder`: the user the agent runs as.
fn run_as(engine: &Engine, folder: &Path) -> RunAs {
    if engine.podman {
        return RunAs::KeepId;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(folder) {
            return RunAs::User(meta.uid(), meta.gid());
        }
    }
    let _ = folder;
    RunAs::Image
}

/// The `args` of a `container`: a list of words, or a text cut at its spaces.
fn extra_args(container: &Value) -> Vec<String> {
    match container.get("args") {
        Some(Value::String(text)) => text.split_whitespace().map(str::to_owned).collect(),
        Some(value) => texts(Some(value)),
        None => Vec::new(),
    }
}

/// Makes sure `image` is on the machine: what is there is used, what is not is pulled, each line
/// to the log. Fails saying "buscar imagem" and the image's name.
pub fn fetch_image(
    cx: &NodeContext<'_>,
    engine: &Engine,
    image: &str,
    log: &dyn Fn(&str),
) -> Result<(), NodeError> {
    let there = engine.ask(["image", "inspect", "--format", "{{.Id}}", image])?;
    if there.end == End::Code(0) {
        log(&format!("buscar imagem: {image} já está na máquina"));
        return Ok(());
    }
    log(&format!("buscar imagem: baixando {image}"));
    let watched = watch(cx, engine.command(["pull", image]), log, &|| {})?;
    if watched.end == End::Code(0) {
        return Ok(());
    }
    Err(NodeError::new(format!(
        "buscar imagem: a imagem {image} não foi encontrada nem baixada: {}",
        said(&watched.errors)
    )))
}

/// The step `container`. See the [crate](crate).
#[derive(Debug)]
pub struct Container(pub Arc<State>);

impl NodeKind for Container {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(CONTAINER, 1)
            .title("Comando num contêiner")
            .input("main")
            .output("main")
            .requires("container")
            .requires("script")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let build = BuildScope::of(cx)?;
        let engine = self.0.engine()?;
        let step = cx.node().id.clone();
        let log = |line: &str| build.log(&step, line);
        let container = cx.parameter("container").cloned().unwrap_or(Value::Null);
        let image = container
            .get("image")
            .and_then(Value::as_str)
            .filter(|image| !image.trim().is_empty())
            .ok_or_else(|| NodeError::new("o passo container precisa de container.image"))?
            .to_owned();
        let script = cx
            .parameter("script")
            .and_then(Value::as_str)
            .ok_or_else(|| NodeError::new("o passo container precisa do script, em texto"))?
            .to_owned();
        cleanup::watch(&build, &engine);
        fetch_image(cx, &engine, &image, &log)?;

        let context = build.context();
        let workspace = context.workspace.display().to_string();
        let mounted = if cfg!(windows) { "/workspace".to_owned() } else { workspace.clone() };
        // What the container gets: the build's variables, then the container's own `env`.
        let mut values: BTreeMap<String, String> = build.environment().clone();
        if cfg!(windows) {
            values.insert("WORKSPACE".to_owned(), mounted.clone());
        }
        if let Some(Value::Object(env)) = container.get("env") {
            for (name, value) in env {
                let text = value.as_str().map_or_else(|| value.to_string(), str::to_owned);
                values.insert(name.clone(), text);
            }
        }
        let (by_value, by_name): (Vec<_>, Vec<_>) = values
            .into_iter()
            .partition(|(name, _)| BY_VALUE.contains(&name.as_str()) || name.starts_with("DOCKER_"));
        let network = cleanup::projects(&context.run).into_iter().find_map(|project| {
            let network = format!("{}_default", project.name);
            (engine.ask(["network", "inspect", &network]).is_ok_and(|o| o.end == End::Code(0)))
                .then_some(network)
        });
        let plan = RunPlan {
            // The build's run and this process tell it apart from a container of another server
            // or of a server before this one on the same engine.
            name: format!(
                "raven-ci-{}-{}-{}-{}-{}-{}",
                slug(&context.job),
                context.number,
                slug(&step),
                short(&context.run),
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ),
            run: context.run.clone(),
            job: context.job.clone(),
            number: context.number,
            image: image.clone(),
            workspace,
            mounted,
            run_as: run_as(&engine, &context.workspace),
            env_names: by_name.iter().map(|(name, _)| name.clone()).collect(),
            env_values: by_value,
            network,
            extra: extra_args(&container),
            nice: cx.parameter("nice").and_then(Value::as_i64),
            script,
        };
        let mut create = engine.command(plan.args()).dir(&context.workspace).timeout(crate::engine::QUICK);
        for (name, value) in &by_name {
            // `-e NAME` takes the value from the command's environment when the container is made.
            create = create.env(name, value);
        }
        let made =
            create.output().map_err(|e| NodeError::new(format!("o {} não rodou: {e}", engine.name())))?;
        if made.end != End::Code(0) {
            return Err(NodeError::new(format!(
                "o {} não criou o contêiner: {}",
                engine.name(),
                said(&made.stderr)
            )));
        }
        let name = plan.name.clone();
        let stop = || {
            // Stopping the command does not stop the container: the engine runs it.
            let _ = engine.ask(["rm", "-f", "-v", &name]);
        };
        let mut spec = engine.command(["start", "-a", &name]).dir(&context.workspace);
        if let Some(ms) = cx.node().timeout_ms {
            spec = spec.timeout(Duration::from_millis(ms));
        }
        log(&format!("rodando em {image}, como {}", describe(plan.run_as)));
        let watched = match watch(cx, spec, &log, &stop) {
            Ok(watched) => watched,
            Err(why) => {
                stop();
                return Err(why);
            }
        };
        if watched.end == End::TimedOut {
            stop();
        }
        match watched.end {
            End::Code(125) => Err(NodeError::new(format!(
                "o {} não rodou o contêiner: {}",
                engine.name(),
                said(&watched.errors)
            ))),
            end => ended(end, cx.node().retry.is_some()).map(|()| BuildScope::pass_on(cx)),
        }
    }
}

/// The last eight letters or digits of `run`: enough to tell two builds apart in a name.
fn short(run: &str) -> String {
    let kept: Vec<char> = slug(run).chars().filter(char::is_ascii_alphanumeric).collect();
    kept[kept.len().saturating_sub(8)..].iter().collect()
}

/// What an engine said when it refused: its line of error (`Error response from daemon: …`),
/// not the advice after it (`Run 'docker run --help'…`).
pub(crate) fn said(lines: &[String]) -> String {
    let lines: Vec<&str> = lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    lines
        .iter()
        .find(|l| l.contains("Error") || l.starts_with("error"))
        .or(lines.last())
        .map(|l| (*l).to_owned())
        .unwrap_or_else(|| "sem mensagem".to_owned())
}

fn describe(run_as: RunAs) -> String {
    match run_as {
        RunAs::User(uid, gid) => format!("o usuário {uid}:{gid} do agente"),
        RunAs::KeepId => "o usuário do agente (--userns=keep-id)".to_owned(),
        RunAs::Image => "o usuário da imagem".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plan() -> RunPlan {
        RunPlan {
            name: "raven-ci-site-1-s-0".into(),
            run: "run-1".into(),
            job: "site".into(),
            number: 1,
            image: "rust:1".into(),
            workspace: "/b/site/1".into(),
            mounted: "/b/site/1".into(),
            run_as: RunAs::User(501, 20),
            env_names: vec!["TOKEN".into()],
            env_values: vec![("PATH".into(), "/x".into())],
            network: Some("raven-ci-run-1_default".into()),
            extra: vec!["--cpus".into(), "2".into()],
            nice: None,
            script: "cargo --version".into(),
        }
    }

    #[test]
    fn the_run_has_the_label_the_user_the_folder_and_ends_with_the_script() {
        let args = plan().args();
        assert_eq!(&args[..3], ["create", "--rm", "--name"], "made first, started after");
        let has = |pair: [&str; 2]| args.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1]);
        assert!(has(["--label", "raven-ci.build=run-1"]), "{args:?}");
        assert!(has(["--user", "501:20"]), "{args:?}");
        assert!(has(["-w", "/b/site/1"]), "{args:?}");
        assert!(has(["--network", "raven-ci-run-1_default"]), "{args:?}");
        assert!(has(["--entrypoint", "sh"]), "{args:?}");
        assert_eq!(&args[args.len() - 4..], ["rust:1", "-e", "-c", "cargo --version"]);
        let image_at = args.iter().position(|a| a == "rust:1").expect("the image");
        let cpus_at = args.iter().position(|a| a == "--cpus").expect("the extra args");
        assert!(cpus_at < image_at, "the args of the container come before the image");
    }

    #[test]
    fn a_secret_goes_by_its_name_never_by_its_value() {
        let args = plan().args();
        assert!(args.windows(2).any(|w| w[0] == "-e" && w[1] == "TOKEN"), "{args:?}");
        assert!(!args.iter().any(|a| a.starts_with("TOKEN=")), "{args:?}");
    }

    #[test]
    fn podman_keeps_the_id_and_nice_wraps_the_script() {
        let mut plan = plan();
        plan.run_as = RunAs::KeepId;
        plan.nice = Some(5);
        let args = plan.args();
        assert!(args.contains(&"--userns=keep-id".to_owned()), "{args:?}");
        assert!(!args.contains(&"--user".to_owned()), "{args:?}");
        let tail: Vec<&str> = args[args.len() - 7..].iter().map(String::as_str).collect();
        assert_eq!(tail, ["rust:1", "-n", "5", "sh", "-e", "-c", "cargo --version"]);
    }

    #[test]
    fn a_refusal_says_the_error_not_the_advice() {
        let lines = [
            "docker: Error response from daemon: Conflict.",
            "",
            "Run 'docker run --help' for more information",
        ]
        .map(str::to_owned);
        assert_eq!(said(&lines), "docker: Error response from daemon: Conflict.");
        assert_eq!(short("docker-teste-1097806-3"), "10978063");
    }

    #[test]
    fn the_args_of_a_container_are_a_list_or_a_text() {
        assert_eq!(
            extra_args(&json!({ "args": "--cpus 2  --memory 1g" })),
            ["--cpus", "2", "--memory", "1g"]
        );
        assert_eq!(extra_args(&json!({ "args": ["-v", "/a b:/c"] })), ["-v", "/a b:/c"]);
        assert!(extra_args(&json!({})).is_empty());
    }
}
