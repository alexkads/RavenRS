//! The plugin `format-jenkinsfile` of Raven CI: a declarative `Jenkinsfile`, read as it is or
//! converted into a `.raven-ci.toml` by `raven-ci converter`.
//!
//! # Its role
//!
//! Moving from Jenkins should not mean writing every job again. This plugin reads the declarative
//! part of a `Jenkinsfile` (the `pipeline { }` with its agent, environment, parameters, options,
//! stages, `when`, `parallel` and `post`) and says the same pipeline in the format of the house. It
//! never guesses: what has no counterpart is listed with its line and why, and the rest is still
//! converted.
//!
//! # How it works
//!
//! - **Reading** is a small reader of the Groovy the declarative pipeline uses: calls with and
//!   without parentheses, named arguments, blocks, the four kinds of strings with their escapes and
//!   interpolation. `script { }` and `expression { }` are kept as text, unread.
//! - **Converting** writes the `.raven-ci.toml`. Every part ends converted, **ignored** (Raven CI
//!   does it alone, like `checkout scm`, or it belongs to the job, like `triggers`), or **not
//!   converted** (Groovy code, `withCredentials`, steps with no counterpart). A part converted
//!   with a difference, like the `timeout` of a stage that becomes the limit of each step, is a
//!   **warning**. Every note goes in comments at the start of the TOML, with the line in the
//!   `Jenkinsfile`.
//! - **`${env.X}` and `${params.X}`** in a `sh` become `${X}`: the core puts each parameter in
//!   the environment of the build, under its name, as Jenkins does. Any other interpolation is
//!   Groovy code and is not converted.
//! - **The flow** is made by the plugin `format-toml` from the TOML: this plugin depends on it
//!   and makes no flow of its own, so the two formats never run a pipeline differently.
//! - **As a format** (`jenkinsfile`, file `Jenkinsfile`), a job reads the `Jenkinsfile` of its
//!   repository straight. With any part not converted, the job is refused with an error on each
//!   line, so a pipeline never runs with a part missing.
//!
//! # How to use it
//!
//! ```text
//! raven-ci converter Jenkinsfile                      # the TOML on the screen
//! raven-ci converter Jenkinsfile --saida .raven-ci.toml
//! raven-ci converter Jenkinsfile --saida .raven-ci.toml --substituir
//! ```
//!
//! It exits with 0 when everything was converted, 1 when a part was not, 2 when asked wrong.

mod convert;
mod groovy;

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use raven_ci_plugin_api::{CiError, CiPlugin, CliCommand, FormatError, Pipeline, PipelineFormat};
use raven_ci_plugin_format_toml::TomlFormat;
use ravenrs_app::{Module, ModuleManifest, VersionRange};

pub use convert::{Conversion, Note, NoteKind, convert};

/// The name of the file the format reads.
pub const FILE_NAME: &str = "Jenkinsfile";

/// The plugin `format-jenkinsfile`.
#[derive(Clone, Copy, Debug, Default)]
pub struct JenkinsfilePlugin;

impl Module for JenkinsfilePlugin {
    fn name(&self) -> &str {
        "format-jenkinsfile"
    }

    fn manifest(&self) -> ModuleManifest {
        let manifest = ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default();
        match VersionRange::compatible("0.1.0") {
            Ok(range) => manifest.requires("format-toml", range),
            Err(_) => manifest,
        }
    }
}

impl CiPlugin for JenkinsfilePlugin {
    fn summary(&self) -> String {
        "O Jenkinsfile declarativo: lido direto ou convertido em .raven-ci.toml, com o que não converte na linha."
            .to_owned()
    }

    fn pipeline_formats(&self) -> Vec<Arc<dyn PipelineFormat>> {
        vec![Arc::new(JenkinsfileFormat)]
    }

    fn cli_commands(&self) -> Vec<Arc<dyn CliCommand>> {
        vec![Arc::new(ConvertCommand)]
    }
}

/// The format `jenkinsfile`. See the [crate](crate).
#[derive(Clone, Copy, Debug, Default)]
pub struct JenkinsfileFormat;

impl PipelineFormat for JenkinsfileFormat {
    fn id(&self) -> &str {
        "jenkinsfile"
    }

    fn file_names(&self) -> Vec<String> {
        vec![FILE_NAME.to_owned()]
    }

    fn read(&self, job: &str, text: &str) -> Result<Pipeline, Vec<FormatError>> {
        let conversion = convert(text);
        let pending: Vec<FormatError> = conversion
            .pending()
            .map(|note| FormatError::at(note.line, format!("não convertido: {} ({})", note.what, note.why)))
            .collect();
        if !pending.is_empty() {
            return Err(pending);
        }
        TomlFormat.read(job, &conversion.toml).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| FormatError::whole(format!("no .raven-ci.toml convertido, {error}")))
                .collect()
        })
    }
}

/// `raven-ci converter ARQUIVO [--saida ARQ] [--substituir]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConvertCommand;

const USAGE: &str = "uso: raven-ci converter ARQUIVO [--saida ARQ] [--substituir]";

impl CliCommand for ConvertCommand {
    fn name(&self) -> &str {
        "converter"
    }

    fn summary(&self) -> String {
        "converte um Jenkinsfile declarativo em .raven-ci.toml, listando o que não converte".to_owned()
    }

    fn run(&self, args: &[String], out: &mut dyn Write) -> Result<i32, CiError> {
        let mut input = None;
        let mut output = None;
        let mut replace = false;
        let mut words = args.iter();
        while let Some(word) = words.next() {
            match word.as_str() {
                "--saida" => match words.next() {
                    Some(path) => output = Some(PathBuf::from(path)),
                    None => return usage(out, "--saida precisa do arquivo"),
                },
                "--substituir" => replace = true,
                flag if flag.starts_with("--") => return usage(out, &format!("não conheço {flag}")),
                path if input.is_none() => input = Some(PathBuf::from(path)),
                extra => return usage(out, &format!("sobrou {extra}")),
            }
        }
        let Some(input) = input else { return usage(out, "falta o Jenkinsfile") };
        let text = std::fs::read_to_string(&input)
            .map_err(|e| CiError::failed(format!("não consegui ler {}: {e}", input.display())))?;
        let conversion = convert(&text);
        let write_error = |e: std::io::Error| CiError::failed(format!("não consegui escrever: {e}"));
        match &output {
            None => out.write_all(conversion.toml.as_bytes()).map_err(write_error)?,
            Some(path) => {
                if path.exists() && !replace {
                    writeln!(out, "{} já existe; use --substituir para escrever por cima", path.display())
                        .map_err(write_error)?;
                    return Ok(1);
                }
                std::fs::write(path, &conversion.toml)
                    .map_err(|e| CiError::failed(format!("não consegui escrever {}: {e}", path.display())))?;
                let pending = conversion.pending().count();
                let others = conversion.notes.len() - pending;
                writeln!(
                    out,
                    "escrevi {}: {pending} não convertido(s), {others} aviso(s) ou ignorado(s)",
                    path.display()
                )
                .map_err(write_error)?;
                for note in &conversion.notes {
                    writeln!(out, "  {note}").map_err(write_error)?;
                }
            }
        }
        Ok(if conversion.complete() { 0 } else { 1 })
    }
}

fn usage(out: &mut dyn Write, why: &str) -> Result<i32, CiError> {
    writeln!(out, "{why}\n{USAGE}").map_err(|e| CiError::failed(format!("não consegui escrever: {e}")))?;
    Ok(2)
}
