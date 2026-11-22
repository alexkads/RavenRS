//! `raven-ci servir` and `raven-ci usuario criar`: the server that runs builds by itself, and the
//! people who may reach it.
//!
//! `servir` opens the core with every plugin, watches the triggers the jobs name, starts the
//! executors and answers HTTP with `raven_ci_core::api` until it is told to stop (Ctrl+C, or
//! `SIGTERM` from a service manager), then lets the triggers go and waits for the builds running.

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use raven_ci_core::{Ci, Executors, api};
use raven_ci_plugin_api::{CiError, ROLE_RUN};
use ravenrs_app_server::ApiOptions;

/// Where it listens unless told: this machine only.
pub const DEFAULT_ADDRESS: &str = "127.0.0.1:8080";
/// How many threads answer HTTP at once.
const HTTP_WORKERS: usize = 2;
/// How often an idle executor looks at the queue.
const POLL: Duration = Duration::from_millis(500);

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(e.to_string())
}

/// The value after `flag` in `args`, if it is there.
fn flag<'a>(args: &[&'a str], flag: &str) -> Result<Option<&'a str>, CiError> {
    match args.iter().position(|a| *a == flag) {
        Some(at) => {
            args.get(at + 1).copied().map(Some).ok_or_else(|| CiError::settings(flag, "precisa de um valor"))
        }
        None => Ok(None),
    }
}

/// What stops the server: Ctrl+C or, on Linux and macOS, `SIGTERM`. Made before the server says
/// where it listens, so a signal sent as soon as it starts is not lost; it has to be made inside
/// the runtime.
fn stopping() -> Result<impl std::future::Future<Output = ()>, CiError> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).map_err(failed)?;
        let mut interrupt = signal(SignalKind::interrupt()).map_err(failed)?;
        Ok(async move {
            tokio::select! {
                _ = term.recv() => {}
                _ = interrupt.recv() => {}
            }
        })
    }
    #[cfg(not(unix))]
    {
        Ok(async {
            let _ = tokio::signal::ctrl_c().await;
        })
    }
}

/// `raven-ci servir [--endereco HOST:PORTA] [--executores N]`.
pub fn serve(ci: Ci, data: &Path, args: &[&str], out: &mut dyn Write) -> Result<i32, CiError> {
    let address = flag(args, "--endereco")?.unwrap_or(DEFAULT_ADDRESS).to_owned();
    let executors: usize = match flag(args, "--executores")? {
        Some(n) => n.parse().map_err(|_| CiError::settings("--executores", "tem que ser um número"))?,
        None => 1,
    };
    let ci = Arc::new(ci);
    for problem in ci.watch_triggers() {
        writeln!(out, "aviso: {problem}").map_err(failed)?;
    }
    let running = Executors::start(ci.clone(), executors, POLL);
    let runtime =
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(failed)?;
    let users = api::users_file(data);
    let served: Result<(), CiError> = runtime.block_on(async {
        let stop = stopping()?;
        let listener =
            tokio::net::TcpListener::bind(&address).await.map_err(|e| failed(format!("{address}: {e}")))?;
        let at = listener.local_addr().map_err(failed)?;
        writeln!(out, "Raven CI em http://{at}, com {executors} executor(es)").map_err(failed)?;
        out.flush().map_err(failed)?;
        let options = ApiOptions::new()
            .workers(HTTP_WORKERS)
            .open(api::OPEN_PREFIX)
            .pass_header("x-")
            .pass_header("content-type");
        let made = ci.clone();
        let make = move || api::service(made.clone(), &users);
        tokio::select! {
            result = ravenrs_app_server::serve_with(listener, options, make) => result.map_err(failed),
            () = stop => Ok(()),
        }
    });
    ci.unwatch_triggers();
    running.stop();
    runtime.shutdown_timeout(Duration::from_secs(5));
    served?;
    // Whoever started it may have stopped reading; stopping well does not depend on this line.
    let _ = writeln!(out, "Raven CI parou");
    Ok(0)
}

/// `raven-ci usuario criar NOME [--papel PAPEL]`, with the password on the first line of `input`.
pub fn create_user(
    data: &Path,
    name: &str,
    args: &[&str],
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Result<i32, CiError> {
    let role = flag(args, "--papel")?.unwrap_or(ROLE_RUN);
    let mut password = String::new();
    input.read_line(&mut password).map_err(failed)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        writeln!(out, "a senha vem na primeira linha da entrada: echo SENHA | raven-ci usuario criar {name}")
            .map_err(failed)?;
        return Ok(2);
    }
    std::fs::create_dir_all(data).map_err(failed)?;
    match api::create_user(&api::users_file(data), name, password, role) {
        Ok(role) => {
            writeln!(out, "{name}: criado, com o papel {role}").map_err(failed)?;
            Ok(0)
        }
        Err(why) => {
            writeln!(out, "{why}").map_err(failed)?;
            Ok(2)
        }
    }
}
