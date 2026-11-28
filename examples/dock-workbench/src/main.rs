//! Native entry point: `cargo run --manifest-path examples/dock-workbench/Cargo.toml -- editor` (or `ide`, or `chat`).

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), on
    // data in memory: nothing on disk is opened.
    if std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let session = ravenrs_e2e::E2e::new(
            dock_workbench::Workbench::Editor.app(),
            ravenrs::prelude::Theme::raven_dark(),
        )
        .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
        let hello = ravenrs_e2e_wire::Hello::of("Dock workbench").scenarios(dock_workbench::SCENARIOS);
        if let Err(error) =
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    use dock_workbench::Workbench;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let which = std::env::args().nth(1).and_then(|word| Workbench::named(&word)).unwrap_or(Workbench::Editor);
    let options = ravenrs::native::WindowOptions::new(which.title()).size(1440.0, 900.0);
    ravenrs::native::run(which.app(), options)
}

#[cfg(target_arch = "wasm32")]
fn main() {}
