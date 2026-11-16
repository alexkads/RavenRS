//! `cargo run -p raven-test-studio` opens the Studio.
//!
//! `cargo run -p raven-test-studio -- --self-test` runs the Studio's own test with no window
//! and prints a report; `--run <autoteste|lancamento|quebrado>` runs another scenario that
//! comes with it, and `--format json` prints the report for a tool or a coding agent. The exit
//! code is 0 when the scenario passed and 1 when it did not.
//!
//! A scenario that did not change, on a build that did not change, comes back from the
//! Studio's cache at once and says so; `--no-cache` runs it again, and `--clear-cache` forgets
//! every run kept.
//!
//! `--delegate codex` (or `claude`) hands the scenario of `--run` (the self-test by default)
//! to that coding agent, with its cheapest model unless `--model` names another, and checks its
//! verdict by running the scenario here.

use raven_test_studio::cache::{self, RunCache};

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    // The installer proves a new binary opens with this, before any window or folder.
    if has("--versao") {
        println!("{}", raven_test_studio::VERSION);
        return std::process::ExitCode::SUCCESS;
    }
    if has("--clear-cache") {
        if let Some(file) = cache::default_file()
            && let Err(e) = RunCache::at(&file).clear()
        {
            eprintln!("não deu para limpar o cache: {e}");
            return std::process::ExitCode::FAILURE;
        }
        println!("cache do Studio limpo");
        return std::process::ExitCode::SUCCESS;
    }
    let scenario = if has("--self-test") { Some("autoteste".to_owned()) } else { value("--run") };
    if let Some(agent) = value("--delegate") {
        let name = scenario.unwrap_or_else(|| "autoteste".to_owned());
        return delegate(&agent, &name, value("--model"), value("--format").as_deref() == Some("json"));
    }
    if let Some(name) = scenario {
        let mut kept = if has("--no-cache") { None } else { cache::default_file().map(|f| RunCache::at(&f)) };
        return report(&name, value("--format").as_deref() == Some("json"), kept.as_mut());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = ravenrs::native::WindowOptions::new("Raven Test Studio").size(1440.0, 900.0);
    // The window's id is the package, as the installer names the .desktop file and the icon.
    // Closing the window keeps the Studio beside the clock, as Raven Finanças does: opening it
    // again brings the same window back, and launching it twice opens the copy already running.
    let tray =
        ravenrs::native::SystemTray::new("Raven Test Studio").open("Abrir o Raven Test Studio").quit("Sair");
    let identity = ravenrs::native::AppIdentity::new(raven_test_studio::versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    let identity = match raven_test_studio::app_icon() {
        Some(icon) => identity.icon(icon),
        None => identity,
    };
    match ravenrs::native::run_as(raven_test_studio::installed_app(), options, identity) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Hands a scenario to a coding agent, with its cheapest model unless `model` says otherwise,
/// checks its verdict by running the scenario here, and prints both. The exit code is 0 only
/// when the scenario passed and the agent said so.
fn delegate(agent: &str, name: &str, model: Option<String>, json: bool) -> std::process::ExitCode {
    use raven_test_studio::delegate;
    let Some(agent) = delegate::agent_named(agent) else {
        eprintln!("--delegate aceita codex ou claude, não {agent:?}");
        return std::process::ExitCode::from(2);
    };
    match delegate::delegate(&ravenrs_agent_cli::Cli::new(agent), name, model) {
        Ok(done) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&done.json()).unwrap_or_default());
            } else {
                println!("{}", done.text());
            }
            let good = done.checked.passed() && done.confirmed();
            if good { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
        }
        Err(why) => {
            eprintln!("{why}");
            std::process::ExitCode::from(2)
        }
    }
}

/// Runs a bundled scenario with no window and prints its report.
fn report(name: &str, json: bool, cache: Option<&mut RunCache>) -> std::process::ExitCode {
    let Some(report) = raven_test_studio::run_bundled(name, cache) else {
        let known: Vec<&str> = raven_test_studio::SCENARIOS.iter().map(|(key, _)| *key).collect();
        eprintln!("não há cenário {name:?}; os que vêm com o Studio: {}", known.join(", "));
        return std::process::ExitCode::from(2);
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report.json()).unwrap_or_default());
    } else {
        println!("{}", report.text());
    }
    std::process::ExitCode::from(u8::try_from(report.exit_code()).unwrap_or(1))
}
