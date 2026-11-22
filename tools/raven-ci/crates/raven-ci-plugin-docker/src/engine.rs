//! The engine: the `docker` or `podman` command, found once, and the commands run with it.

use std::path::PathBuf;
use std::time::Duration;

use ravenrs_flows::NodeError;
use ravenrs_process::{End, Output, Spec};

/// The variables of the server the engine's command gets, besides the basic ones of
/// `ravenrs-process`: where the engine is and where its configuration is.
pub const ENGINE_ENV: &[&str] = &[
    "DOCKER_HOST",
    "DOCKER_CONTEXT",
    "DOCKER_CONFIG",
    "DOCKER_CERT_PATH",
    "DOCKER_TLS_VERIFY",
    "CONTAINER_HOST",
    "XDG_RUNTIME_DIR",
];

/// How long a short question to the engine (does the image exist, which containers) can take.
pub const QUICK: Duration = Duration::from_secs(60);

/// The `docker` or `podman` command the steps run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    /// Where the command is.
    pub program: PathBuf,
    /// Whether it is Podman, which takes `--userns=keep-id` for the agent's user and reads the
    /// login of a registry from `REGISTRY_AUTH_FILE`.
    pub podman: bool,
}

impl Engine {
    /// The engine `setting` names (`docker`, `podman` or a path), or, when it is empty, the
    /// first of `docker` and `podman` in `PATH`.
    pub fn find(setting: &str) -> Result<Self, String> {
        let setting = setting.trim();
        if setting.is_empty() {
            return ["docker", "podman"].iter().find_map(ravenrs_process::resolve).map(Self::of).ok_or_else(
                || "nem docker nem podman no PATH: instale um dos dois na máquina do build".to_owned(),
            );
        }
        ravenrs_process::resolve(setting)
            .map(Self::of)
            .ok_or_else(|| format!("o motor {setting} da configuração do plugin docker não foi encontrado"))
    }

    fn of(program: PathBuf) -> Self {
        let name = program.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        Self { podman: name.contains("podman"), program }
    }

    /// What it is called in messages.
    pub fn name(&self) -> &'static str {
        if self.podman { "podman" } else { "docker" }
    }

    /// The command with `args`, with the variables of [`ENGINE_ENV`] let through.
    pub fn command<I, S>(&self, args: I) -> Spec
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString>,
    {
        ENGINE_ENV.iter().fold(Spec::new(&self.program), |spec, name| spec.allow_env(*name)).args(args)
    }

    /// Runs `args` to the end, at most [`QUICK`], and gives what it said.
    pub fn ask<I, S>(&self, args: I) -> Result<Output, NodeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString>,
    {
        self.command(args)
            .timeout(QUICK)
            .output()
            .map_err(|e| NodeError::new(format!("o {} não rodou: {e}", self.name())))
    }

    /// The lines `args` writes on its output, when it ends well: ids, names.
    pub fn lines<I, S>(&self, args: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString>,
    {
        match self.ask(args) {
            Ok(output) if output.end == End::Code(0) => {
                output.stdout.into_iter().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect()
            }
            _ => Vec::new(),
        }
    }
}

/// `text` as a name an engine takes for a container, a network or a project: small letters,
/// digits and `-`, never empty, never starting with `-`.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() { "x".to_owned() } else { out.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_has_only_small_letters_digits_and_dashes() {
        assert_eq!(slug("apps/Site/PR-12"), "apps-site-pr-12");
        assert_eq!(slug("--/ teste_1 "), "teste-1");
        assert_eq!(slug("///"), "x");
    }

    #[test]
    fn podman_is_told_by_the_name_of_its_program() {
        assert!(Engine::of(PathBuf::from("/usr/bin/podman")).podman);
        assert!(!Engine::of(PathBuf::from("/usr/bin/docker")).podman);
        assert!(Engine::of(PathBuf::from("C:/bin/podman.exe")).podman);
    }
}
