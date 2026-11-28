//! Subcommands a plugin adds to `raven-ci`: the [`CliCommand`] point.

use std::io::Write;

use crate::CiError;

/// A subcommand of `raven-ci`: `raven-ci converter Jenkinsfile`.
///
/// **Its role.** The commands of the core are few (`plugins`, `--versao`, `--self-test`); a plugin
/// adds what belongs to it, such as the converter of the format it reads.
///
/// **How it works.** `raven-ci <name> <args…>` runs the command of the plugin switched on whose
/// [`name`](Self::name) it is, after the plugin started; `raven-ci` with no command lists every
/// command with its [`summary`](Self::summary). The code it returns is the exit code. The core
/// refuses a name another plugin or the core took.
///
/// **How to write one.** Write to `out`, not to the terminal, so tests and agents read it.
/// Return 0 when it did what was asked, 1 when it failed, 2 when it was asked wrong.
pub trait CliCommand: Send + Sync {
    /// What follows `raven-ci`: `converter`.
    fn name(&self) -> &str;

    /// One line for the list of commands.
    fn summary(&self) -> String;

    /// Runs with `args`, the words after its name, writing to `out`; the exit code.
    fn run(&self, args: &[String], out: &mut dyn Write) -> Result<i32, CiError>;
}
