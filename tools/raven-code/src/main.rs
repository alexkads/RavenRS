//! Native entry point: `cargo run -p raven-code -- <folder>`, or with no folder for the sample.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this, before any window or folder.
    if arguments.iter().any(|a| a == "--versao") {
        println!("{}", raven_code::versao::VERSION);
        return Ok(());
    }
    // A test tool such as Raven Test Studio drives the editor with no window (`--e2e-serve`), on
    // the sample project in a folder of its own: nothing the person keeps is opened.
    if arguments.iter().any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let hello = ravenrs_e2e_wire::Hello::of("Raven Code").scenarios(raven_code::SCENARIOS);
        let served = raven_code::e2e_session("canal").and_then(|session| {
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        });
        if let Err(error) = served {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if arguments.iter().any(|a| a == "--self-test") {
        match raven_code::self_test() {
            Ok(said) => println!("{said}"),
            Err(why) => {
                eprintln!("o autoteste falhou: {why}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let project = raven_code::from_args(arguments.into_iter())?;
    let title = format!("{} — Raven Code", project.name());
    let options = ravenrs::native::WindowOptions::new(title).size(1440.0, 900.0);
    let mut app = raven_code::RavenCode::new(project).with_language_server();
    if let Some(config) = raven_code::config::Config::of_user() {
        app = app.remembering(config);
    }
    // Closing the window keeps the editor beside the clock, as the other tools do.
    let tray = ravenrs::native::SystemTray::new("Raven Code").open("Abrir o Raven Code").quit("Sair");
    let identity = ravenrs::native::AppIdentity::new(raven_code::versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    ravenrs::native::run_as(app, options, identity)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
