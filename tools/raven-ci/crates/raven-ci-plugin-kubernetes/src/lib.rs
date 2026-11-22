//! The plugin `kubernetes` of Raven CI: deploys with `kubectl`, the kubeconfig in the vault, and
//! a stuck rollout told in clear words.
//!
//! # Its role
//!
//! A pipeline that publishes to a cluster applies manifests and waits for the rollout. When it
//! gets stuck, the reason is in the pods (an image that does not download, a container killed for
//! memory, a pod no node accepts), and people go fishing for it with `kubectl describe`. This
//! plugin runs `kubectl` through `ravenrs-process` (no `kube-rs`), with the kubeconfig sealed in
//! the vault, and says why in the build's log.
//!
//! # How it works
//!
//! - **The kind of credential [`kubeconfig`](Kubeconfig)** holds the file. A log hides the whole
//!   file and each `token`, `client-key-data`, `client-certificate-data` and `password` in it.
//! - **The step `kubectl`** needs `namespace` and `context`: a deploy never falls into the
//!   `default` namespace or the current context of the machine that runs it. It does `apply`
//!   (one path or a list, relative to the build's folder) and then `rollout` (`deployment/site`),
//!   or free `args`. With `credential`, the kubeconfig goes into a file of a moment, outside the
//!   build's folder and readable only by its owner, given with `--kubeconfig` and removed when the
//!   step ends; without, `kubectl` reads the machine's (`KUBECONFIG` or `~/.kube/config`). Each
//!   line it writes goes to the log.
//! - **A rollout that does not end** is looked into: the pods of the resource's selector are read,
//!   and `ImagePullBackOff`, `OOMKilled` (with the memory limit), `CrashLoopBackOff` and a
//!   `Pending` pod no node accepts ("sem recurso no cluster") become the failure's message. A
//!   refusal of the server (`Forbidden`) becomes "o kubeconfig não tem permissão (403)", with who
//!   cannot do what, where.
//! - **The setting `kubectl`** says where the program is; empty, it is the one in `PATH`.
//!
//! # How to use it
//!
//! ```toml
//! [[stages]]
//! name = "Deploy"
//! when = { branch = "main" }
//! steps = [
//!   { uses = "kubernetes/kubectl", with = { namespace = "site", context = "producao", credential = "kube-producao", apply = "k8s/", rollout = "deployment/site" } },
//! ]
//! ```
//!
//! The plugin needs the permissions `ci.process` (it runs `kubectl`) and `ci.secrets` (the
//! kubeconfigs).

mod diagnose;
mod kubectl;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{CiError, CiPlugin, CredentialKind, Host, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest, Rule};
use ravenrs_flows::{Catalog, NodeError};
use serde_json::Value;

pub use diagnose::{forbidden, pods, selector};
pub use kubectl::{KUBECTL, KUBECTL_ENV, Kubectl, Plan, ROLLOUT_WAIT};

/// What the steps share with the plugin: the host and the `kubectl` program, while it is
/// switched on.
#[derive(Default)]
pub struct State {
    host: Mutex<Option<Arc<dyn Host>>>,
    program: Mutex<Option<Result<PathBuf, String>>>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State").finish_non_exhaustive()
    }
}

impl State {
    /// A state with `program` as `kubectl` and `host` for the vault: for a step run on its own,
    /// in a test.
    pub fn with(program: PathBuf, host: Option<Arc<dyn Host>>) -> Arc<Self> {
        Arc::new(Self { host: Mutex::new(host), program: Mutex::new(Some(Ok(program))) })
    }

    /// The `kubectl` program, or why there is none.
    pub fn program(&self) -> Result<PathBuf, NodeError> {
        match &*self.program.lock().unwrap_or_else(PoisonError::into_inner) {
            Some(Ok(program)) => Ok(program.clone()),
            Some(Err(why)) => Err(NodeError::new(why.clone())),
            None => Err(NodeError::new("o plugin kubernetes não está ligado")),
        }
    }

    fn secret(&self, name: &str) -> Result<String, NodeError> {
        let host = self
            .host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| NodeError::new("o plugin kubernetes não está ligado para ler a credencial"))?;
        host.secret(name).map_err(|e| NodeError::new(format!("a credencial {name}: {e}")))
    }
}

/// One text or a list of texts of a parameter.
pub(crate) fn texts(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) if !text.is_empty() => vec![text.clone()],
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        _ => Vec::new(),
    }
}

/// The kind of credential `kubeconfig`: the whole file.
#[derive(Clone, Copy, Debug, Default)]
pub struct Kubeconfig;

/// The keys of a kubeconfig whose values a log must hide.
const SECRET_KEYS: &[&str] =
    &["token", "client-key-data", "client-certificate-data", "password", "id-token", "refresh-token"];

impl CredentialKind for Kubeconfig {
    fn id(&self) -> &str {
        "kubeconfig"
    }

    fn caption(&self) -> String {
        "Kubeconfig".to_owned()
    }

    fn fields(&self) -> EntityInfo {
        let mut field = FieldInfo::new("kubeconfig", FieldKind::LongText);
        field.secret = true;
        EntityInfo::new("Kubeconfig").field(field).rule(Rule::required("kubeconfig"))
    }

    fn hidden(&self, credential: &Value) -> Vec<String> {
        let file = credential.get("kubeconfig").and_then(Value::as_str).unwrap_or_default();
        let mut hidden: Vec<String> = file
            .lines()
            .filter_map(|line| {
                let (key, value) = line.trim().trim_start_matches("- ").split_once(':')?;
                let value = value.trim().trim_matches('"').trim_matches('\'');
                (SECRET_KEYS.contains(&key.trim()) && value.len() >= 8).then(|| value.to_owned())
            })
            .collect();
        hidden.push(file.to_owned());
        hidden
    }
}

/// The plugin `kubernetes`.
#[derive(Debug, Default)]
pub struct KubernetesPlugin {
    state: Arc<State>,
}

impl KubernetesPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for KubernetesPlugin {
    fn name(&self) -> &str {
        "kubernetes"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Process.capability())
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for KubernetesPlugin {
    fn summary(&self) -> String {
        "kubectl apply e rollout com namespace e contexto, kubeconfig no cofre e mensagens claras.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(EntityInfo::new("KubernetesSettings").field(FieldInfo::new("kubectl", FieldKind::Text)))
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o kubectl")?;
        let setting =
            host.settings().get("kubectl").and_then(Value::as_str).unwrap_or_default().trim().to_owned();
        let name = if setting.is_empty() { "kubectl" } else { setting.as_str() };
        let program = ravenrs_process::resolve(name)
            .ok_or_else(|| format!("o kubectl ({name}) não foi encontrado: instale-o na máquina do build"));
        if let Err(why) = &program {
            // Switched on all the same: a step says why when a build needs it.
            host.log(why);
        }
        *self.state.program.lock().unwrap_or_else(PoisonError::into_inner) = Some(program);
        *self.state.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.state.host.lock().unwrap_or_else(PoisonError::into_inner).take();
        self.state.program.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Kubectl(self.state.clone()));
    }

    fn credential_kinds(&self) -> Vec<Arc<dyn CredentialKind>> {
        vec![Arc::new(Kubeconfig)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_kubeconfig_hides_its_tokens_and_keys() {
        let file = "users:\n- name: ci\n  user:\n    token: eyJhbGciOiJSUzI1NiIs\n    client-key-data: LS0tLS1CRUdJTiBSU0E=\n";
        let hidden = Kubeconfig.hidden(&json!({ "kubeconfig": file }));
        assert!(hidden.contains(&"eyJhbGciOiJSUzI1NiIs".to_owned()), "{hidden:?}");
        assert!(hidden.contains(&"LS0tLS1CRUdJTiBSU0E=".to_owned()), "{hidden:?}");
        assert!(hidden.contains(&file.to_owned()));
        assert_eq!(Kubeconfig.secret(&json!({ "kubeconfig": file })), file);
    }
}
