//! `cargo run --manifest-path tools/raven-ci/Cargo.toml -p raven-ci-painel -- --servidor http://127.0.0.1:8080`: the panel on the desktop,
//! over a Raven CI that `raven-ci servir` answers.
//!
//! `--abrir build/<job>/<número>` opens on that page (#969); the whole address of the panel, as a
//! notice of a build links to it (`http://ci:8080/painel/#/build/apps%2Fsite/3`), also does, and
//! says the server when `--servidor` does not. The web build starts from `lib.rs`
//! (`#[wasm_bindgen(start)]`); this binary is not used there.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a tool run from the terminal")]

/// Where the panel looks for the server unless told.
const DEFAULT_SERVER: &str = "http://127.0.0.1:8080";
/// Where the server serves the panel, in a pasted address.
const PANEL: &str = "/painel";

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
    let after = |flag: &str| args.iter().position(|a| a == flag).and_then(|at| args.get(at + 1).cloned());
    // The whole address of the panel, pasted from a notice.
    let pasted = args.iter().find(|a| a.starts_with("http") && a.contains(PANEL));
    let server = after("--servidor")
        .or_else(|| pasted.and_then(|url| url.split_once(PANEL).map(|(server, _)| server.to_owned())))
        .or_else(|| std::env::var("RAVEN_CI_SERVIDOR").ok())
        .unwrap_or_else(|| DEFAULT_SERVER.to_owned());
    let asked = after("--abrir").or_else(|| pasted.cloned());
    let mut painel = raven_ci_painel::Painel::new(raven_ci_painel::RemoteClient::http(server), true);
    if let Some(asked) = asked {
        match raven_ci_painel::Pagina::from_path(&asked) {
            Some(page) => painel = painel.abrir(page),
            None => eprintln!("{asked} não é uma página do painel; abrindo os jobs"),
        }
    }
    let options = ravenrs::native::WindowOptions::new("Raven CI").size(1440.0, 900.0);
    let identity = ravenrs::native::AppIdentity::new("raven-ci-painel")
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new");
    ravenrs::native::run_as(raven_ci_painel::app_of(painel), options, identity)
}
