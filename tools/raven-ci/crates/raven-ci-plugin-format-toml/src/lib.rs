//! The plugin `format-toml` of Raven CI: the pipeline in `.raven-ci.toml`.
//!
//! # Its role
//!
//! The core runs flows; it reads no file. This plugin reads the format of the house, a short TOML
//! that says what a declarative `Jenkinsfile` says without Groovy, and turns it into the flow a
//! build runs, with its parameters, post steps, publishers and time limit.
//!
//! # The file
//!
//! ```toml
//! # Keys of the top come before the first table, or TOML puts them inside it.
//! artifacts = ["target/*.tar.gz"]   # plugin artifacts
//! junit = ["**/junit.xml"]          # plugin junit
//!
//! [pipeline]
//! timeout = "30m"         # the whole build; the build is cancelled when it passes
//! keep = 10                # how many builds the job keeps
//! agent = "any"            # or { label = "linux" }, { docker = { image = "rust:1" } }, { kubernetes = { … } }
//!
//! [environment]
//! CARGO_TERM_COLOR = "always"
//!
//! [[parameters]]
//! name = "E2E"
//! type = "bool"            # bool, text or choice (with choices = ["a", "b"])
//! default = false
//! description = "Roda os testes de ponta a ponta"
//!
//! [[stages]]
//! name = "Teste"
//! when = { branch = "main" }       # or ["main", "release/*"], { pull_request = true }, { parameter = "E2E" }
//! timeout = "10m"                  # each step of the stage
//! retry = 2                        # each step tried up to 2 more times
//! lock = "maquina"                 # one build at a time holds it (plugin lock)
//! nice = 10                        # each sh runs under nice -n 10
//! container = { image = "rust:1" } # each sh runs in the image (plugin docker)
//! steps = [
//!   "cargo test",                                # a sh
//!   { sh = "./e2e.sh", timeout = "5m", name = "Ponta a ponta" },
//!   { cargo = "clippy" },                        # plugin cargo
//!   { uses = "kubernetes/kubectl", with = { args = "apply -f k8s/" } },
//! ]
//!
//! [[stages]]
//! name = "Paralelo"
//! parallel = [
//!   { name = "linux", steps = ["make linux"] },
//!   { name = "web", steps = ["make web"] },
//! ]
//!
//! [post]
//! always = ["./limpa.sh"]
//! failure = ["echo falhou"]
//! ```
//!
//! # How it works
//!
//! - **Every error at once**, each with its line: a field that does not exist, a value of the
//!   wrong type, a duration it cannot read, a stage with no steps, two stages with one name.
//! - **Steps are nodes.** Each step is a node of the flow with its stage ([`STAGE_KEY`](raven_ci_plugin_api::STAGE_KEY)) and the
//!   plugin it comes from ([`PLUGIN_KEY`](raven_ci_plugin_api::PLUGIN_KEY)): `sh` from `shell`, `cargo` from `cargo`, a step in a
//!   stage with `container` from `docker`, `lock` from `lock`, `uses = "p/k"` from `p`. With that
//!   plugin switched off, the core refuses the build saying "falta o plugin p".
//! - **`when.branch` and `when.pull_request`** are decided by the last part of the job's name, the
//!   head of a job of many branches (`main`, `PR-12`): a stage that does not run has its steps
//!   switched off, and they pass the build on. **`when.parameter`** is decided as the build runs,
//!   by an `if` of `ravenrs-flows` before the stage and a `merge` after it.
//! - **`parallel`** branches leave the step before and meet in `merge` nodes after.
//! - **`timeout` and `retry` of a stage** go to each of its steps, as the node's own
//!   `timeout_ms` and `retry`.

mod read;

use std::sync::Arc;

use raven_ci_plugin_api::{CiPlugin, FormatError, Pipeline, PipelineFormat};
use ravenrs_app::{Module, ModuleManifest};

/// The name of the file the format reads.
pub const FILE_NAME: &str = ".raven-ci.toml";

/// The plugin `format-toml`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TomlFormatPlugin;

impl Module for TomlFormatPlugin {
    fn name(&self) -> &str {
        "format-toml"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default()
    }
}

impl CiPlugin for TomlFormatPlugin {
    fn summary(&self) -> String {
        "O pipeline em .raven-ci.toml: estágios, passos, parâmetros, when, post e erros com a linha."
            .to_owned()
    }

    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        vec![Arc::new(TomlFormat)]
    }
}

/// The format `toml`. See the [crate](crate).
#[derive(Clone, Copy, Debug, Default)]
pub struct TomlFormat;

impl PipelineFormat for TomlFormat {
    fn id(&self) -> &str {
        "toml"
    }

    fn file_names(&self) -> Vec<String> {
        vec![FILE_NAME.to_owned()]
    }

    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
        read::read(job, text)
    }
}
