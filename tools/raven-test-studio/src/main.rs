//! `cargo run --manifest-path tools/raven-test-studio/Cargo.toml` opens the Studio.
//!
//! `cargo run --manifest-path tools/raven-test-studio/Cargo.toml -- --self-test` runs the Studio's own test with no window
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
//! `--headed` runs the scenarios with the app's real window on screen, on its test data, with the
//! same blocks and codes: `--slow-mo <ms>` waits after each step, `--step` starts paused (Enter
//! runs one block, c carries on, p pauses, q stops) and `--keep-open on-failure|always` leaves the
//! window to look at the end. With no window is the default.
//!
//! `--atualizacao` says whether there is a newer Studio (`--format json` for a tool, as the
//! Claude pane asks), and `--atualizar` installs it, printing each stage.

use raven_test_studio::cache::{self, RunCache};

/// Every option the Studio reads; any other is refused.
const OPTIONS: [&str; 24] = [
    "--versao",
    "--headed",
    "--headless",
    "--slow-mo",
    "--keep-open",
    "--step",
    "--film",
    "--out",
    "--e2e-serve",
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
             --format jsonl                          um evento por linha, com o tempo de cada passo\n\
             --format junit                          o XML JUnit que servidores de build leem\n\
             --format html                           uma página com o resumo, os passos e a falha\n\
             --film <never|on-failure|always>        filma o app; on-failure guarda só o filme da falha\n\
             --headed                                roda com a janela real do app na tela (padrão: sem janela)\n\
             --slow-mo <ms>                          com a janela: espera entre os passos, para acompanhar\n\
             --step                                  com a janela: começa pausado; Enter = próximo passo,\n\
                                                     c = continuar, p = pausar, q = parar\n\
             --keep-open <never|on-failure|always>   com a janela: deixa-a aberta no fim para inspecionar\n\
             --headless                              sem janela, mesmo que o e2e.toml peça\n\
             --out <pasta>                           grava relatorio.json e .html, trace.jsonl e, na falha,\n\
                                                     arvore.txt (com --project: o target/e2e do projeto)\n\
             --no-cache, --clear-cache               roda de novo / esquece o cache\n\
             --delegate <codex|claude> [--model m]   entrega o cenário a outro agente\n\
             --atualizacao [--format json]           diz se há versão nova\n\
             --atualizar                             instala a versão nova\n\
             --e2e-serve                             outro Studio testa este, sem janela\n\
             --versao                                a versão",
            raven_test_studio::VERSION
        );
        return std::process::ExitCode::SUCCESS;
    }
    let format = value("--format").unwrap_or_else(|| "text".to_owned());
    if !raven_test_studio::report::FORMATS.contains(&format.as_str()) {
        eprintln!(
            "--format não aceita {format:?}; os formatos: {}",
            raven_test_studio::report::FORMATS.join(", ")
        );
        return std::process::ExitCode::from(2);
    }
    let json = format == "json";
    let mut out = value("--out").map(std::path::PathBuf::from);
    let film = match value("--film") {
        None => raven_test_studio::runner::FilmMode::Never,
        Some(word) => match raven_test_studio::runner::FilmMode::read(&word) {
            Some(mode) => mode,
            None => {
                eprintln!(
                    "--film não aceita {word:?}; aceita: {}",
                    raven_test_studio::runner::FilmMode::WORDS.join(", ")
                );
                return std::process::ExitCode::from(2);
            }
        },
    };
    // The window: the command line, over the e2e.toml of the project, over no window.
    let config = match value("--project") {
        Some(folder) => ravenrs_e2e_runner::config::Config::read(std::path::Path::new(&folder)),
        None => Ok(ravenrs_e2e_runner::config::Config::default()),
    };
    let ask = ravenrs_e2e_runner::window::Ask {
        headed: has("--headed"),
        headless: has("--headless"),
        slow_mo: value("--slow-mo"),
        keep_open: value("--keep-open"),
        step: has("--step"),
    };
    let (window, control) = match config.and_then(|config| config.window_from(&ask)) {
        Ok(chosen) => chosen,
        Err(why) => {
            eprintln!("{why}");
            return std::process::ExitCode::from(2);
        }
    };
    // Another Studio drives this one with no window over the test channel, to test it in a
    // process of its own; it brings no scenario.
    if has(ravenrs_e2e_wire::SERVE_FLAG) {
        let session =
            ravenrs_e2e::E2e::new(raven_test_studio::served_app(), ravenrs::prelude::Theme::raven_dark())
                .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
        // Filmed when the Studio testing it asks: the film of a failure shows what happened.
        let hello = ravenrs_e2e_wire::Hello::of("Raven Test Studio");
        return match ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default()) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("o canal de teste parou: {e}");
                std::process::ExitCode::FAILURE
            }
        };
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
        if json {
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
        if (has("--run-all") || has("--run") || has("--suggest") || has("--self-test"))
            && let Err(why) = project.check_configs()
        {
            eprintln!("{why}");
            return std::process::ExitCode::from(2);
        }
        project.learn_apps();
        // A run of a project keeps what it found where cargo keeps what it builds.
        out = out.or_else(|| Some(project.e2e_folder()));
        if has("--run-all") {
            let Some(options) = run_options(film, out, window, control) else {
                return std::process::ExitCode::from(2);
            };
            return run_all(&project, &format, &options);
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
        return delegate(&agent, &name, value("--model"), json);
    }
    if let Some(name) = scenario {
        let mut kept = if has("--no-cache") { None } else { cache::default_file().map(|f| RunCache::at(&f)) };
        let Some(options) = run_options(film, out, window, control) else {
            return std::process::ExitCode::from(2);
        };
        return report(&name, &format, &options, kept.as_mut());
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
    // Its menu says how the self-test went, runs it again and offers an update that is ready.
    let link = raven_test_studio::tray::TrayLink::new(ravenrs::core::TrayMenu::default());
    let chosen = link.clone();
    let tray = ravenrs::native::SystemTray::new("Raven Test Studio")
        .open("Abrir o Raven Test Studio")
        .quit("Sair")
        .menu(link.menu().clone())
        .on_select(move |id| chosen.choose(id));
    let identity = ravenrs::native::AppIdentity::new(raven_test_studio::versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    let identity = match raven_test_studio::app_icon() {
        Some(icon) => identity.icon(icon),
        None => identity,
    };
    match ravenrs::native::run_as(raven_test_studio::installed_app_in_tray(Some(link)), options, identity) {
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
/// What a run does beyond its blocks; `None`, after saying why, when it is asked to film with
/// nowhere to keep the film.
fn run_options(
    film: raven_test_studio::runner::FilmMode,
    out: Option<std::path::PathBuf>,
    window: ravenrs_e2e_runner::window::Window,
    control: Option<ravenrs_e2e_runner::window::Control>,
) -> Option<raven_test_studio::RunOptions> {
    if film != raven_test_studio::runner::FilmMode::Never && out.is_none() {
        eprintln!("--film precisa de onde guardar o filme: diga --out <pasta>, ou rode com --project");
        return None;
    }
    if let Some(control) = &control {
        ravenrs_e2e_runner::window::steer_when_terminal(control);
    }
    Some(raven_test_studio::RunOptions { film, out, window, control })
}

fn run_all(
    project: &raven_test_studio::project::Project,
    format: &str,
    options: &raven_test_studio::RunOptions,
) -> std::process::ExitCode {
    let folders = project.scenario_folders();
    if folders.is_empty() {
        eprintln!("nenhum app do projeto tem o canal de teste e cenários em tests/e2e:\n{}", project.text());
        return std::process::ExitCode::from(2);
    }
    let mut worst = 0;
    let mut reports = Vec::new();
    let mut kept = Vec::new();
    for folder in folders {
        match raven_test_studio::run_files_kept(&folder, options) {
            Ok((report, inspection)) => {
                worst = worst.max(report.exit_code());
                reports.push(report);
                kept.extend(inspection);
            }
            Err(why) => {
                eprintln!("{why}");
                worst = 2;
            }
        }
        if options.control.as_ref().is_some_and(ravenrs_e2e_runner::window::Control::is_stopped) {
            break;
        }
    }
    show(&reports, true, format, options.out.as_deref());
    for inspection in kept {
        inspection.hold();
    }
    std::process::ExitCode::from(u8::try_from(worst).unwrap_or(1))
}

/// Keeps each report in `out`, when given, and prints them all in `format`, as
/// `ravenrs-e2e-runner` does for `cargo ravenrs e2e`.
fn show(
    reports: &[raven_test_studio::report::Report],
    list: bool,
    format: &str,
    out: Option<&std::path::Path>,
) {
    raven_test_studio::report::deliver(reports, list, format, out);
}

/// Runs a bundled scenario, or `.raventest` files by path, with no window and prints the report.
fn report(
    name: &str,
    format: &str,
    options: &raven_test_studio::RunOptions,
    cache: Option<&mut RunCache>,
) -> std::process::ExitCode {
    let (report, inspection) = match raven_test_studio::run_named_kept(name, cache, options) {
        Ok(kept) => kept,
        Err(why) => {
            eprintln!("{why}");
            return std::process::ExitCode::from(2);
        }
    };
    let code = report.exit_code();
    show(std::slice::from_ref(&report), false, format, options.out.as_deref());
    if let Some(inspection) = inspection {
        inspection.hold();
    }
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
