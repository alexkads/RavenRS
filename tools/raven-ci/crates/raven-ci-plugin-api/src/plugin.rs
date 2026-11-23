//! The trait every plugin implements, over the module of `ravenrs-app`.

use std::fmt;
use std::sync::Arc;

use ravenrs_app::{EntityInfo, Module};
use ravenrs_flows::Catalog;
use serde::{Deserialize, Serialize};

use crate::{
    AgentLauncher, BranchSource, BuildHook, CiError, CliCommand, CredentialKind, Host, Notifier,
    PipelineFormat, Publisher, Route, Scm, StatusReporter, Trigger,
};

/// A change to a plugin's own database, applied once, in the order given, before the plugin
/// starts. The core keeps which ran in that database, so an id never runs twice, even when the
/// plugin is switched off and on again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Migration {
    /// What it is known by; never reused for another change.
    pub id: String,
    /// The SQL it runs.
    pub sql: String,
}

impl Migration {
    /// The change `id`, running `sql`.
    pub fn new(id: impl Into<String>, sql: impl Into<String>) -> Self {
        Self { id: id.into(), sql: sql.into() }
    }
}

/// A plugin of Raven CI: a [`Module`] of `ravenrs-app` (name, manifest, entities, screens) with
/// the extension points of the CI. Every point has a default that offers nothing, so a plugin
/// writes only the points it has.
///
/// **Making one does no work.** The core holds an instance of every plugin it knows, switched on
/// or not: the constructor sets nothing up, and [`start`](Self::start) does. A plugin keeps what it
/// sets up behind a lock of its own and lets it go in [`stop`](Self::stop).
///
/// **The points are asked after `start`.** The core asks a plugin for its points only while it
/// is switched on, and asks again each time the set of plugins switched on changes. A point may
/// keep the [`Host`] the plugin got in `start`.
///
/// It is `Send + Sync` because builds run on threads of their own.
pub trait CiPlugin: Module + Send + Sync {
    /// What it is for, in one line, for the list of plugins.
    fn summary(&self) -> String {
        String::new()
    }

    /// Whether it is on the first time the server sees it, before an administrator says. True by
    /// default; a plugin that needs something the machine may not have (a model of AI, a program
    /// installed apart) says false, and an administrator switches it on (#942).
    fn on_from_the_start(&self) -> bool {
        true
    }

    /// What its configuration holds, as an entity of `ravenrs-app`: the same description a form
    /// edits (`DetailView`, `ParameterForm`) and [`check_settings`](crate::check_settings) checks.
    /// `None` when it has no configuration.
    fn config(&self) -> Option<EntityInfo> {
        None
    }

    /// The changes to its own database, in order. A plugin with migrations needs
    /// [`Permission::Disk`](crate::Permission::Disk).
    fn migrations(&self) -> Vec<Migration> {
        Vec::new()
    }

    /// Switched on: sets up what it needs, through `host`. An error keeps it switched off, with
    /// the error as the reason.
    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        let _ = host;
        Ok(())
    }

    /// Switched off: lets go of what it set up. The plugins that require it stopped before.
    fn stop(&self) {}

    /// Adds its kinds of step to `catalog`, each a [`NodeKind`](ravenrs_flows::NodeKind): what a
    /// pipeline names in its steps.
    fn steps(&self, catalog: &mut Catalog) {
        let _ = catalog;
    }

    /// Its ways of fetching code.
    fn scms(&self) -> Vec<Arc<dyn Scm>> {
        Vec::new()
    }

    /// Its ways of finding branches and pull requests.
    fn branch_sources(&self) -> Vec<Arc<dyn BranchSource>> {
        Vec::new()
    }

    /// Its ways of asking for builds.
    fn triggers(&self) -> Vec<Arc<dyn Trigger>> {
        Vec::new()
    }

    /// What it takes from a build when it ends.
    fn publishers(&self) -> Vec<Arc<dyn Publisher>> {
        Vec::new()
    }

    /// Where it sends the status of a commit.
    fn status_reporters(&self) -> Vec<Arc<dyn StatusReporter>> {
        Vec::new()
    }

    /// How it tells people about builds.
    fn notifiers(&self) -> Vec<Arc<dyn Notifier>> {
        Vec::new()
    }

    /// The kinds of credential it brings.
    fn credential_kinds(&self) -> Vec<Arc<dyn CredentialKind>> {
        Vec::new()
    }

    /// How it brings agents up.
    fn agent_launchers(&self) -> Vec<Arc<dyn AgentLauncher>> {
        Vec::new()
    }

    /// The pipeline files it reads.
    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        Vec::new()
    }

    /// Its addresses in the HTTP API.
    fn routes(&self) -> Vec<Route> {
        Vec::new()
    }

    /// Its subcommands of `raven-ci`.
    fn cli_commands(&self) -> Vec<Arc<dyn CliCommand>> {
        Vec::new()
    }

    /// What it does to every build, before and after it.
    fn build_hooks(&self) -> Vec<Arc<dyn BuildHook>> {
        Vec::new()
    }

    /// The scenarios of Raven Test Studio (the text of `.raventest` files) that test the screens
    /// this plugin puts in the panel: its pages and its tabs on the page of a build. The panel
    /// offers them with its own when the plugin is on, and `raven_ci_painel::e2e` plays them in a
    /// test of the plugin's crate. Keep the files in the plugin's `tests/e2e/` and bring them with
    /// `include_str!`. A plugin with no screen gives none (#919).
    fn scenarios(&self) -> Vec<String> {
        Vec::new()
    }
}

/// An extension point, as the list of plugins names what each offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Point {
    /// Kinds of step.
    Step,
    /// Fetching code.
    Scm,
    /// Finding branches and pull requests.
    BranchSource,
    /// Asking for builds.
    Trigger,
    /// Taking things from a build.
    Publisher,
    /// The status of a commit.
    StatusReporter,
    /// Telling people.
    Notifier,
    /// Kinds of credential.
    CredentialKind,
    /// Bringing agents up.
    AgentLauncher,
    /// Reading pipeline files.
    PipelineFormat,
    /// Addresses of the API.
    Routes,
    /// Subcommands.
    CliCommand,
    /// Screens of the panel.
    Screens,
    /// What is done to every build, before and after it.
    BuildHook,
}

impl Point {
    /// Every point, in the order of the table of the crate's documentation.
    pub const ALL: [Self; 14] = [
        Self::Step,
        Self::Scm,
        Self::BranchSource,
        Self::Trigger,
        Self::Publisher,
        Self::StatusReporter,
        Self::Notifier,
        Self::CredentialKind,
        Self::AgentLauncher,
        Self::PipelineFormat,
        Self::Routes,
        Self::CliCommand,
        Self::Screens,
        Self::BuildHook,
    ];

    /// Its name, as JSON and a `plugin.toml` write it: `branch_source`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Scm => "scm",
            Self::BranchSource => "branch_source",
            Self::Trigger => "trigger",
            Self::Publisher => "publisher",
            Self::StatusReporter => "status_reporter",
            Self::Notifier => "notifier",
            Self::CredentialKind => "credential_kind",
            Self::AgentLauncher => "agent_launcher",
            Self::PipelineFormat => "pipeline_format",
            Self::Routes => "routes",
            Self::CliCommand => "cli_command",
            Self::Screens => "screens",
            Self::BuildHook => "build_hook",
        }
    }

    /// The point of a name such as `branch_source`.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|point| point.name() == text)
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The points `plugin` offers now, in the order of [`Point::ALL`]. It asks each point, so it is
/// for a plugin that started.
pub fn points_of(plugin: &dyn CiPlugin) -> Vec<Point> {
    let mut catalog = Catalog::new();
    plugin.steps(&mut catalog);
    let offers = [
        !catalog.specs().is_empty(),
        !plugin.scms().is_empty(),
        !plugin.branch_sources().is_empty(),
        !plugin.triggers().is_empty(),
        !plugin.publishers().is_empty(),
        !plugin.status_reporters().is_empty(),
        !plugin.notifiers().is_empty(),
        !plugin.credential_kinds().is_empty(),
        !plugin.agent_launchers().is_empty(),
        !plugin.pipeline_formats().is_empty(),
        !plugin.routes().is_empty(),
        !plugin.cli_commands().is_empty(),
        has_screens(plugin),
        !plugin.build_hooks().is_empty(),
    ];
    Point::ALL.into_iter().zip(offers).filter_map(|(point, offered)| offered.then_some(point)).collect()
}

/// Whether `plugin` puts screens in the application: controllers of a frame or of a window, or
/// what it adds to the model with [`Module::extend_model`](ravenrs_app::Module::extend_model) (its
/// views, its items of the menu), which is where the panel reads screens from. The model is the one
/// generated from the plugin's own entities; a plugin whose `extend_model` leaves it as it was puts
/// none. Like [`points_of`], it is for a plugin that started.
pub fn has_screens(plugin: &dyn CiPlugin) -> bool {
    if !plugin.view_controllers().is_empty() || !plugin.window_controllers().is_empty() {
        return true;
    }
    let Ok(generated) = ravenrs_app::generate_model(&plugin.entities()) else { return false };
    let mut extended = generated.clone();
    plugin.extend_model(&mut extended);
    extended != generated
}

#[cfg(test)]
mod tests {
    use ravenrs_app::{ModelNode, Module};

    use super::*;

    struct Quiet;

    impl Module for Quiet {
        fn name(&self) -> &str {
            "quieto"
        }
    }

    impl CiPlugin for Quiet {}

    /// A plugin whose screen is a view it adds to the model.
    struct Painted;

    impl Module for Painted {
        fn name(&self) -> &str {
            "pintado"
        }
        fn extend_model(&self, root: &mut ModelNode) {
            root.children.push(ModelNode::new("Views").child(ModelNode::new("Pintado_View")));
        }
    }

    impl CiPlugin for Painted {}

    #[test]
    fn screens_put_in_the_model_count_as_screens() {
        assert!(!has_screens(&Quiet));
        assert!(points_of(&Quiet).is_empty());
        assert!(has_screens(&Painted));
        assert_eq!(points_of(&Painted), [Point::Screens]);
    }
}
