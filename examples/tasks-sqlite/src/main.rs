//! `cargo run -p quick-app`. The web build starts from `lib.rs`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), ravenrs::native::PlatformError> {
    // A test tool such as Raven Test Studio drives the app with no window (`--e2e-serve`), on
    // data in memory: nothing on disk is opened.
    if std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        let session =
            ravenrs_e2e::E2e::new(tasks_sqlite::tasks().into_view(), ravenrs::prelude::Theme::raven_dark())
                .viewport(ravenrs::prelude::Size::new(1100.0, 720.0), 1.0);
        let hello = ravenrs_e2e_wire::Hello::of("Minhas tarefas SQLite").scenarios(tasks_sqlite::SCENARIOS);
        if let Err(error) =
            ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        {
            eprintln!("o canal de teste parou: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    ravenrs::native::run(
        tasks_sqlite::app(),
        ravenrs::native::WindowOptions::new("Raven Contas").size(1100.0, 720.0),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
