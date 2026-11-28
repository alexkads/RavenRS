//! The launcher `kubernetes`: an agent in a pod of its own, one per build, from the job's model
//! (image, resources, `nodeSelector`, helper containers), deleted at the end of the build or when
//! it passes its time, with a limit of pods at once in the cluster.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use raven_ci_plugin_api::{AgentArgs, AgentLauncher, CiError, LaunchRequest, LaunchedAgent, TOKEN_VARIABLE};
use ravenrs_process::{End, Output, Spec};
use serde_json::{Value, json};

use crate::kubectl::{KUBECTL_ENV, Scratch};
use crate::{State, diagnose, texts};

/// The label every pod of an agent has, with its name.
pub const AGENT_LABEL: &str = "raven-ci.agent";

/// How many pods of agents a cluster runs at once when the settings say nothing.
pub const MAX_PODS: u64 = 10;

/// How long a pod may wait for a node when the settings say nothing.
pub const PENDING_TIMEOUT: Duration = Duration::from_secs(300);

/// Where the agent keeps its builds in the pod: an `emptyDir`, gone with the pod.
pub const AGENT_FOLDER: &str = "/var/lib/raven-ci-agent";

/// `text` as a name or a label value Kubernetes takes: small letters, digits and `-`, at most
/// `max` long, starting and ending with a letter or a digit.
pub fn dns(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(max).collect();
    let out = out.trim_end_matches('-');
    if out.is_empty() { "x".to_owned() } else { out.to_owned() }
}

/// What a pod of an agent is made of.
#[derive(Clone, Debug, PartialEq)]
pub struct PodPlan {
    /// The pod's name, which is the agent's.
    pub name: String,
    /// Where it goes.
    pub namespace: String,
    /// The cluster.
    pub context: String,
    /// The kubeconfig of the vault, by name; none for the machine's.
    pub credential: Option<String>,
    /// How many pods of agents the cluster runs at once.
    pub max_pods: u64,
    /// How long it may wait for a node.
    pub pending_timeout: Duration,
    /// How long it may live; none for as long as the build.
    pub timeout: Option<Duration>,
    /// The pod, as JSON.
    pub pod: Value,
}

impl PodPlan {
    /// The plan of `request`, from its settings: `image`, `namespace` and `context` (needed),
    /// `credential`, `resources`, `node_selector`, `sidecars`, `command` (the agent's program,
    /// `raven-ci-agent` unless said), `executors`, `max_pods`, `pending_timeout_s` and
    /// `timeout_s`.
    pub fn read(request: &LaunchRequest, n: u64) -> Result<Self, CiError> {
        let settings = &request.settings;
        let needed = |name: &str, why: &str| {
            settings
                .get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| CiError::settings(name, why))
        };
        let image = needed("image", "a imagem do agente, com o raven-ci-agent dentro")?;
        let namespace = needed("namespace", "onde os pods dos agentes entram")?;
        let context = needed("context", "o cluster, para não cair no contexto atual da máquina")?;
        let whose = request.build.as_deref().unwrap_or("livre");
        let name = format!("{}-{}-{n}", dns(&format!("raven-ci-agente-{whose}"), 40), std::process::id());
        let command = match texts(settings.get("command")) {
            words if words.is_empty() => vec!["raven-ci-agent".to_owned()],
            words => words,
        };
        let executors = settings.get("executors").and_then(Value::as_u64).unwrap_or(1).max(1);
        let timeout =
            settings.get("timeout_s").and_then(Value::as_u64).filter(|s| *s > 0).map(Duration::from_secs);
        let secret = format!("{name}-token");
        let mut agent = json!({
            "name": "agente",
            "image": image,
            "command": command,
            "args": AgentArgs::from_request(request, &name, executors, AGENT_FOLDER).to_args(),
            "env": [{
                "name": TOKEN_VARIABLE,
                "valueFrom": { "secretKeyRef": { "name": secret, "key": "token" } },
            }],
            "volumeMounts": [{ "name": "trabalho", "mountPath": AGENT_FOLDER }],
        });
        if let Some(resources) = settings.get("resources").filter(|r| r.is_object()) {
            agent["resources"] = resources.clone();
        }
        let mut containers = vec![agent];
        if let Some(Value::Array(sidecars)) = settings.get("sidecars") {
            containers.extend(sidecars.iter().cloned());
        }
        let mut labels = json!({ AGENT_LABEL: name });
        if let Some(build) = &request.build {
            labels["raven-ci.agent-build"] = json!(dns(build, 63));
        }
        let mut spec = json!({
            "restartPolicy": "Never",
            "automountServiceAccountToken": false,
            "terminationGracePeriodSeconds": 10,
            "containers": containers,
            "volumes": [{ "name": "trabalho", "emptyDir": {} }],
        });
        if let Some(selector) = settings.get("node_selector").filter(|s| s.is_object()) {
            spec["nodeSelector"] = selector.clone();
        }
        if let Some(limit) = timeout {
            // The cluster ends the pod at its time even when this server is down.
            spec["activeDeadlineSeconds"] = json!(limit.as_secs());
        }
        Ok(Self {
            // The token in a Secret of the pod's own, never in the pod's spec; both go together.
            pod: json!({
                "apiVersion": "v1",
                "kind": "List",
                "items": [
                    {
                        "apiVersion": "v1",
                        "kind": "Secret",
                        "metadata": { "name": secret, "labels": labels },
                        "type": "Opaque",
                        "stringData": { "token": request.token },
                    },
                    {
                        "apiVersion": "v1",
                        "kind": "Pod",
                        "metadata": { "name": name, "labels": labels },
                        "spec": spec,
                    },
                ],
            }),
            name,
            namespace,
            context,
            credential: settings
                .get("credential")
                .and_then(Value::as_str)
                .filter(|c| !c.is_empty())
                .map(str::to_owned),
            max_pods: settings.get("max_pods").and_then(Value::as_u64).unwrap_or(MAX_PODS).max(1),
            pending_timeout: settings
                .get("pending_timeout_s")
                .and_then(Value::as_u64)
                .map_or(PENDING_TIMEOUT, Duration::from_secs),
            timeout,
        })
    }
}

/// `kubectl` on one cluster and namespace, with its kubeconfig of a moment.
#[derive(Debug)]
struct Cluster {
    program: PathBuf,
    base: Vec<String>,
    scratch: Scratch,
}

impl Cluster {
    fn run(&self, args: &[&str]) -> Result<Output, CiError> {
        KUBECTL_ENV
            .iter()
            .fold(Spec::new(&self.program), |spec, name| spec.allow_env(*name))
            .args(self.base.iter().map(String::as_str).chain(args.iter().copied()))
            .timeout(Duration::from_secs(60))
            .output()
            .map_err(|e| CiError::failed(format!("o kubectl não rodou: {e}")))
    }

    fn json(&self, args: &[&str]) -> Option<Value> {
        let output = self.run(args).ok()?;
        (output.end == End::Code(0)).then(|| serde_json::from_str(&output.stdout.join("\n")).ok()).flatten()
    }

    /// Deletes the pod `name` and the Secret of its token.
    fn delete(&self, name: &str) {
        let _ = self.run(&["delete", "pod", name, "--ignore-not-found", "--wait=false"]);
        let _ = self.run(&["delete", "secret", &format!("{name}-token"), "--ignore-not-found"]);
    }

    /// The failure of `output`, explained when the server refused.
    fn failure(what: &str, output: &Output) -> CiError {
        if let Some(refused) = diagnose::forbidden(&output.stderr) {
            return CiError::failed(refused);
        }
        let why = output.stderr.iter().rev().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or_default();
        CiError::failed(format!("{what}: {why}"))
    }
}

/// How many pods of agents run in the cluster: in every namespace when the kubeconfig may list
/// them, otherwise in the agents' own.
fn running(cluster: &Cluster) -> usize {
    let selector = format!("-l={AGENT_LABEL}");
    let count = |output: Output| output.stdout.iter().filter(|l| !l.trim().is_empty()).count();
    match cluster.run(&[
        "get",
        "pods",
        "-A",
        &selector,
        "-o",
        "name",
        "--field-selector=status.phase!=Succeeded,status.phase!=Failed",
    ]) {
        Ok(output) if output.end == End::Code(0) => count(output),
        _ => cluster
            .run(&[
                "get",
                "pods",
                &selector,
                "-o",
                "name",
                "--field-selector=status.phase!=Succeeded,status.phase!=Failed",
            ])
            .map_or(0, count),
    }
}

/// The launcher `kubernetes`. See the [crate](crate).
#[derive(Debug)]
pub struct KubernetesLauncher(pub Arc<State>);

/// An agent in a pod.
#[derive(Debug)]
pub struct PodAgent {
    name: String,
    cluster: Arc<Cluster>,
    stopped: Arc<Mutex<bool>>,
}

impl LaunchedAgent for PodAgent {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn alive(&self) -> bool {
        let phase = self.cluster.json(&["get", "pod", &self.name, "-o", "json"]);
        matches!(
            phase.as_ref().and_then(|p| p.get("status")).and_then(|s| s.get("phase")).and_then(Value::as_str),
            Some("Running" | "Pending")
        )
    }

    fn stop(&self) {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.cluster.delete(&self.name);
    }
}

impl AgentLauncher for KubernetesLauncher {
    fn id(&self) -> &str {
        "kubernetes"
    }

    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, CiError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let program = self.0.program().map_err(|e| CiError::unavailable(e.message))?;
        let plan = PodPlan::read(request, NEXT.fetch_add(1, Ordering::SeqCst))?;
        let scratch = Scratch::new(&plan.name).map_err(|e| CiError::failed(e.message))?;
        let mut base = Vec::new();
        if let Some(name) = &plan.credential {
            let text = self.0.secret(name).map_err(|e| CiError::failed(e.message))?;
            let path = scratch.write("config", &text).map_err(|e| CiError::failed(e.message))?;
            base.extend(["--kubeconfig".to_owned(), path.display().to_string()]);
        }
        base.extend([
            "--context".to_owned(),
            plan.context.clone(),
            "--namespace".to_owned(),
            plan.namespace.clone(),
        ]);
        let cluster = Arc::new(Cluster { program, base, scratch });

        let now = running(&cluster);
        if u64::try_from(now).unwrap_or(u64::MAX) >= plan.max_pods {
            return Err(CiError::unavailable(format!(
                "o limite de {} pods de agente no cluster foi atingido ({now} rodando): o build espera um terminar",
                plan.max_pods
            )));
        }
        let manifest = cluster
            .scratch
            .write("pod.json", &plan.pod.to_string())
            .map_err(|e| CiError::failed(e.message))?;
        let created = cluster.run(&["create", "-f", &manifest.display().to_string()]);
        // The manifest has the token: it goes as soon as the cluster has it.
        let _ = std::fs::remove_file(&manifest);
        let created = created?;
        if created.end != End::Code(0) {
            cluster.delete(&plan.name);
            return Err(Cluster::failure("o pod do agente não foi criado", &created));
        }

        // Until a node takes it: a pod no node takes, or whose image does not come, goes.
        let until = Instant::now() + plan.pending_timeout;
        loop {
            let pod = cluster.json(&["get", "pod", &plan.name, "-o", "json"]).unwrap_or(Value::Null);
            let phase =
                pod.get("status").and_then(|s| s.get("phase")).and_then(Value::as_str).unwrap_or_default();
            let found = diagnose::pods(&json!({ "items": [pod] }));
            let pulling = found.iter().find(|m| m.starts_with("a imagem "));
            match phase {
                "Running" | "Succeeded" => break,
                "Failed" => {
                    cluster.delete(&plan.name);
                    return Err(CiError::failed(format!("o pod do agente {} falhou ao subir", plan.name)));
                }
                _ if pulling.is_some() => {
                    cluster.delete(&plan.name);
                    return Err(CiError::failed(pulling.cloned().unwrap_or_default()));
                }
                _ if Instant::now() >= until => {
                    cluster.delete(&plan.name);
                    let why = found
                        .iter()
                        .find(|m| m.starts_with("sem recurso no cluster"))
                        .cloned()
                        .unwrap_or_else(|| {
                            format!("sem recurso no cluster: o pod {} ficou em Pending", plan.name)
                        });
                    return Err(CiError::failed(format!(
                        "{why} (esperou {} s e foi apagado)",
                        plan.pending_timeout.as_secs()
                    )));
                }
                _ => std::thread::sleep(Duration::from_secs(2)),
            }
        }

        let agent = PodAgent { name: plan.name.clone(), cluster, stopped: Arc::default() };
        if let Some(limit) = plan.timeout {
            // Past its time the pod goes, whether or not the core remembered it.
            let (cluster, name, stopped) = (agent.cluster.clone(), agent.name.clone(), agent.stopped.clone());
            let _ = std::thread::Builder::new().name(format!("prazo-{name}")).spawn(move || {
                std::thread::sleep(limit);
                if !*stopped.lock().unwrap_or_else(PoisonError::into_inner) {
                    cluster.delete(&name);
                }
            });
        }
        Ok(Box::new(agent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(settings: Value) -> LaunchRequest {
        LaunchRequest {
            build: Some("apps/site#12".into()),
            labels: vec!["build:apps/site#12".into()],
            settings,
            server: "https://ci.casa".into(),
            token: "tok-uso-unico".into(),
        }
    }

    #[test]
    fn names_are_what_kubernetes_takes() {
        assert_eq!(dns("Raven CI/apps#12", 63), "raven-ci-apps-12");
        assert_eq!(dns(&"a".repeat(80), 63).len(), 63);
        assert_eq!(dns("--", 10), "x");
    }

    #[test]
    fn a_pod_without_image_namespace_or_context_is_refused() {
        for missing in ["image", "namespace", "context"] {
            let mut settings = json!({ "image": "ghcr.io/eu/agente:1", "namespace": "ci", "context": "k3s" });
            settings.as_object_mut().expect("an object").remove(missing);
            let refused = PodPlan::read(&request(settings), 0).expect_err("refused");
            assert!(refused.to_string().contains(missing), "{refused}");
        }
    }

    #[test]
    fn the_pod_has_the_model_of_the_job_and_the_words_of_the_contract() {
        let plan = PodPlan::read(
            &request(json!({
                "image": "ghcr.io/eu/agente:1", "namespace": "ci", "context": "k3s",
                "resources": { "limits": { "memory": "2Gi" } },
                "node_selector": { "kubernetes.io/arch": "amd64" },
                "sidecars": [{ "name": "db", "image": "postgres:17" }],
                "timeout_s": 3600,
            })),
            0,
        )
        .expect("a plan");
        let secret = &plan.pod["items"][0];
        assert_eq!(secret["kind"], "Secret");
        assert_eq!(secret["stringData"]["token"], "tok-uso-unico");
        let pod = &plan.pod["items"][1];
        let spec = &pod["spec"];
        assert_eq!(spec["restartPolicy"], "Never");
        assert_eq!(spec["activeDeadlineSeconds"], 3600);
        assert_eq!(spec["nodeSelector"]["kubernetes.io/arch"], "amd64");
        assert_eq!(spec["containers"][1]["name"], "db");
        let agent = &spec["containers"][0];
        assert_eq!(agent["command"], json!(["raven-ci-agent"]));
        assert_eq!(agent["resources"]["limits"]["memory"], "2Gi");
        let args: Vec<&str> =
            agent["args"].as_array().expect("args").iter().filter_map(Value::as_str).collect();
        assert!(args.windows(2).any(|w| w == ["--servidor", "https://ci.casa"]), "{args:?}");
        assert!(args.windows(2).any(|w| w == ["--rotulos", "build:apps/site#12"]), "{args:?}");
        assert!(!args.contains(&"tok-uso-unico"), "the token is not in the pod's words: {args:?}");
        assert_eq!(agent["env"][0]["name"], TOKEN_VARIABLE);
        assert_eq!(agent["env"][0]["valueFrom"]["secretKeyRef"]["name"], format!("{}-token", plan.name));
        assert_eq!(pod["metadata"]["labels"][AGENT_LABEL], json!(plan.name));
        assert_eq!(pod["metadata"]["labels"]["raven-ci.agent-build"], "apps-site-12");
        assert!(spec.get("hostNetwork").is_none(), "the agent connects out; nothing is opened");
    }
}
