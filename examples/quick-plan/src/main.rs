//! `cargo run --manifest-path examples/quick-plan/Cargo.toml`. The web build starts from `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), on
    // data in memory: nothing on disk is opened.
    if std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let session = ravenrs_e2e::E2e::new(quick_plan::app(), ravenrs::prelude::Theme::raven_dark())
            .viewport(ravenrs::prelude::Size::new(1200.0, 800.0), 1.0);
        let hello = ravenrs_e2e_wire::Hello::of("Raven Plano").scenarios(quick_plan::SCENARIOS);
        if let Err(error) =
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    ravenrs::native::run(
        quick_plan::app(),
        ravenrs::native::WindowOptions::new("Raven Plano").size(1200.0, 800.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
