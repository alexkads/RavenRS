//! `raven-ci servico instalar|remover|estado` (#883): the server as a service of the system, with
//! the services of `ravenrs-app-update` (systemd with `MemoryMax`, launchd, the Task Scheduler).
//!
//! What runs is `raven-ci --dados PASTA servir --endereco … --executores …`, with the binary the
//! installer put in place (`APP.installed_binary()`) when there is one, else this one. Installing
//! again restarts it, so a new version is the one that runs. The installer does it at the end with
//! `RCI_SERVICO=1 sh install.cmd`.

use std::io::Write;
use std::path::{Path, PathBuf};

use raven_ci_plugin_api::CiError;
use ravenrs_app_update::{ServiceManager, ServiceSpec};
use serde_json::json;

use crate::versao::APP;

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// The value after `flag` in `args`.
fn flag<'a>(args: &[&'a str], flag: &str) -> Result<Option<&'a str>, String> {
    match args.iter().position(|a| *a == flag) {
        Some(at) => args.get(at + 1).copied().map(Some).ok_or_else(|| format!("{flag} precisa de um valor")),
        None => Ok(None),
    }
}

/// The binary the service runs: the installed one, else this one.
fn program() -> Result<PathBuf, CiError> {
    match APP.installed_binary().filter(|path| path.exists()) {
        Some(path) => Ok(path),
        None => std::env::current_exe().map_err(failed),
    }
}

/// The service, from the words after `servico <verbo>`.
fn spec(data: &Path, args: &[&str]) -> Result<ServiceSpec, String> {
    let address = flag(args, "--endereco")?.unwrap_or(crate::server::DEFAULT_ADDRESS);
    let executors = flag(args, "--executores")?.unwrap_or("1");
    executors.parse::<usize>().map_err(|_| "--executores tem que ser um número".to_owned())?;
    let program = program().map_err(|e| e.to_string())?;
    let mut spec = ServiceSpec::for_app(&APP, program)
        .args([
            "--dados".to_owned(),
            data.display().to_string(),
            "servir".to_owned(),
            "--endereco".to_owned(),
            address.to_owned(),
            "--executores".to_owned(),
            executors.to_owned(),
        ])
        .logs(data.join("logs"))
        .system(args.contains(&"--sistema"));
    if let Some(name) = flag(args, "--nome")? {
        spec.label = format!("{}.{name}", APP.bundle_id);
        spec.name = name.to_owned();
    }
    if let Some(limit) = flag(args, "--memoria")? {
        spec = spec.memory_max(limit);
    }
    spec.check()?;
    Ok(spec)
}

/// `raven-ci servico instalar|remover|estado [--memoria 1G] [--endereco HOST:PORTA] [--executores N]
/// [--sistema] [--nome NOME]`.
pub fn run(data: &Path, words: &[&str], json: bool, out: &mut dyn Write) -> Result<i32, CiError> {
    let Some(manager) = ServiceManager::current() else {
        writeln!(out, "este sistema não tem gerenciador de serviços que o Raven CI conheça")
            .map_err(failed)?;
        return Ok(2);
    };
    let (verb, args) = match words {
        [verb, args @ ..] => (*verb, args),
        [] => ("", &[][..]),
    };
    let json_too = json || args.windows(2).any(|pair| pair == ["--format", "json"]);
    let spec = match spec(data, args) {
        Ok(spec) => spec,
        Err(why) => {
            writeln!(out, "{why}").map_err(failed)?;
            return Ok(2);
        }
    };
    let system = manager.system().words();
    match verb {
        "instalar" => match manager.install(&spec) {
            Ok(done) => {
                if json_too {
                    let value = json!({ "file": done.file, "commands": done.commands, "notes": done.notes, "system": system });
                    writeln!(out, "{value:#}").map_err(failed)?;
                } else {
                    writeln!(out, "Raven CI instalado como serviço do {system}: {}", done.file.display())
                        .map_err(failed)?;
                    writeln!(out, "roda: {} {}", spec.program.display(), spec.args.join(" "))
                        .map_err(failed)?;
                    for note in &done.notes {
                        writeln!(out, "aviso: {note}").map_err(failed)?;
                    }
                }
                Ok(0)
            }
            Err(why) => {
                writeln!(out, "não instalei o serviço: {why}").map_err(failed)?;
                Ok(1)
            }
        },
        "remover" => match manager.uninstall(&spec) {
            Ok(_) => {
                writeln!(out, "serviço {} removido", spec.name).map_err(failed)?;
                Ok(0)
            }
            Err(why) => {
                writeln!(out, "não removi o serviço: {why}").map_err(failed)?;
                Ok(1)
            }
        },
        "estado" => match manager.status(&spec) {
            Ok(status) => {
                if json_too {
                    let value = json!({ "installed": status.installed, "running": status.running, "file": status.file, "system": system });
                    writeln!(out, "{value:#}").map_err(failed)?;
                } else {
                    let state = match (status.installed, status.running) {
                        (true, true) => "instalado e rodando",
                        (true, false) => "instalado, parado",
                        (false, true) => "rodando, mas o arquivo sumiu",
                        (false, false) => "não instalado",
                    };
                    writeln!(out, "serviço {} ({system}): {state}", spec.name).map_err(failed)?;
                    writeln!(out, "arquivo: {}", status.file.display()).map_err(failed)?;
                }
                Ok(if status.running { 0 } else { 3 })
            }
            Err(why) => {
                writeln!(out, "{why}").map_err(failed)?;
                Ok(1)
            }
        },
        _ => {
            writeln!(out, "{}", crate::cli::USAGE).map_err(failed)?;
            Ok(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_service_runs_the_server_with_the_data_the_address_and_the_limit() {
        let made = spec(
            Path::new("/srv/ci"),
            &["--memoria", "1G", "--endereco", "0.0.0.0:9000", "--executores", "2", "--nome", "ci-teste"],
        )
        .expect("a spec");
        assert_eq!(
            made.args,
            ["--dados", "/srv/ci", "servir", "--endereco", "0.0.0.0:9000", "--executores", "2"]
        );
        assert_eq!(made.memory_max.as_deref(), Some("1G"));
        assert_eq!((made.name.as_str(), made.label.as_str()), ("ci-teste", "dev.ravenrs.raven-ci.ci-teste"));
        assert!(made.systemd_unit().contains("MemoryMax=1G"));
        assert!(spec(Path::new("/d"), &["--executores", "muitos"]).is_err());
        assert!(spec(Path::new("/d"), &["--memoria", "um-tanto"]).is_err());
    }
}
