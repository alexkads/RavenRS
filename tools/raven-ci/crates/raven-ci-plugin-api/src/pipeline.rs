//! Reading a pipeline file: the [`PipelineFormat`] point.

use std::collections::BTreeMap;
use std::fmt;

use ravenrs_app::FieldInfo;
use ravenrs_flows::FlowDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The parameter of a node that names its stage: the core groups the steps of a build by it, in
/// the order the stages first come.
pub const STAGE_KEY: &str = "$stage";

/// The parameter of a node that names the plugin its kind comes from, by the plugin's name: with
/// that plugin switched off, the core refuses the build saying "falta o plugin `shell`" rather
/// than only the missing kind.
pub const PLUGIN_KEY: &str = "$plugin";

/// When the steps of a [`PostSteps`] run, by how the steps of the build ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostWhen {
    /// Whatever happened: first of all.
    Always,
    /// When every step passed.
    Success,
    /// When a step failed or ran out of time.
    Failure,
}

impl PostWhen {
    /// Whether steps of this kind run after steps that ended `passed` (or not).
    pub fn runs_after(self, passed: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Success => passed,
            Self::Failure => !passed,
        }
    }
}

/// Steps a build runs after its own steps, in the same folder and log: cleaning up, telling
/// someone. Their failure is written to the log and does not change how the build ended.
#[derive(Clone, Debug, PartialEq)]
pub struct PostSteps {
    /// When they run.
    pub when: PostWhen,
    /// The steps, as a flow like the pipeline's.
    pub flow: FlowDefinition,
    /// The node they start at.
    pub start: String,
}

/// A point a pipeline names, with its settings: a publisher, a trigger, an agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Use {
    /// The id of the point: `junit`.
    pub id: String,
    /// The plugin it comes from, by name, for the refusal when it is switched off.
    pub plugin: Option<String>,
    /// Its settings.
    pub settings: Value,
}

impl Use {
    /// The point `id` from the plugin `plugin`, with `settings`.
    pub fn new(id: impl Into<String>, plugin: impl Into<String>, settings: Value) -> Self {
        Self { id: id.into(), plugin: Some(plugin.into()), settings }
    }
}

/// A pipeline as a format read it: the flow its builds run and what a build asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct Pipeline {
    /// The steps as a flow of `ravenrs-flows`: each step a node whose kind a plugin brings, with
    /// its stage in [`STAGE_KEY`].
    pub flow: FlowDefinition,
    /// The node a build starts at, with one item holding the values of its parameters.
    pub start: String,
    /// The parameters a build takes, as a form asks for them.
    pub parameters: Vec<FieldInfo>,
    /// The variables every step gets, on top of the build's own (see
    /// [`BuildScope::environment`](crate::BuildScope::environment)).
    pub environment: BTreeMap<String, String>,
    /// The steps that run after the build's, by when.
    pub post: Vec<PostSteps>,
    /// What is taken from the build when its steps end: artifacts, test results.
    pub publishers: Vec<Use>,
    /// The most the steps of a build may take, in milliseconds; when it passes, the build is
    /// cancelled and ends as out of time.
    pub timeout_ms: Option<u64>,
    /// How many builds of the job to keep, when the pipeline says.
    pub keep_builds: Option<u32>,
    /// Where the build runs, when the pipeline says: a label, a container, a pod.
    pub agent: Option<Use>,
}

impl Pipeline {
    /// The pipeline whose steps are `flow`, starting at `start`, with nothing else.
    pub fn new(flow: FlowDefinition, start: impl Into<String>) -> Self {
        Self {
            flow,
            start: start.into(),
            parameters: Vec::new(),
            environment: BTreeMap::new(),
            post: Vec::new(),
            publishers: Vec::new(),
            timeout_ms: None,
            keep_builds: None,
            agent: None,
        }
    }

    /// Every plugin the pipeline needs, by name, with what needs it: the plugins of its steps
    /// (from [`PLUGIN_KEY`]), of its post steps, its publishers and its agent.
    pub fn needs(&self) -> Vec<(String, String)> {
        let mut needs = Vec::new();
        let flows = std::iter::once(&self.flow).chain(self.post.iter().map(|post| &post.flow));
        for flow in flows {
            for node in &flow.nodes {
                if let Some(plugin) = node.parameters.get(PLUGIN_KEY).and_then(Value::as_str) {
                    needs.push((plugin.to_owned(), format!("o passo {}", node.id)));
                }
            }
        }
        for publisher in &self.publishers {
            if let Some(plugin) = &publisher.plugin {
                needs.push((plugin.clone(), format!("o publicador {}", publisher.id)));
            }
        }
        if let Some(agent) = &self.agent
            && let Some(plugin) = &agent.plugin
        {
            needs.push((plugin.clone(), format!("o agente {}", agent.id)));
        }
        needs
    }
}

/// What is wrong with a pipeline file, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError {
    /// The line, counting from 1, when there is one.
    pub line: Option<usize>,
    /// What is wrong, in words.
    pub message: String,
}

impl FormatError {
    /// `message`, at `line`.
    pub fn at(line: usize, message: impl Into<String>) -> Self {
        Self { line: Some(line), message: message.into() }
    }

    /// `message`, about the whole file.
    pub fn whole(message: impl Into<String>) -> Self {
        Self { line: None, message: message.into() }
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "linha {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// A way of reading a pipeline file: `.raven-ci.toml`, a declarative `Jenkinsfile`.
///
/// **Its role.** The core runs flows of `ravenrs-flows`; it reads no file. A format turns the
/// text of a job into a [`Pipeline`].
///
/// **How it works.** When a build is asked for, the core reads the job's text with the format
/// the job names (or the one whose [`file_names`](Self::file_names) has the file's name). Every
/// node kind the flow uses has to be in the catalog of the plugins switched on; one that is not
/// refuses the build saying which step is missing.
///
/// **How to write one.** Make one node per step, with its stage in [`STAGE_KEY`], connected in
/// the order they run, and start at a node of the built-in kind `manual`. Say every error at
/// once, each with its line, rather than stopping at the first.
pub trait PipelineFormat: Send + Sync {
    /// What a job names it by: `toml`.
    fn id(&self) -> &str;

    /// The names of the files it reads: `.raven-ci.toml`.
    fn file_names(&self) -> Vec<String>;

    /// The pipeline of the job `job` in `text`, or every error found.
    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>>;
}
