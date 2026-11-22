//! Reading a pipeline file: the [`PipelineFormat`] point.

use std::fmt;

use ravenrs_app::FieldInfo;
use ravenrs_flows::FlowDefinition;

/// The parameter of a node that names its stage: the core groups the steps of a build by it, in
/// the order the stages first come.
pub const STAGE_KEY: &str = "$stage";

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
