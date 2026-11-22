//! The step `kubectl`: `apply`, `rollout` or free `args`, with the namespace and the context
//! declared and the kubeconfig of the vault in a file of a moment.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use raven_ci_plugin_api::BuildScope;
use raven_ci_plugin_shell::{Watched, ended, watch};
use ravenrs_flows::{NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::{End, Spec};
use serde_json::Value;

use crate::{State, diagnose, texts};

/// The kind of the step.
pub const KUBECTL: &str = "kubectl";

/// How long `rollout status` waits when the step has no `timeout`.
pub const ROLLOUT_WAIT: Duration = Duration::from_secs(300);

/// The variables of the server `kubectl` gets, besides the basic ones of `ravenrs-process`.
pub const KUBECTL_ENV: &[&str] = &["KUBECONFIG", "HTTPS_PROXY", "HTTP_PROXY", "NO_PROXY"];

/// A kubeconfig of the vault in a file only its owner reads, outside the build's folder, removed
/// when dropped.
#[derive(Debug)]
struct KubeconfigFile {
    dir: PathBuf,
}

impl KubeconfigFile {
    fn write(run: &str, text: &str) -> Result<Self, NodeError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let safe: String = run.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let dir = std::env::temp_dir().join(format!(
            "raven-ci-kube-{}-{safe}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let shown = dir.display().to_string();
        let failed =
            move |e: std::io::Error| NodeError::new(format!("o kubeconfig de um momento em {shown}: {e}"));
        std::fs::create_dir_all(&dir).map_err(&failed)?;
        let file = Self { dir };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file.dir, std::fs::Permissions::from_mode(0o700)).map_err(&failed)?;
        }
        std::fs::write(file.path(), text).map_err(&failed)?;
        Ok(file)
    }

    fn path(&self) -> PathBuf {
        self.dir.join("config")
    }
}

impl Drop for KubeconfigFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What a `kubectl` step does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// `--namespace`.
    pub namespace: String,
    /// `--context`.
    pub context: String,
    /// The files or folders to apply, relative to the build's folder.
    pub apply: Vec<String>,
    /// The resource whose rollout is waited for: `deployment/site`.
    pub rollout: Option<String>,
    /// Free arguments, when it does neither.
    pub args: Vec<String>,
}

impl Plan {
    /// The plan of the parameters `parameter`, or why it cannot run.
    pub fn read(parameter: &dyn Fn(&str) -> Option<Value>) -> Result<Self, NodeError> {
        let required = |name: &str, why: &str| {
            parameter(name)
                .and_then(|v| v.as_str().map(str::trim).map(str::to_owned))
                .filter(|v| !v.is_empty())
                .ok_or_else(|| NodeError::new(format!("o passo kubectl precisa de {name}: {why}")))
        };
        let namespace =
            required("namespace", "declare onde o deploy entra, para não cair no default de quem roda")?;
        let context = required("context", "declare o cluster, para não cair no contexto atual da máquina")?;
        let apply = texts(parameter("apply").as_ref());
        let rollout =
            parameter("rollout").and_then(|v| v.as_str().map(str::to_owned)).filter(|r| !r.is_empty());
        let args = match parameter("args") {
            Some(Value::String(text)) => text.split_whitespace().map(str::to_owned).collect(),
            other => texts(other.as_ref()),
        };
        if apply.is_empty() && rollout.is_none() && args.is_empty() {
            return Err(NodeError::new("o passo kubectl precisa de apply, rollout ou args"));
        }
        if !args.is_empty() && (!apply.is_empty() || rollout.is_some()) {
            return Err(NodeError::new("o passo kubectl diz args sozinho, ou apply e rollout"));
        }
        if args.iter().any(|a| a.starts_with("--namespace") || a.starts_with("--context") || a == "-n") {
            return Err(NodeError::new(
                "o namespace e o contexto vão nos campos namespace e context do passo, não em args",
            ));
        }
        Ok(Self { namespace, context, apply, rollout, args })
    }

    /// The arguments before each command: the kubeconfig, the context and the namespace.
    pub fn base(&self, kubeconfig: Option<&str>) -> Vec<String> {
        let mut base = Vec::new();
        if let Some(path) = kubeconfig {
            base.extend(["--kubeconfig".to_owned(), path.to_owned()]);
        }
        base.extend([
            "--context".to_owned(),
            self.context.clone(),
            "--namespace".to_owned(),
            self.namespace.clone(),
        ]);
        base
    }
}

/// The step `kubectl`. See the [crate](crate).
#[derive(Debug)]
pub struct Kubectl(pub Arc<State>);

struct Call<'a> {
    cx: &'a NodeContext<'a>,
    build: &'a BuildScope,
    program: PathBuf,
    base: Vec<String>,
}

impl Call<'_> {
    fn spec(&self, args: &[String]) -> Spec {
        let mut spec = KUBECTL_ENV
            .iter()
            .fold(Spec::new(&self.program), |spec, name| spec.allow_env(*name))
            .args(self.base.iter().chain(args))
            .dir(&self.build.context().workspace);
        for (name, value) in self.build.environment() {
            spec = spec.env(name, value);
        }
        spec
    }

    /// Runs `args`, each line to the log.
    fn run(&self, args: &[String], limit: Option<Duration>) -> Result<Watched, NodeError> {
        let step = self.cx.node().id.clone();
        let log = |line: &str| self.build.log(&step, line);
        log(&format!("kubectl {}", args.join(" ")));
        let mut spec = self.spec(args);
        if let Some(limit) = limit {
            spec = spec.timeout(limit);
        }
        watch(self.cx, spec, &log, &|| {})
    }

    /// Runs `args` quietly and reads its output as JSON.
    fn json(&self, args: &[&str]) -> Option<Value> {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let output = self.spec(&args).timeout(Duration::from_secs(30)).output().ok()?;
        (output.end == End::Code(0)).then(|| serde_json::from_str(&output.stdout.join("\n")).ok()).flatten()
    }

    /// The failure of `watched`, explained when the server refused.
    fn failure(&self, what: &str, watched: &Watched) -> NodeError {
        if let Some(refused) = diagnose::forbidden(&watched.errors) {
            return NodeError::new(refused);
        }
        let why = watched.last_error();
        match ended(watched.end, self.cx.node().retry.is_some()) {
            Err(e) if why.is_empty() => NodeError { message: format!("{what}: {}", e.message), ..e },
            Err(e) => NodeError { message: format!("{what}: {why}"), ..e },
            Ok(()) => NodeError::new(what.to_owned()),
        }
    }

    /// Why the rollout of `resource` did not end, from its pods.
    fn diagnose(&self, resource: &str) -> Vec<String> {
        let Some(selector) =
            self.json(&["get", resource, "-o", "json"]).as_ref().and_then(diagnose::selector)
        else {
            return Vec::new();
        };
        self.json(&["get", "pods", "-l", &selector, "-o", "json"])
            .map(|pods| diagnose::pods(&pods))
            .unwrap_or_default()
    }
}

impl NodeKind for Kubectl {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(KUBECTL, 1)
            .title("kubectl")
            .input("main")
            .output("main")
            .requires("namespace")
            .requires("context")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let plan = Plan::read(&|name| cx.parameter(name).cloned())?;
        let build = BuildScope::of(cx)?;
        let program = self.0.program()?;
        let file = match cx.parameter("credential").and_then(Value::as_str).filter(|n| !n.is_empty()) {
            Some(name) => {
                let text = self.0.secret(name)?;
                build.log(&cx.node().id, &format!("kubeconfig da credencial {name}"));
                Some(KubeconfigFile::write(&build.context().run, &text)?)
            }
            None => None,
        };
        let path = file.as_ref().map(|f| f.path().display().to_string());
        let call = Call { cx, build: &build, program, base: plan.base(path.as_deref()) };
        let limit = cx.node().timeout_ms.map(Duration::from_millis);

        if !plan.args.is_empty() {
            let watched = call.run(&plan.args, limit)?;
            if watched.end != End::Code(0) {
                return Err(call.failure("kubectl falhou", &watched));
            }
        }
        for target in &plan.apply {
            let watched = call.run(&["apply".to_owned(), "-f".to_owned(), target.clone()], limit)?;
            if watched.end != End::Code(0) {
                return Err(call.failure(&format!("kubectl apply -f {target} falhou"), &watched));
            }
        }
        if let Some(resource) = &plan.rollout {
            let wait = limit.unwrap_or(ROLLOUT_WAIT);
            let args = vec![
                "rollout".to_owned(),
                "status".to_owned(),
                resource.clone(),
                format!("--timeout={}s", wait.as_secs().max(1)),
            ];
            // Its own --timeout says when to give up; the process gets a little more.
            let watched = call.run(&args, Some(wait + Duration::from_secs(30)))?;
            if watched.end != End::Code(0) {
                if let Some(refused) = diagnose::forbidden(&watched.errors) {
                    return Err(NodeError::new(refused));
                }
                let found = call.diagnose(resource);
                if found.is_empty() {
                    return Err(call.failure(&format!("o rollout de {resource} não terminou"), &watched));
                }
                for line in &found {
                    build.log(&cx.node().id, line);
                }
                return Err(NodeError::new(format!(
                    "o rollout de {resource} não terminou: {}",
                    found.join("; ")
                )));
            }
        }
        drop(file);
        Ok(BuildScope::pass_on(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(parameters: Value) -> Result<Plan, NodeError> {
        Plan::read(&|name| parameters.get(name).cloned())
    }

    #[test]
    fn a_step_without_namespace_or_context_is_refused_saying_which() {
        let none = read(json!({ "context": "c", "apply": "k8s/" })).expect_err("refused");
        assert!(none.message.contains("precisa de namespace"), "{}", none.message);
        let none = read(json!({ "namespace": "n", "apply": "k8s/" })).expect_err("refused");
        assert!(none.message.contains("precisa de context"), "{}", none.message);
    }

    #[test]
    fn the_namespace_and_context_go_in_their_fields_not_in_args() {
        let mixed = read(json!({ "namespace": "n", "context": "c", "args": "get pods -n outro" }))
            .expect_err("refused");
        assert!(mixed.message.contains("não em args"), "{}", mixed.message);
    }

    #[test]
    fn apply_and_rollout_go_together_and_the_base_names_the_cluster() {
        let plan = read(json!({ "namespace": "prod", "context": "k3s", "apply": ["a.yml", "b/"], "rollout": "deployment/site" }))
            .expect("a plan");
        assert_eq!(plan.apply, ["a.yml", "b/"]);
        assert_eq!(
            plan.base(Some("/t/config")),
            ["--kubeconfig", "/t/config", "--context", "k3s", "--namespace", "prod"]
        );
    }
}
