//! `cargo run -p raven-ci-painel -- --servidor http://127.0.0.1:8080`: the panel on the desktop,
//! over a Raven CI that `raven-ci servir` answers. The web build starts from `lib.rs`
//! (`#[wasm_bindgen(start)]`); this binary is not used there.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a tool run from the terminal")]

/// Where the panel looks for the server unless told.
const DEFAULT_SERVER: &str = "http://127.0.0.1:8080";

fn main() -> Result<(), ravenrs::native::PlatformError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this, before any window.
    if args.first().map(String::as_str) == Some("--versao") {
        println!("{}", raven_ci_painel::VERSION);
        return Ok(());
    }
    // A test tool such as Raven Test Studio drives the panel with no window (`--e2e-serve`).
    if raven_ci_painel::e2e::serve_if_asked() {
        return Ok(());
    }
    let server = args
        .iter()
        .position(|a| a == "--servidor")
        .and_then(|at| args.get(at + 1).cloned())
        .or_else(|| std::env::var("RAVEN_CI_SERVIDOR").ok())
        .unwrap_or_else(|| DEFAULT_SERVER.to_owned());
    let client = raven_ci_painel::RemoteClient::http(server);
    let options = ravenrs::native::WindowOptions::new("Raven CI").size(1440.0, 900.0);
    let identity = ravenrs::native::AppIdentity::new("raven-ci-painel")
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new");
    ravenrs::native::run_as(raven_ci_painel::app(client, true), options, identity)
}
