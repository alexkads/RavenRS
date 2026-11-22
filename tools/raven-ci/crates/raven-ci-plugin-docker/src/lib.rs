//! The plugin `docker` of Raven CI: steps in an image, images built and sent, the services of the
//! tests, and nothing left behind.
//!
//! # Its role
//!
//! A build often needs a toolchain the machine does not have (`rust:1`, `node:22`), an image to
//! publish, or a database for its tests. This plugin brings that through the `docker` command,
//! or `podman`, run by `ravenrs-process`: no SDK, no daemon of its own, the same engine people
//! use by hand.
//!
//! # How it works
//!
//! - **The engine** is the setting `engine` (`docker`, `podman` or a path), or the first of the
//!   two in `PATH`. The command gets the server's `DOCKER_HOST`, `DOCKER_CONTEXT`,
//!   `DOCKER_CONFIG`, `CONTAINER_HOST` and `XDG_RUNTIME_DIR`, so it talks to the engine the
//!   machine's user talks to. With no engine, a step fails saying "nem docker nem podman no PATH".
//! - **`container`** (what a stage with `container = { image, args, env }` of `.raven-ci.toml`
//!   makes): "buscar imagem" (the image on the machine is used; one that is not is pulled, and
//!   one that does not exist fails with its name), then `run --rm` with the build's folder mounted
//!   at its own path and as the working folder, `sh -e -c <script>` as the entry, as the owner of
//!   the folder (`--user uid:gid`, or `--userns=keep-id` with Podman) with `HOME=/tmp`, so no file
//!   of root is left in the folder. The build's variables and the container's `env` go as
//!   `-e NAME`, their values in the command's environment: no secret is in the arguments or in
//!   `docker inspect`. When the build is cancelled or the step passes its time, the container is
//!   removed (`rm -f`), not only the command.
//! - **`docker-build`** and **`docker-push`** (`uses = "docker/docker-build"`): the login of a
//!   credential of kind [`registry`](registry::Registry) (server, user, password) goes into a
//!   configuration of a moment, outside the build's folder and removed after the step, read with
//!   `--config` (`REGISTRY_AUTH_FILE` for Podman). The password and its `auth` are hidden in the
//!   log; the machine's own configuration is not touched.
//! - **`compose`** (`uses = "docker/compose"`): `up -d` with the project `raven-ci-<run>`, then
//!   a wait on `compose ps --format json` until each service is `healthy` (or running, with no
//!   healthcheck, or ended well), failing with the name of one that is `unhealthy` or stopped.
//!   The `container` steps after it join the project's network, `<project>_default`, so the
//!   tests reach `db:5432`.
//! - **Nothing outlives the build.** Every container has the label `raven-ci.build=<run>`; the
//!   first step of the plugin in a build asks [`BuildScope::at_end`] to bring down its compose
//!   projects with their volumes and to remove every container, network and volume with the
//!   label, however the build ends: passed, failed, cancelled or out of time.
//!
//! # How to use it
//!
//! ```toml
//! [[stages]]
//! name = "Teste"
//! container = { image = "rust:1", env = { CARGO_TERM_COLOR = "always" } }
//! steps = [
//!   { uses = "docker/compose", with = { file = "tests/compose.yml" } },
//!   "cargo test",
//! ]
//!
//! [[stages]]
//! name = "Imagem"
//! steps = [
//!   { uses = "docker/docker-build", with = { tag = "ghcr.io/eu/site:1", push = true, credential = "ghcr" } },
//! ]
//! ```
//!
//! The plugin needs the permissions `ci.process` (it runs the engine) and `ci.secrets` (the
//! logins of registries).

mod cleanup;
mod compose;
mod container;
mod engine;
mod image;
mod registry;

use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{CiError, CiPlugin, CredentialKind, Host, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use ravenrs_flows::{Catalog, NodeError};
use serde_json::Value;

pub use cleanup::{LABEL, Project, clean, project_name};
pub use compose::{COMPOSE, Compose, Health, Service, health, services};
pub use container::{CONTAINER, Container, RunAs, RunPlan, fetch_image};
pub use engine::{ENGINE_ENV, Engine, slug};
pub use image::{DOCKER_BUILD, DOCKER_PUSH, DockerBuild, DockerPush, build_args};
pub use registry::{DOCKER_HUB, Login, Registry, server_of};

/// What the steps share with the plugin: the host and the engine, while it is switched on.
#[derive(Default)]
pub struct State {
    host: Mutex<Option<Arc<dyn Host>>>,
    engine: Mutex<Option<Result<Engine, String>>>,
}

impl State {
    /// A state with the engine `engine` and no host: for a step run on its own, in a test.
    pub fn with_engine(engine: Engine) -> Arc<Self> {
        Arc::new(Self { host: Mutex::default(), engine: Mutex::new(Some(Ok(engine))) })
    }

    /// The engine, or why there is none.
    pub fn engine(&self) -> Result<Engine, NodeError> {
        match &*self.engine.lock().unwrap_or_else(PoisonError::into_inner) {
            Some(Ok(engine)) => Ok(engine.clone()),
            Some(Err(why)) => Err(NodeError::new(why.clone())),
            None => Err(NodeError::new("o plugin docker não está ligado")),
        }
    }

    /// The secret `name` of the vault.
    fn secret(&self, name: &str) -> Result<String, NodeError> {
        let host = self
            .host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| NodeError::new("o plugin docker não está ligado para ler a credencial"))?;
        host.secret(name).map_err(|e| NodeError::new(format!("a credencial {name}: {e}")))
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State").finish_non_exhaustive()
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

/// The plugin `docker`.
#[derive(Debug, Default)]
pub struct DockerPlugin {
    state: Arc<State>,
}

impl DockerPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for DockerPlugin {
    fn name(&self) -> &str {
        "docker"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Process.capability())
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for DockerPlugin {
    fn summary(&self) -> String {
        "Passos numa imagem, docker build e push, compose com saúde, e limpeza no fim do build.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(EntityInfo::new("DockerSettings").field(FieldInfo::new("engine", FieldKind::Text)))
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o docker ou o podman")?;
        let setting = host.settings().get("engine").and_then(Value::as_str).unwrap_or_default().to_owned();
        let engine = Engine::find(&setting);
        match &engine {
            Ok(engine) => host.log(&format!("motor: {}", engine.program.display())),
            // Switched on all the same: a step says why when a build needs it.
            Err(why) => host.log(why),
        }
        *self.state.engine.lock().unwrap_or_else(PoisonError::into_inner) = Some(engine);
        *self.state.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.state.host.lock().unwrap_or_else(PoisonError::into_inner).take();
        self.state.engine.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Container(self.state.clone()));
        catalog.add(DockerBuild(self.state.clone()));
        catalog.add(DockerPush(self.state.clone()));
        catalog.add(Compose(self.state.clone()));
    }

    fn credential_kinds(&self) -> Vec<Arc<dyn CredentialKind>> {
        vec![Arc::new(Registry)]
    }
}
