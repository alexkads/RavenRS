//! What a plugin of Raven CI implements.
//!
//! # The role of a plugin
//!
//! The core of Raven CI knows a queue, jobs and builds, and the register of plugins. It knows no
//! git, no GitHub, no cron, no JUnit, no shell: with no plugin it starts and answers, and has
//! nothing to run. Everything a CI does comes from a plugin, through the same API a plugin
//! written by someone else uses. A plugin is either compiled in (a crate `raven-ci-plugin-*`) or
//! a program of any language that speaks JSON-RPC on its standard input and output; both are
//! the same [`CiPlugin`] to the core.
//!
//! # How a plugin works
//!
//! A plugin is a [`ravenrs_app::Module`] (its name, its [`ModuleManifest`](ravenrs_app::ModuleManifest)
//! with its version and the versions it accepts of other plugins, its entities and screens) and
//! a [`CiPlugin`], which adds the extension points of the CI. Its life:
//!
//! 1. **Made.** The core holds an instance of every plugin it knows, switched on or not. Making
//!    one does no work: a plugin switched off costs nothing but its struct, opens no file and
//!    adds no route.
//! 2. **Planned.** The core reads every manifest and puts the plugins in an order where each
//!    comes after what it requires ([`ravenrs_app::plan_modules`]). A plugin that does not fit
//!    stays out with the reason, and so do the plugins that require it; the others go on.
//! 3. **Started.** Switched on, in that order, [`CiPlugin::start`] gets a [`Host`]: its
//!    configuration, already checked against [`CiPlugin::config`], its own folder and SQLite
//!    database with its [`CiPlugin::migrations`] applied, secrets and a log. Each service checks
//!    the [`Permission`] it needs, and every answer, yes or no, goes to the audit.
//! 4. **Asked.** Only started plugins are asked for their extension points, and asked again
//!    each time the set of plugins switched on changes.
//! 5. **Stopped.** Switched off, the plugins that require it stop first, then it; its routes and
//!    steps go with it, and nothing restarts.
//!
//! # The extension points
//!
//! | Point | Trait | What it offers |
//! |---|---|---|
//! | Step | [`ravenrs_flows::NodeKind`], from [`CiPlugin::steps`] | a kind of step of a pipeline (sh, container, cargo, kubectl), which finds its build with [`BuildScope::of`] |
//! | Scm | [`Scm`] | fetching code: a clone, a reference, a pull request merged onto its target |
//! | Branch source | [`BranchSource`] | finding the branches and pull requests of a repository (multibranch) |
//! | Trigger | [`Trigger`] | when a build is asked for: by hand, by the API, by a schedule, a poll or a webhook |
//! | Publisher | [`Publisher`] | what a build leaves: artifacts, test results, images, as [`Attachment`]s |
//! | Status reporter | [`StatusReporter`] | the status of a commit, back to its provider |
//! | Notifier | [`Notifier`] | telling people: e-mail, an outgoing webhook |
//! | Credential kind | [`CredentialKind`] | a kind of credential: user and password, token, SSH key, registry, kubeconfig |
//! | Agent launcher | [`AgentLauncher`] | how an agent comes up: local, remote, a container, a pod |
//! | Pipeline format | [`PipelineFormat`] | reading a pipeline file into a flow |
//! | Routes | [`Route`] | addresses of the HTTP API |
//! | CLI command | [`CliCommand`] | subcommands of `raven-ci` |
//! | Build hook | [`BuildHook`] | what is done to every build, before and after it: refusing it with no room on the disk |
//! | Screens | [`Module::view_controllers`](ravenrs_app::Module::view_controllers) and [`DashboardInfo`](ravenrs_app::DashboardInfo) | a page, a tab on the page of a build, an item of the menu |
//!
//! Each trait says, on its own page, what it is for, how the core calls it and how to write one.
//!
//! # How to write one
//!
//! ```
//! use std::sync::Arc;
//!
//! use raven_ci_plugin_api::{CiError, CiPlugin, Host, Permission};
//! use ravenrs_app::{Module, ModuleManifest};
//! use ravenrs_flows::{Catalog, NodeContext, NodeError, NodeKind, NodeSpec, Ports};
//!
//! /// Says hello in the log of the build.
//! struct Hello;
//!
//! impl NodeKind for Hello {
//!     fn spec(&self) -> NodeSpec {
//!         NodeSpec::new("hello", 1).input("main").output("main")
//!     }
//!     fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
//!         Ok(Ports::from([("main".to_owned(), cx.input("main").to_vec())]))
//!     }
//! }
//!
//! struct HelloPlugin;
//!
//! impl Module for HelloPlugin {
//!     fn name(&self) -> &str {
//!         "hello"
//!     }
//!     fn manifest(&self) -> ModuleManifest {
//!         ModuleManifest::new()
//!             .version("0.1.0")
//!             .expect("a version")
//!             .capability(Permission::Disk.capability())
//!     }
//! }
//!
//! impl CiPlugin for HelloPlugin {
//!     fn summary(&self) -> String {
//!         "Says hello.".to_owned()
//!     }
//!     fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
//!         host.log("hello is on");
//!         Ok(())
//!     }
//!     fn steps(&self, catalog: &mut Catalog) {
//!         catalog.add(Hello);
//!     }
//! }
//! ```
//!
//! `tools/raven-ci/docs/writing-a-plugin.md` goes through both kinds of plugin, the manifest of an
//! external one and its protocol.

mod agent;
mod build;
mod cli;
mod credential;
mod error;
mod hook;
mod host;
mod http;
mod permission;
mod pipeline;
mod plugin;
mod publish;
mod scm;
mod settings;
mod trigger;

#[cfg(feature = "testing")]
pub mod testing;

pub use agent::{AgentLauncher, LaunchRequest, LaunchedAgent};
pub use build::{BuildContext, BuildLog, BuildScope, MemoryLog, ScopeGuard, enter};
pub use cli::CliCommand;
pub use credential::CredentialKind;
pub use error::CiError;
pub use hook::BuildHook;
pub use host::Host;
pub use http::{HttpRequest, HttpResponse, ROLE_ADMIN, ROLE_READ, ROLE_RUN, Route};
pub use permission::Permission;
pub use pipeline::{FormatError, PLUGIN_KEY, Pipeline, PipelineFormat, PostSteps, PostWhen, STAGE_KEY, Use};
pub use plugin::{CiPlugin, Migration, Point, has_screens, points_of};
pub use publish::{
    Attachment, BuildEvent, BuildInfo, BuildStatus, CommitState, CommitStatus, Notifier, Publisher,
    StatusReporter,
};
pub use scm::{BranchSource, Checkout, CheckoutRequest, Head, HeadKind, Reference, Scm};
pub use settings::check_settings;
pub use trigger::{BuildRequest, Builds, Trigger};
