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

/// The designer of the report in `path`, opened from it and saved back to it with Salvar, with no
/// file dialog: what `raven-report-designer relatorio.json` opens, and what a test drives.
#[cfg(not(target_arch = "wasm32"))]
fn designer_of(
    path: std::path::PathBuf,
    on_close: impl Fn() + 'static,
) -> Result<raven_report_designer::Designer, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| format!("não deu para ler {}: {e}", path.display()))?;
    let report = ravenrs_report::Report::from_json(&json)
        .map_err(|e| format!("{} não é um relatório: {e}", path.display()))?;
    let save = move |report: ravenrs_report::Report| {
        if let Err(e) = std::fs::write(&path, report.to_json()) {
            eprintln!("não deu para salvar {}: {e}", path.display());
        }
    };
    Ok(raven_report_designer::Designer::embedded(report, "Salvar", "Fechar", save, on_close))
}

/// The report a test edits: the sample invoice, written once into the folder that
/// `RAVEN_REPORT_DESIGNER_E2E_DIR` names, or into one of this process under the temporary folder.
/// Opened again on the same folder, it is the copy saved before.
#[cfg(not(target_arch = "wasm32"))]
fn e2e_report() -> Result<std::path::PathBuf, String> {
    let folder = std::env::var_os("RAVEN_REPORT_DESIGNER_E2E_DIR").map_or_else(
        || std::env::temp_dir().join(format!("raven-report-designer-e2e-{}", std::process::id())),
        std::path::PathBuf::from,
    );
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let path = folder.join("fatura.json");
    if !path.exists() {
        std::fs::write(&path, ravenrs_report::samples::invoice().to_json())
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this, before any window or folder.
    if arguments.iter().any(|a| a == "--versao") {
        println!("{}", versao::VERSION);
        return Ok(());
    }
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), on a
    // copy of the sample invoice in a folder of its own: nothing the person keeps is opened.
    if arguments.iter().any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let designer = match e2e_report().and_then(|path| designer_of(path, || {})) {
            Ok(designer) => designer,
            Err(why) => {
                eprintln!("{why}");
                std::process::exit(1);
            }
        };
        let session = ravenrs_e2e::E2e::new(designer, ravenrs::prelude::Theme::raven_dark())
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
    // A report file on the command line (or opened with the designer) is edited in place.
    if let Some(file) = arguments.iter().find(|a| !a.starts_with("--")) {
        return match designer_of(std::path::PathBuf::from(file), || std::process::exit(0)) {
            Ok(designer) => ravenrs::native::run_as(designer, options, identity),
            Err(why) => {
                eprintln!("{why}");
                std::process::exit(2);
            }
        };
    }
    ravenrs::native::run_as(raven_report_designer::app(), options, identity)
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`); this binary is unused there.
#[cfg(target_arch = "wasm32")]
fn main() {}
