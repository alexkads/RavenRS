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
            let template = choose(template, yes, "Modelo (hello, app, app-sqlite, relatorios)", "hello");
            let target = choose(target, yes, "Alvo (desktop, web, both)", "desktop");
            template.and_then(|template| target.map(|target| (template, target))).and_then(
                |(template, target)| {
                    cargo_ravenrs::new_project(&name, &template, &target, !no_git, !no_ai, !no_check, plugin)
                },
            )
        }
        Commands::Ai { plugin } => cargo_ravenrs::configure_ai(std::path::Path::new("."), plugin),
        Commands::Doctor => cargo_ravenrs::doctor(std::path::Path::new(".")),
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
