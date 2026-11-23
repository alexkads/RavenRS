//! Talking to the program of the model, `raven-ci-explicador` (#942), which runs out of the
//! server's process.
//!
//! # The protocol
//!
//! One line of JSON in, `{"modelo": "qwen3-1.7b-q4", "resumo": {…}}` (the summary of the build,
//! #941), then the input closes; the program writes one line of JSON out:
//! `{"texto": "…", "modelo": "…", "ms": 1234}`, or `{"erro": "…"}`, and ends. The same line goes
//! through either way:
//!
//! - **the socket of systemd** (`socket`): the RPM's `raven-ci-explicador.socket` has `Accept=yes`,
//!   so each connection starts an instance of the program with the connection as its input and
//!   output, its own `MemoryMax` and no network (`PrivateNetwork=yes`);
//! - **a child** (`programa`), when there is no socket: on a desktop, in a test.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use raven_ci_plugin_api::{CiError, Host};
use serde_json::{Value, json};

/// The model unless the settings say.
pub const MODEL: &str = "qwen3-1.7b-q4";
/// The socket of the RPM's `raven-ci-explicador.socket`.
pub const SOCKET: &str = "/run/raven-ci/explicador.sock";
/// The program unless the settings say.
pub const PROGRAM: &str = if cfg!(windows) { "raven-ci-explicador.exe" } else { "raven-ci-explicador" };

/// What the program wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    /// The explanation.
    pub text: String,
    /// The model that wrote it.
    pub model: String,
    /// How long it took, in milliseconds, as the program measured it.
    pub ms: u64,
}

fn text<'s>(settings: &'s Value, field: &str) -> Option<&'s str> {
    settings.get(field).and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty())
}

/// The program beside the server's: `raven-ci-explicador` in the folder of `raven-ci`.
fn program_beside() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(PROGRAM)))
        .unwrap_or_else(|| PathBuf::from(PROGRAM))
}

/// Asks the program to explain `summary`, by the socket or as a child as `settings` say. See the
/// [module](self).
pub fn explain(host: &dyn Host, settings: &Value, summary: &Value) -> Result<Made, CiError> {
    let model = text(settings, "modelo").unwrap_or(MODEL).to_owned();
    let request = json!({ "modelo": model, "resumo": summary }).to_string();
    let socket = PathBuf::from(text(settings, "socket").unwrap_or(SOCKET));
    let started = Instant::now();
    let said = if socket.exists() {
        host.log(&format!("explicando pelo socket {}", socket.display()));
        through_socket(&socket, &request, crate::deadline(settings))?
    } else {
        let program = text(settings, "programa").map_or_else(program_beside, PathBuf::from);
        let folder = match text(settings, "pasta") {
            Some(folder) => PathBuf::from(folder),
            None => host.data_dir()?.join("modelos"),
        };
        host.log(&format!("explicando com {}", program.display()));
        as_child(&program, &folder, &request, crate::deadline(settings))?
    };
    let answer: Value = serde_json::from_str(said.trim()).map_err(|_| {
        CiError::failed(format!("o explicador respondeu o que não é JSON: {}", said.chars().take(200).collect::<String>()))
    })?;
    if let Some(error) = answer.get("erro").and_then(Value::as_str) {
        return Err(CiError::failed(format!("o explicador não explicou: {error}")));
    }
    let text = answer.get("texto").and_then(Value::as_str).unwrap_or_default().trim().to_owned();
    if text.is_empty() {
        return Err(CiError::failed("o explicador não escreveu nada"));
    }
    let ms = answer
        .get("ms")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    let model = answer.get("modelo").and_then(Value::as_str).map_or(model, str::to_owned);
    Ok(Made { text, model, ms })
}

#[cfg(unix)]
fn through_socket(socket: &Path, request: &str, deadline: std::time::Duration) -> Result<String, CiError> {
    let failed = |e: std::io::Error| CiError::failed(format!("o socket {}: {e}", socket.display()));
    let mut stream = std::os::unix::net::UnixStream::connect(socket).map_err(failed)?;
    stream.set_read_timeout(Some(deadline)).map_err(failed)?;
    stream.write_all(request.as_bytes()).map_err(failed)?;
    stream.write_all(b"\n").map_err(failed)?;
    stream.shutdown(std::net::Shutdown::Write).map_err(failed)?;
    let mut said = String::new();
    stream.read_to_string(&mut said).map_err(failed)?;
    Ok(said)
}

#[cfg(not(unix))]
fn through_socket(socket: &Path, _: &str, _: std::time::Duration) -> Result<String, CiError> {
    Err(CiError::unavailable(format!("o socket {} só existe com o systemd", socket.display())))
}

fn as_child(program: &Path, folder: &Path, request: &str, deadline: std::time::Duration) -> Result<String, CiError> {
    let running = ravenrs_process::Spec::new(program.as_os_str())
        .args(["--pasta".into(), folder.as_os_str().to_owned()])
        .stdin()
        .timeout(deadline)
        .spawn()
        .map_err(|e| {
            CiError::unavailable(format!(
                "o programa {} não rodou ({e}): instale o raven-ci-explicador, ou diga onde ele está em programa",
                program.display()
            ))
        })?;
    let control = running.control();
    control.send(request);
    control.close();
    let output = running.wait();
    if output.end == ravenrs_process::End::TimedOut {
        return Err(CiError::failed(format!("o explicador passou do prazo de {} s", deadline.as_secs())));
    }
    Ok(output.stdout.join("\n"))
}
