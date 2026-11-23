//! What makes Raven CI friendly to sessions of AI and to programs (#941), apart from the server:
//! `esquema`, `validar`, `ia instalar` and `ia` (what a session needs to know, in one page).
//!
//! # Its role
//!
//! A session of AI writes a pipeline, asks for a build and reads why it failed. These commands
//! are what it needs on its own machine, with no server: the JSON Schema of `.raven-ci.toml` to
//! write a pipeline right ([`schema`]), the check of a file before it is applied, with each error
//! on its line, and the installation of the MCP server ([`crate::mcp`]) in Claude Code.
//!
//! # How it works
//!
//! - **`esquema`** prints the JSON Schema of the format `toml`, made from the same lists of
//!   fields its reader takes (`raven_ci_plugin_format_toml::schema`).
//! - **`validar ARQUIVO`** reads the file with the format its name says (`Jenkinsfile`, else
//!   `.raven-ci.toml`), as the server would, and checks that every step is of a plugin that comes
//!   with Raven CI. It ends with 0 when the file is right, 1 with the errors, 6 when the file
//!   cannot be read. With `--format json`: `{arquivo, formato, valido, erros: [{linha, mensagem}]}`.
//! - **`ia instalar`** saves the key (from the standard input, `--chave -`) in [`key_file`], only
//!   its owner reads it, and registers the server `raven-ci mcp` in Claude Code
//!   (`claude mcp add --scope user`) with the address of the server; the key never goes to the
//!   configuration of Claude Code. With `--so-mostrar` it only says what it would do.

use std::io::Write;
use std::path::{Path, PathBuf};

use raven_ci_core::Ci;
use raven_ci_plugin_api::{CiError, PipelineFormat};
use serde_json::{Value, json};

use crate::remote::Exit;

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// Where the key a session uses is kept, when `RAVEN_CI_TOKEN` does not say it: readable only by
/// its owner.
pub fn key_file() -> PathBuf {
    crate::versao::APP.home().join("chave-do-servidor")
}

/// The key in [`key_file`], when there is one.
pub fn saved_key() -> Option<String> {
    std::fs::read_to_string(key_file()).ok().map(|key| key.trim().to_owned()).filter(|key| !key.is_empty())
}

/// Runs `words` when they are one of these commands; the exit code, or `None` for other words.
pub fn run(words: &[String], json: bool, out: &mut dyn Write) -> Option<Result<i32, CiError>> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let json = json || words.windows(2).any(|pair| pair == ["--format", "json"]);
    // The words that are not flags nor the value of one.
    let takes_value = ["--servidor", "--chave", "--format"];
    let plain: Vec<&str> = words
        .iter()
        .enumerate()
        .filter(|(n, word)| !word.starts_with("--") && (*n == 0 || !takes_value.contains(&words[n - 1])))
        .map(|(_, word)| *word)
        .collect();
    Some(match plain.as_slice() {
        ["esquema"] => esquema(out),
        ["validar", file] => validar(Path::new(file), json, out),
        ["ia", "instalar"] => instalar(&words, out),
        ["ia"] => guia(out),
        ["mcp"] => crate::mcp::serve(),
        _ => return None,
    })
}

fn esquema(out: &mut dyn Write) -> Result<i32, CiError> {
    let schema = raven_ci_plugin_format_toml::schema();
    writeln!(out, "{}", serde_json::to_string_pretty(&schema).map_err(failed)?).map_err(failed)?;
    Ok(Exit::Done.code())
}

/// The errors of the pipeline `text`, whose file is called `name`: the format the name says, and
/// every step of a plugin that comes with Raven CI. The format's id and the errors.
pub fn check(name: &str, text: &str) -> Result<(String, Vec<Value>), CiError> {
    let file = Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or(name);
    let format: Box<dyn PipelineFormat> = if file == "Jenkinsfile" {
        Box::new(raven_ci_plugin_format_jenkinsfile::JenkinsfileFormat)
    } else {
        Box::new(raven_ci_plugin_format_toml::TomlFormat)
    };
    let id = format.id().to_owned();
    let pipeline = match format.read("validar", text) {
        Ok(pipeline) => pipeline,
        Err(errors) => {
            let errors = errors.iter().map(|e| json!({ "linha": e.line, "mensagem": e.message })).collect();
            return Ok((id, errors));
        }
    };
    let ci = Ci::in_memory()?;
    for plugin in crate::plugins::builtin() {
        ci.add_plugin(plugin);
    }
    ci.apply_plugins()?;
    let catalog = ci.registry().catalog();
    let errors = pipeline
        .flow
        .nodes
        .iter()
        .filter(|node| catalog.get(&node.kind, node.version).is_none())
        .map(|node| {
            json!({ "linha": null, "mensagem": format!("o passo {} usa {}, que nenhum plugin que vem com o Raven CI oferece", node.id, node.kind) })
        })
        .collect();
    Ok((id, errors))
}

fn validar(file: &Path, json: bool, out: &mut dyn Write) -> Result<i32, CiError> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) => {
            let said = format!("{}: {e}", file.display());
            if json {
                writeln!(
                    out,
                    "{}",
                    json!({ "codigo": Exit::Local.name(), "saida": Exit::Local.code(), "mensagem": said })
                )
                .map_err(failed)?;
            } else {
                writeln!(out, "{said}").map_err(failed)?;
            }
            return Ok(Exit::Local.code());
        }
    };
    let (format, errors) = check(&file.display().to_string(), &text)?;
    if json {
        let said = json!({ "arquivo": file.display().to_string(), "formato": format, "valido": errors.is_empty(), "erros": errors });
        writeln!(out, "{said}").map_err(failed)?;
    } else if errors.is_empty() {
        writeln!(out, "{}: certo ({format})", file.display()).map_err(failed)?;
    } else {
        for error in &errors {
            let at = error["linha"].as_u64().map(|line| format!("linha {line}: ")).unwrap_or_default();
            writeln!(out, "{}: {at}{}", file.display(), error["mensagem"].as_str().unwrap_or_default())
                .map_err(failed)?;
        }
    }
    Ok(if errors.is_empty() { Exit::Done.code() } else { Exit::BuildFailed.code() })
}

/// The value after `flag` in `words`.
fn flag<'w>(words: &[&'w str], flag: &str) -> Option<&'w str> {
    words.iter().position(|word| *word == flag).and_then(|at| words.get(at + 1)).copied()
}

fn instalar(words: &[&str], out: &mut dyn Write) -> Result<i32, CiError> {
    let only_show = words.contains(&"--so-mostrar");
    let server = flag(words, "--servidor")
        .map(str::to_owned)
        .or_else(|| std::env::var("RAVEN_CI_SERVIDOR").ok().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| crate::remote::DEFAULT_SERVER.to_owned());
    let program = std::env::current_exe().map_err(failed)?.display().to_string();
    if flag(words, "--chave") == Some("-") {
        let mut key = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut key).map_err(failed)?;
        let key = key.trim();
        if key.is_empty() {
            writeln!(out, "a chave veio vazia na entrada").map_err(failed)?;
            return Ok(Exit::Usage.code());
        }
        if !only_show {
            save_key(key)?;
            writeln!(out, "chave guardada em {} (só o dono lê)", key_file().display()).map_err(failed)?;
        }
    }
    let add = vec![
        "mcp".to_owned(),
        "add".to_owned(),
        "raven-ci".to_owned(),
        "--scope".to_owned(),
        "user".to_owned(),
        "-e".to_owned(),
        format!("RAVEN_CI_SERVIDOR={server}"),
        "--".to_owned(),
        program.clone(),
        "mcp".to_owned(),
    ];
    let config = json!({ "mcpServers": { "raven-ci": {
        "command": program,
        "args": ["mcp"],
        "env": { "RAVEN_CI_SERVIDOR": server },
    }}});
    writeln!(out, "Claude Code: claude {}", add.join(" ")).map_err(failed)?;
    writeln!(out, "outro cliente de MCP:\n{}", serde_json::to_string_pretty(&config).map_err(failed)?)
        .map_err(failed)?;
    writeln!(
        out,
        "a skill: claude plugin marketplace add alexkads/RavenRS && claude plugin install raven-ci@ravenrs"
    )
    .map_err(failed)?;
    if saved_key().is_none() && std::env::var("RAVEN_CI_TOKEN").is_err() {
        writeln!(
            out,
            "falta a chave: crie uma de programa (POST /api/auth/keys) e guarde com: echo CHAVE | raven-ci ia instalar --chave -"
        )
        .map_err(failed)?;
    }
    if only_show {
        return Ok(Exit::Done.code());
    }
    match ravenrs_process::Spec::new("claude").args(&add).output() {
        Ok(output) if output.end == ravenrs_process::End::Code(0) => {
            writeln!(out, "registrado no Claude Code: raven-ci").map_err(failed)?;
            Ok(Exit::Done.code())
        }
        Ok(output) => {
            writeln!(out, "o claude recusou: {}", output.stderr.join(" ")).map_err(failed)?;
            Ok(Exit::Local.code())
        }
        Err(_) => {
            writeln!(out, "não achei o claude no PATH: registre com o comando ou o JSON acima")
                .map_err(failed)?;
            Ok(Exit::Local.code())
        }
    }
}

/// Keeps `key` in [`key_file`], readable only by its owner.
fn save_key(key: &str) -> Result<(), CiError> {
    let file = key_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(failed)?;
    }
    let _ = std::fs::remove_file(&file);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut handle = options.open(&file).map_err(failed)?;
    handle.write_all(key.as_bytes()).map_err(failed)
}

/// `raven-ci ia`: what a session of AI needs to know, in one page.
fn guia(out: &mut dyn Write) -> Result<i32, CiError> {
    let codes: Vec<String> =
        Exit::ALL.iter().map(|exit| format!("{} {}", exit.code(), exit.name())).collect();
    let text = format!(
        "Raven CI para sessões de IA (docs/ai.md)\n\
         \n\
         - MCP: raven-ci mcp (raven-ci ia instalar registra no Claude Code)\n\
         - um build: raven-ci rodar JOB --esperar > /dev/null; raven-ci resumo JOB --format json\n\
         - por que falhou: raven-ci resumo JOB [NUMERO] (o passo, as últimas linhas, os testes, os diagnósticos)\n\
         - um passo inteiro: raven-ci log JOB NUMERO, só quando o resumo não basta\n\
         - antes de aplicar: raven-ci validar .raven-ci.toml --format json; raven-ci esquema\n\
         - saída: {}\n",
        codes.join(", ")
    );
    out.write_all(text.as_bytes()).map_err(failed)?;
    Ok(Exit::Done.code())
}
