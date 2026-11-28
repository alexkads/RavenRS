//! `raven-ci aplicar ARQUIVO [--remover]` (#898): the configuration of the server as code, applied
//! to the folder of data when no server is named. With `--servidor URL` (or `RAVEN_CI_SERVIDOR`)
//! it goes to the server running instead ([`crate::remote`]), which watches the triggers of what
//! changed at once.

use std::io::Write;
use std::path::Path;

use raven_ci_core::Ci;
use raven_ci_core::config::ServerConfig;
use raven_ci_plugin_api::CiError;
use serde_json::json;

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// Whether `aplicar` with these words goes to a server: `--servidor`, or `RAVEN_CI_SERVIDOR` set.
pub fn to_server(words: &[String]) -> bool {
    words.iter().any(|word| word == "--servidor")
        || std::env::var("RAVEN_CI_SERVIDOR").is_ok_and(|server| !server.trim().is_empty())
}

/// `raven-ci aplicar ARQUIVO [--remover]` on the folder of data of `ci`.
pub fn local(ci: &Ci, args: &[&str], json: bool, out: &mut dyn Write) -> Result<i32, CiError> {
    let remove = args.contains(&"--remover");
    let json = json || args.windows(2).any(|pair| pair == ["--format", "json"]);
    let Some(file) = args.iter().find(|a| !a.starts_with("--") && **a != "json") else {
        writeln!(out, "diga o arquivo: raven-ci aplicar raven-ci.toml [--remover]").map_err(failed)?;
        return Ok(2);
    };
    let config = match ServerConfig::read(Path::new(file)) {
        Ok(config) => config,
        Err(why) => {
            writeln!(out, "{why}").map_err(failed)?;
            return Ok(2);
        }
    };
    match ci.apply_config(&config, remove) {
        Ok(changes) => {
            write_changes(out, &changes, json)?;
            Ok(0)
        }
        Err(why) => {
            writeln!(out, "{why}").map_err(failed)?;
            Ok(2)
        }
    }
}

/// What applying changed, one per line, or as JSON.
pub fn write_changes(out: &mut dyn Write, changes: &[String], json: bool) -> Result<(), CiError> {
    if json {
        writeln!(out, "{:#}", json!({ "changes": changes })).map_err(failed)
    } else if changes.is_empty() {
        writeln!(out, "nada mudou").map_err(failed)
    } else {
        changes.iter().try_for_each(|change| writeln!(out, "{change}").map_err(failed))
    }
}
