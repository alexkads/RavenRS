//! The app on the test channel: what Raven Test Studio drives when it opens Raven Contas with
//! `--e2e-serve`, with no window, on the sample records in memory. It lives apart from the
//! application, which stays under a hundred lines.
#![cfg(not(target_arch = "wasm32"))]

/// The scenarios the app brings, one per file of `tests/e2e/`: the tool gets them when it opens
/// the app, and `tests/scenarios.rs` plays them.
pub const SCENARIOS: [&str; 1] = [include_str!("../tests/e2e/abrir.raventest")];

/// The session a scenario runs on: the whole app, at the size of its window.
pub fn session() -> ravenrs_e2e::E2e {
    ravenrs_e2e::E2e::new(crate::app(), ravenrs::prelude::Theme::raven_dark())
        .viewport(ravenrs::prelude::Size::new(1100.0, 720.0), 1.0)
}

/// Answers the tool when the app was opened with `--e2e-serve`, until it says bye, and then ends
/// the program; opened as usual, it returns and the window opens.
pub fn serve_if_asked() {
    if !std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        return;
    }
    let hello = ravenrs_e2e_wire::Hello::of("Raven Contas").scenarios(SCENARIOS);
    if let Err(error) =
        ravenrs_e2e_wire::serve_filmed(hello, session(), ravenrs::movie::ChannelFilm::default())
    {
        eprintln!("o canal de teste parou: {error}");
        std::process::exit(1);
    }
    std::process::exit(0);
}
