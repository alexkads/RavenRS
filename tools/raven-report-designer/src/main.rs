//! Native entry point: `cargo run -p raven-report-designer`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this, before any window or folder.
    if arguments.iter().any(|a| a == "--versao") {
        println!("{}", raven_report_designer::versao::VERSION);
        return Ok(());
    }
    if arguments.iter().any(|a| a == "--self-test") {
        match raven_report_designer::self_test() {
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
    let options = ravenrs::native::WindowOptions::new("Raven Report Designer")
        .size(1440.0, 900.0)
        .follow_system_theme();
    // Closing the window keeps the designer beside the clock, as the other tools do.
    let tray = ravenrs::native::SystemTray::new("Raven Report Designer").open("Abrir o Raven Report Designer").quit("Sair");
    let identity = ravenrs::native::AppIdentity::new(raven_report_designer::versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    ravenrs::native::run_as(raven_report_designer::app(), options, identity)
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`); this binary is unused there.
#[cfg(target_arch = "wasm32")]
fn main() {}
