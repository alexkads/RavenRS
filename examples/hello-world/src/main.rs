//! Native entry point: `cargo run --manifest-path examples/hello-world/Cargo.toml`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), on
    // data in memory: nothing on disk is opened.
    if std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let session = ravenrs_e2e::E2e::new(hello_world::app(), ravenrs::prelude::Theme::raven_dark())
            .viewport(ravenrs::prelude::Size::new(760.0, 560.0), 1.0);
        let hello = ravenrs_e2e_wire::Hello::of("Hello World").scenarios(hello_world::SCENARIOS);
        if let Err(error) =
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let options = ravenrs::native::WindowOptions::new("RavenRS — Hello World").size(760.0, 560.0);
    ravenrs::native::run(hello_world::app(), options)
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`); this binary is unused there.
#[cfg(target_arch = "wasm32")]
fn main() {}
