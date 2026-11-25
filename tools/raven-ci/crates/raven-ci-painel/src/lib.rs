//! The panel of Raven CI: the same code on the desktop and in the browser, over the server's API.
//!
//! It is made only of RavenRS components and creates none: what a screen needs that no component
//! does is a gap of the library, filled in the component first. It runs no code of the server: it
//! reads JSON through [`RemoteClient`] and hears what changed on the topic `ci` of the live channel
//! (`/api/live/ci`), falling back to reading the log again while a build runs when the channel is
//! not there.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`endereco`] | The address of each page: `#/build/<job>/<número>` and the others |
//! | [`texto`] | How statuses, durations and times are said |
//! | `ui` | The screens: sign in, jobs, a build, the queue, the pipeline, plugins, credentials, people, and the pages plugins put in |
//! | [`e2e`] | The test session: a real core in memory, answered with no HTTP |
//!
//! The design is the one approved for #803 (D-01): pages T1 to T7 and interactions I1 to I7.

#[cfg(not(target_arch = "wasm32"))]
pub mod e2e;
pub mod endereco;
pub mod texto;
mod ui;

pub use endereco::Endereco;
use ravenrs::prelude::*;
pub use ravenrs_app_remote::RemoteClient;
pub use ui::{Pagina, Painel};

/// The panel's version, the one its news speak of.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The panel over the server `client` talks to, in Portuguese. With `live`, it listens to the
/// server's live channel; without it (the tests), it reads again when asked.
pub fn app(client: RemoteClient, live: bool) -> Localized<Painel> {
    app_of(Painel::new(client, live))
}

/// `painel` in Portuguese, as [`app`] gives it: for one opened on a page or over an address.
pub fn app_of(painel: Painel) -> Localized<Painel> {
    Localized::new(Labels::pt_br(), painel)
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    /// The part of the address after `#`, as the browser has it now.
    fn hash() -> String {
        web_sys::window().and_then(|w| w.location().hash().ok()).unwrap_or_default()
    }

    /// Runs when the module is loaded: the panel talks to the server that served the page, opens
    /// on the page its address names (#969), and keeps the address with the page, so a link,
    /// back and forward work.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        let origin = web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default();
        let client = super::RemoteClient::http(origin);
        let address = super::Endereco::new().on_shown(|page| {
            let wanted = format!("#{}", page.to_path());
            if hash() != wanted
                && let Some(window) = web_sys::window()
            {
                // A new entry in the history: back returns to the page before.
                let _ = window.location().set_hash(&wanted);
            }
        });
        let heard = address.clone();
        let on_change = Closure::<dyn FnMut()>::new(move || {
            if let Some(page) = super::Pagina::from_path(&hash()) {
                heard.ask(page);
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = window.add_event_listener_with_callback("hashchange", on_change.as_ref().unchecked_ref());
        }
        // The listener lives as long as the page.
        on_change.forget();
        let mut painel = super::Painel::new(client, true).endereco(address);
        if let Some(page) = super::Pagina::from_path(&hash()) {
            painel = painel.abrir(page);
        }
        wasm_bindgen_futures::spawn_local(async move {
            let options = ravenrs::web::WebOptions::new("raven-canvas");
            match ravenrs::web::start(super::app_of(painel), options).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    /// Unmounts the panel and lets go of its listeners and GPU resources.
    #[wasm_bindgen]
    pub fn stop() {
        APP.with(|a| a.borrow_mut().take());
    }
}
