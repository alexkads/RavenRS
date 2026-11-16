//! `cargo run -p raven-test-studio` opens the Studio.
//!
//! `cargo run -p raven-test-studio -- --self-test` runs the Studio's own test with no window
//! and prints a report; `--run financas` runs the scenarios Raven Finanças brings in its
//! `tests/e2e/`, and `--run <file.raventest|folder>` scenario files kept anywhere, such as a QA
//! team's folder. A Studio installed from scratch brings no test scenarios of its own.
//! `--format json` prints the report for a tool or a coding agent. The exit code is 0 when the
//! scenario passed and 1 when it did not.
//!
//! A scenario that did not change, on a build that did not change, comes back from the
//! Studio's cache at once and says so; `--no-cache` runs it again, and `--clear-cache` forgets
//! every run kept.
//!
//! `--delegate codex` (or `claude`) hands the scenario of `--run` (the self-test by default)
//! to that coding agent, with its cheapest model unless `--model` names another, and checks its
//! verdict by running the scenario here.
//!
//! `--atualizacao` says whether there is a newer Studio (`--format json` for a tool, as the
//! Claude pane asks), and `--atualizar` installs it, printing each stage.

use raven_test_studio::cache::{self, RunCache};

/// Every option the Studio reads; any other is refused.
const OPTIONS: [&str; 16] = [
    "--versao",
    "--project",
    "--run-all",
    "--suggest",
    "--write",
    "--clear-cache",
    "--self-test",
    "--run",
    "--format",
    "--no-cache",
    "--delegate",
    "--model",
    "--atualizacao",
    "--atualizar",
    "--help",
    "--",
];

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    // An option this Studio does not know is refused: opening the window instead would leave
    // whoever asked (a pane, a script) waiting on a window nobody asked for.
    if let Some(unknown) = args.iter().find(|a| a.starts_with("--") && !OPTIONS.contains(&a.as_str())) {
        eprintln!("opção desconhecida: {unknown}; as que o Studio entende: {}", OPTIONS.join(", "));
        return std::process::ExitCode::from(2);
    }
    if has("--help") {
        println!(
            "Raven Test Studio {}\n\n\
             raven-test-studio                       abre a janela\n\
             --self-test                             o Studio testa o Studio, sem janela\n\
             --run <autoteste|financas>              roda o autoteste ou os cenários do Raven Finanças\n\
             --run <arquivo.raventest|pasta>         roda arquivos de cenário, de um app ou do QA\n\
             --project <pasta>                       lê um projeto Rust: apps, telas e cenários RavenRS\n\
             --project <pasta> --run-all             roda os cenários de cada app do projeto\n\
             --project <pasta> --suggest [--write]   a IA local escreve cenários e o Studio os roda\n\
             --suggest --delegate <codex|claude>     o mesmo com o Codex ou o Claude, modelo mais barato\n\
             --format json                           o relatório para uma ferramenta ou uma IA\n\
             --no-cache, --clear-cache               roda de novo / esquece o cache\n\
             --delegate <codex|claude> [--model m]   entrega o cenário a outro agente\n\
             --atualizacao [--format json]           diz se há versão nova\n\
             --atualizar                             instala a versão nova\n\
             --versao                                a versão",
            raven_test_studio::VERSION
        );
        return std::process::ExitCode::SUCCESS;
    }
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
    if has("--atualizacao") {
        let answer = raven_test_studio::atualizacao::ask();
        if value("--format").as_deref() == Some("json") {
            println!("{}", serde_json::to_string_pretty(&answer.json()).unwrap_or_default());
        } else {
            println!("{}", answer.text());
        }
        return std::process::ExitCode::from(if answer.error.is_some() { 2 } else { 0 });
    }
    if has("--atualizar") {
        return update();
    }
    if let Some(folder) = value("--project") {
        let json = value("--format").as_deref() == Some("json");
        let project = match raven_test_studio::project::read(std::path::Path::new(&folder)) {
            Ok(project) => project,
            Err(why) => {
                eprintln!("{why}");
                return std::process::ExitCode::from(2);
            }
        };
        if project.found.is_empty() {
            print_project(&project, json);
            return std::process::ExitCode::from(2);
        }
        // Scenarios that open an app of the project open its code, before any installed copy.
        project.learn_apps();
        if has("--run-all") {
            return run_all(&project, json);
        }
        if has("--suggest") {
            return suggest(&project, value("--delegate"), value("--model"), has("--write"), json);
        }
        if !has("--run") && !has("--self-test") {
            print_project(&project, json);
            return std::process::ExitCode::SUCCESS;
        }
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

/// Installs the newer version now, printing each stage, when this copy was installed by the
/// installer; a checkout updates with `git pull`. 0 when it is up to date afterwards.
fn update() -> std::process::ExitCode {
    use std::io::Write as _;
    let answer = raven_test_studio::atualizacao::ask();
    if let Some(reason) = &answer.error {
        eprintln!("não deu para saber se há versão nova: {reason}");
        return std::process::ExitCode::from(2);
    }
    let Some(news) = answer.latest.filter(|_| answer.newer) else {
        println!("Raven Test Studio {} já é a versão mais nova", answer.installed);
        return std::process::ExitCode::SUCCESS;
    };
    if !answer.updates_itself {
        eprintln!("esta cópia é de um checkout do RavenRS: atualize com git pull");
        return std::process::ExitCode::from(2);
    }
    println!("atualizando de {} para {}", answer.installed, news.versao);
    let said = |stage: String| {
        println!("etapa: {stage}");
        let _ = std::io::stdout().flush();
    };
    match ravenrs_app_update::install_now(&raven_test_studio::versao::APP, &news, &said) {
        Ok(()) => {
            println!("Atualizado para {}", news.versao);
            std::process::ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!("a atualização falhou: {reason}");
            if let Some(tail) = raven_test_studio::versao::APP.installer_log_tail(12) {
                eprintln!("{tail}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

/// Prints what a project has that is made with RavenRS.
fn print_project(project: &raven_test_studio::project::Project, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(&project.json()).unwrap_or_default());
    } else {
        println!("{}", project.text());
    }
}

/// Asks an AI for scenarios for every app of the project that opens on the test channel, and
/// runs each one.
fn suggest(
    project: &raven_test_studio::project::Project,
    agent: Option<String>,
    model: Option<String>,
    write: bool,
    json: bool,
) -> std::process::ExitCode {
    use raven_test_studio::suggest::{self, Writer};
    let writer = match agent.as_deref().map(raven_test_studio::delegate::agent_named) {
        None => Writer::Local,
        Some(Some(agent)) => Writer::Agent(agent, model),
        Some(None) => {
            eprintln!("--delegate aceita codex ou claude");
            return std::process::ExitCode::from(2);
        }
    };
    let apps: Vec<_> = project.found.iter().filter(|f| f.is_app() && f.channel).collect();
    if apps.is_empty() {
        eprintln!("nenhum app do projeto abre pelo canal de teste:\n{}", project.text());
        return std::process::ExitCode::from(2);
    }
    let mut code = 0;
    let mut all = Vec::new();
    for found in apps {
        match suggest::suggest(found, &project.root, &writer, write) {
            Ok(suggestions) => {
                if json {
                    all.push(suggestions.json());
                } else {
                    println!("{}", suggestions.text());
                }
            }
            Err(why) => {
                eprintln!("{}: {why}", found.name);
                code = 2;
            }
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&all).unwrap_or_default());
    }
    std::process::ExitCode::from(code)
}

/// Runs the scenarios of every app of the project that has the test channel and scenarios.
fn run_all(project: &raven_test_studio::project::Project, json: bool) -> std::process::ExitCode {
    let folders = project.scenario_folders();
    if folders.is_empty() {
        eprintln!("nenhum app do projeto tem o canal de teste e cenários em tests/e2e:\n{}", project.text());
        return std::process::ExitCode::from(2);
    }
    let mut worst = 0;
    let mut reports = Vec::new();
    for folder in folders {
        match raven_test_studio::run_files(&folder) {
            Ok(report) => {
                worst = worst.max(report.exit_code());
                reports.push(report);
            }
            Err(why) => {
                eprintln!("{why}");
                worst = 2;
            }
        }
    }
    if json {
        let all: Vec<_> = reports.iter().map(raven_test_studio::report::Report::json).collect();
        println!("{}", serde_json::to_string_pretty(&all).unwrap_or_default());
    } else {
        for report in &reports {
            println!("{}\n", report.text());
        }
    }
    std::process::ExitCode::from(u8::try_from(worst).unwrap_or(1))
}

/// Runs a bundled scenario, or `.raventest` files by path, with no window and prints the report.
fn report(name: &str, json: bool, cache: Option<&mut RunCache>) -> std::process::ExitCode {
    let report = match raven_test_studio::run_named(name, cache) {
        Ok(report) => report,
        Err(why) => {
            eprintln!("{why}");
            return std::process::ExitCode::from(2);
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report.json()).unwrap_or_default());
    } else {
        println!("{}", report.text());
    }
    std::process::ExitCode::from(u8::try_from(report.exit_code()).unwrap_or(1))
}
