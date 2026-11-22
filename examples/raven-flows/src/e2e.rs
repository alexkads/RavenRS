//! The app on the test channel: what Raven Test Studio drives when it opens Raven Flows with
//! `--e2e-serve`, with no window, in a folder of its own under the system's temporary folder, so
//! the flows and runs of the person using the app are never opened.
#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

/// The scenarios the app brings, one per file of `tests/e2e/`: the tool gets them when it opens
/// the app, and `tests/scenarios.rs` plays them.
pub const SCENARIOS: [&str; 1] = [include_str!("../tests/e2e/abrir.raventest")];

/// A folder of its own for one session, emptied, under the system's temporary folder.
pub fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("raven-flows-e2e-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// The session a scenario runs on: the whole app over `folder`, with runs done at once on the
/// calling thread, so a step is over when its click is.
pub fn session(folder: &std::path::Path) -> Result<ravenrs_e2e::E2e, String> {
    let app = crate::app_in(folder, crate::Worker::Inline)?;
    Ok(ravenrs_e2e::E2e::new(app, ravenrs::prelude::Theme::raven_dark())
        .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0))
}

/// Answers the tool when the app was opened with `--e2e-serve`, until it says bye; `false` when
/// the app was opened as usual.
pub fn serve_if_asked() -> bool {
    if !std::env::args().skip(1).any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        return false;
    }
    let hello = ravenrs_e2e_wire::Hello::of("Raven Flows").scenarios(SCENARIOS);
    let served = session(&folder("canal")).map_err(std::io::Error::other).and_then(|session| {
        ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
    });
    if let Err(error) = served {
        eprintln!("o canal de teste parou: {error}");
        std::process::exit(1);
    }
    true
}
