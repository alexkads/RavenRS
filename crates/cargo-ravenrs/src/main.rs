//! Cargo entry point for RavenRS project tools.
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "cargo ravenrs", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Create an app with version-matched examples and AI tools.
    New {
        name: PathBuf,
        #[arg(long)]
        template: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        no_git: bool,
        #[arg(long)]
        no_ai: bool,
        #[arg(long)]
        no_check: bool,
        #[arg(long)]
        plugin: bool,
    },
    /// Run the embedded MCP server over stdio.
    Mcp,
    /// Update managed AI configuration in the current project.
    Ai {
        #[arg(long)]
        plugin: bool,
    },
    /// Check versions, AI configuration and the registered MCP server.
    Doctor,
    /// End-to-end scenarios (.raventest) of the project, with no window.
    E2e {
        #[command(subcommand)]
        command: E2eCommand,
    },
}

#[derive(Debug, Subcommand)]
enum E2eCommand {
    /// Install the native test channel in a recognized generated app, preserving custom code.
    Install {
        /// The generated app's folder; the current one by default.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Run the scenarios: every app's tests/e2e, or the file or folder given. Exit code 0
    /// when all passed, 1 when one failed, 2 when nothing could run.
    Run {
        /// A .raventest file or a folder of them.
        path: Option<PathBuf>,
        /// The project's folder; the current one by default.
        #[arg(long)]
        project: Option<PathBuf>,
        /// text, json, jsonl (one event per line), junit (XML for build servers) or html; the
        /// e2e.toml's, else text.
        #[arg(long, default_value = "")]
        format: String,
        /// Where to keep relatorio.json, trace.jsonl and, on a failure, arvore.txt; the
        /// project's target/e2e by default.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Film the apps: never, on-failure (keep only the film of the one that failed) or
        /// always; the e2e.toml's, else never.
        #[arg(long, default_value = "")]
        film: String,
        /// Show the app's real window while the scenarios run (the e2e.toml's window, else none).
        #[arg(long)]
        headed: bool,
        /// No window, even when the e2e.toml asks for it.
        #[arg(long)]
        headless: bool,
        /// With the window: milliseconds to wait after each step, to follow it.
        #[arg(long, value_name = "MS")]
        slow_mo: Option<String>,
        /// With the window: leave it open at the end (never, on-failure or always).
        #[arg(long, value_name = "QUANDO")]
        keep_open: Option<String>,
        /// With the window: start paused; Enter runs one block, c carries on, p pauses, q stops.
        #[arg(long)]
        step: bool,
    },
    /// Print the JSON Schema of the report (report) or of a scenario as JSON (scenario).
    Schema {
        /// report or scenario.
        which: String,
    },
    /// Print a scenario as a Rust integration test of the app it opens, to read or keep.
    Rust {
        /// The .raventest file.
        file: PathBuf,
        /// The project's folder; the current one by default.
        #[arg(long)]
        project: Option<PathBuf>,
        /// The app's binary, when the scenario's app is not found in the project.
        #[arg(long)]
        bin: Option<String>,
    },
    /// List the project's RavenRS packages, apps, test channel, scenarios and screens.
    List {
        /// The project's folder; the current one by default.
        #[arg(long)]
        project: Option<PathBuf>,
        /// As JSON, for a tool.
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "ravenrs") {
        args.remove(1);
    }
    let cli = Cli::parse_from(args);
    let result = match cli.command {
        Commands::Mcp => ravenrs_mcp::serve_stdio().await.map_err(|e| e.to_string()),
        Commands::New { name, template, target, yes, no_git, no_ai, no_check, plugin } => {
            let state = std::fs::read_to_string(name.join(".ravenrs-project.toml"))
                .ok()
                .and_then(|text| text.parse::<toml_edit::DocumentMut>().ok());
            let template_default = state
                .as_ref()
                .and_then(|state| state.get("template"))
                .and_then(toml_edit::Item::as_str)
                .unwrap_or("hello");
            let target_default = state
                .as_ref()
                .and_then(|state| state.get("target"))
                .and_then(toml_edit::Item::as_str)
                .unwrap_or("desktop");
            let template =
                choose(template, yes, "Modelo (hello, app, app-sqlite, relatorios)", template_default);
            let target = choose(target, yes, "Alvo (desktop, web, both)", target_default);
            template.and_then(|template| target.map(|target| (template, target))).and_then(
                |(template, target)| {
                    cargo_ravenrs::new_project(&name, &template, &target, !no_git, !no_ai, !no_check, plugin)
                },
            )
        }
        Commands::Ai { plugin } => cargo_ravenrs::configure_ai(std::path::Path::new("."), plugin),
        Commands::Doctor => cargo_ravenrs::doctor(std::path::Path::new(".")),
        Commands::E2e { command } => std::process::exit(match command {
            E2eCommand::Install { project } => cargo_ravenrs::e2e::install(project.as_deref()),
            E2eCommand::Run {
                path,
                project,
                format,
                out,
                film,
                headed,
                headless,
                slow_mo,
                keep_open,
                step,
            } => {
                let window = ravenrs_e2e_runner::window::Ask { headed, headless, slow_mo, keep_open, step };
                cargo_ravenrs::e2e::run(&cargo_ravenrs::e2e::RunAsk {
                    project,
                    path,
                    format,
                    out,
                    film,
                    window,
                })
            }
            E2eCommand::List { project, json } => cargo_ravenrs::e2e::list(project.as_deref(), json),
            E2eCommand::Schema { which } => cargo_ravenrs::e2e::schema(&which),
            E2eCommand::Rust { file, project, bin } => {
                cargo_ravenrs::e2e::rust(&file, project.as_deref(), bin.as_deref())
            }
        }),
    };
    if let Err(error) = result {
        eprintln!("Erro: {error}");
        std::process::exit(1);
    }
}

fn choose(value: Option<String>, yes: bool, label: &str, default: &str) -> Result<String, String> {
    use std::io::{IsTerminal, Write};
    if let Some(value) = value {
        return Ok(value);
    }
    if yes || !std::io::stdin().is_terminal() {
        return Ok(default.into());
    }
    print!("{label} [{default}]: ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).map_err(|e| e.to_string())?;
    Ok(if answer.trim().is_empty() { default.into() } else { answer.trim().into() })
}
