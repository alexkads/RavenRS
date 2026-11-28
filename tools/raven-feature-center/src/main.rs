//! `cargo run --manifest-path tools/raven-feature-center/Cargo.toml`, with the parameters of [`raven_feature_center::Opening`]:
//! `-- --modo vitrine`, `-- --modo componentes --pagina CablePlugDemo --incorporado`,
//! `-- --tema claro --idioma en`, and `-- caixa` for the counter window on its own. The web build
//! starts from `lib.rs` and reads the same names from the address of the page. `--versao` prints
//! the version and `--self-test` opens every way with no window.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this, before any window or folder.
    if arguments.iter().any(|a| a == "--versao") {
        println!("{}", raven_feature_center::VERSION);
        return Ok(());
    }
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), in
    // memory: nothing the person keeps on disk is opened.
    if arguments.iter().any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let session = ravenrs_e2e::E2e::new(
            raven_feature_center::view(
                &raven_feature_center::Opening::from_arguments(["--modo".to_owned(), "vitrine".to_owned()])
                    .unwrap_or_default(),
            ),
            ravenrs::prelude::Theme::raven_dark(),
        )
        .viewport(ravenrs::prelude::Size::new(1400.0, 900.0), 1.0);
        let hello =
            ravenrs_e2e_wire::Hello::of("Raven Feature Center").scenarios(raven_feature_center::SCENARIOS);
        if let Err(error) =
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if arguments.iter().any(|a| a == "--self-test") {
        match raven_feature_center::self_test() {
            Ok(said) => println!("{said}"),
            Err(why) => {
                eprintln!("o autoteste falhou: {why}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let opening = match raven_feature_center::Opening::from_arguments(arguments) {
        Ok(opening) => opening,
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(2);
        }
    };
    let options = ravenrs::native::WindowOptions::new("Raven Feature Center")
        .size(1400.0, 900.0)
        .theme(raven_feature_center::window_theme(&opening));
    // Closing the window keeps the center beside the clock, as the other tools do; launching it
    // again opens the copy already running.
    let tray = ravenrs::native::SystemTray::new("Raven Feature Center")
        .open("Abrir o Raven Feature Center")
        .quit("Sair");
    let identity = ravenrs::native::AppIdentity::new(raven_feature_center::versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    ravenrs::native::run_as(raven_feature_center::view(&opening), options, identity)
}

#[cfg(target_arch = "wasm32")]
fn main() {}
