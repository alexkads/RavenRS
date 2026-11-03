//! RavenRS Report Designer: an end-user report designer running on the
//! desktop and in the browser. The designer itself is in `ravenrs-report-designer`.

use ravenrs::prelude::*;
pub use ravenrs_report_designer::Designer;

/// The app, for the native runner and the web entry point.
pub fn app() -> impl View {
    Designer::new()
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    /// Runs automatically when the module is instantiated.
    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            let options = ravenrs::web::WebOptions::new("raven-canvas");
            match ravenrs::web::start(super::app(), options).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }
}
