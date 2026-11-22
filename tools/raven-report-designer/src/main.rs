//! Native entry point: `cargo run -p raven-report-designer`.

#[cfg(not(target_arch = "wasm32"))]
mod versao;

/// Builds the designer with no window, as the installer and an agent ask it to. It stays out of
/// `lib.rs`, which `cargo ravenrs new --template relatorios` copies as it is.
#[cfg(not(target_arch = "wasm32"))]
fn self_test() -> Result<String, String> {
    let mut ui =
        ravenrs::prelude::Ui::new(raven_report_designer::app(), ravenrs::prelude::Theme::raven_dark());
    ui.set_viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let nodes = ui.access_tree().nodes.len();
    if nodes < 3 {
        return Err(format!("o designer abriu com {nodes} nós acessíveis"));
    }
    Ok("Autoteste: o designer abre sem janela".to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this, before any window or folder.
    if arguments.iter().any(|a| a == "--versao") {
        println!("{}", versao::VERSION);
        return Ok(());
    }
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), in
    // memory: nothing the person keeps on disk is opened.
    if arguments.iter().any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let session =
            ravenrs_e2e::E2e::new(raven_report_designer::app(), ravenrs::prelude::Theme::raven_dark())
                .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
        let hello =
            ravenrs_e2e_wire::Hello::of("Raven Report Designer").scenarios(raven_report_designer::SCENARIOS);
        if let Err(error) =
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if arguments.iter().any(|a| a == "--self-test") {
        match self_test() {
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
    let tray = ravenrs::native::SystemTray::new("Raven Report Designer")
        .open("Abrir o Raven Report Designer")
        .quit("Sair");
    let identity = ravenrs::native::AppIdentity::new(versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    ravenrs::native::run_as(raven_report_designer::app(), options, identity)
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`); this binary is unused there.
#[cfg(target_arch = "wasm32")]
fn main() {}
