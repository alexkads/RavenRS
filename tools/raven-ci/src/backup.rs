//! `raven-ci backup criar|ver|restaurar`: the folder of data in a backup and back (#917). The
//! work is `raven_ci_core::backup`; this says it to a person, or in JSON.

use std::io::Write;
use std::path::{Path, PathBuf};

use raven_ci_core::backup::{self, BACKUPS_DIR, Manifest};
use raven_ci_plugin_api::CiError;
use serde_json::json;

use crate::saida::Exit;

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// The date and time of now in UTC, as a name of a folder: `2026-11-23-161502`.
fn stamp_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}-{:02}{:02}{:02}", rest / 3600, rest % 3600 / 60, rest % 60)
}

fn total(manifest: &Manifest) -> u64 {
    manifest.arquivos.iter().map(|file| file.tamanho).sum()
}

/// Runs `backup <args>` on the folder of data `data`; the exit code.
pub fn run(data: &Path, args: &[&str], json: bool, out: &mut dyn Write) -> Result<i32, CiError> {
    let json = json || args.windows(2).any(|pair| pair == ["--format", "json"]);
    let words: Vec<&str> =
        args.iter().copied().filter(|a| !matches!(*a, "--format" | "json" | "--com-chave")).collect();
    let with_key = args.contains(&"--com-chave");
    let said = match words.as_slice() {
        ["criar"] => criar(data, &data.join(BACKUPS_DIR).join(format!("raven-ci-{}", stamp_now())), with_key),
        ["criar", folder] => criar(data, Path::new(folder), with_key),
        ["ver", folder] => {
            backup::inspect(Path::new(folder)).map(|manifest| (manifest, PathBuf::from(folder), None))
        }
        ["restaurar", folder] => backup::restore(data, Path::new(folder)).map(|restored| {
            let note = Some(json!({ "antes": restored.before, "chave_mantida": restored.kept_key }));
            (restored.manifest, PathBuf::from(folder), note)
        }),
        _ => {
            writeln!(out, "uso: raven-ci backup criar [PASTA] [--com-chave] | ver PASTA | restaurar PASTA")
                .map_err(failed)?;
            return Ok(Exit::Usage.code());
        }
    };
    let (manifest, folder, restored) = match said {
        Ok(said) => said,
        Err(e) => {
            if json {
                writeln!(out, "{}", json!({ "codigo": "recusado", "saida": 1, "mensagem": e.to_string() }))
                    .map_err(failed)?;
            } else {
                writeln!(out, "{e}").map_err(failed)?;
            }
            return Ok(1);
        }
    };
    if json {
        let mut said = json!({ "pasta": folder, "manifesto": manifest });
        if let Some(restored) = restored {
            said["restaurado"] = restored;
        }
        writeln!(out, "{said:#}").map_err(failed)?;
        return Ok(0);
    }
    let size = total(&manifest);
    let key = if manifest.com_chave { "com a chave do cofre" } else { "sem a chave do cofre" };
    let verb = match words.first() {
        Some(&"criar") => "backup feito em",
        Some(&"restaurar") => "restaurado de",
        _ => "backup em",
    };
    writeln!(
        out,
        "{verb} {}: {} arquivos, {:.1} MB, {key} (Raven CI {})",
        folder.display(),
        manifest.arquivos.len(),
        size as f64 / 1_048_576.0,
        manifest.versao
    )
    .map_err(failed)?;
    if let Some(restored) = restored {
        if let Some(before) = restored["antes"].as_str() {
            writeln!(out, "o que havia ficou em {before}").map_err(failed)?;
        }
        if restored["chave_mantida"] == true {
            writeln!(out, "o backup não tinha a chave do cofre: a chave que havia ficou").map_err(failed)?;
        }
    } else if !manifest.com_chave && words.first() == Some(&"criar") {
        writeln!(
            out,
            "guarde a chave do cofre (cofre.chave) à parte: sem ela, as credenciais do backup não abrem"
        )
        .map_err(failed)?;
    }
    Ok(0)
}

fn criar(
    data: &Path,
    folder: &Path,
    with_key: bool,
) -> Result<(Manifest, PathBuf, Option<serde_json::Value>), CiError> {
    backup::create(data, folder, with_key, crate::versao::VERSION)
        .map(|manifest| (manifest, folder.to_owned(), None))
}
