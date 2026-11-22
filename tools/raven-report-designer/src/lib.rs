//! RavenRS Report Designer: an end-user report designer running on the
//! desktop and in the browser. The designer itself is in `ravenrs-report-designer`.

use ravenrs::prelude::*;
pub use ravenrs_report_designer::Designer;

/// The app, for the native runner and the web entry point.
pub fn app() -> impl View {
    Designer::new()
}

/// The app opening one of the sample reports, by the key the web page gets in `?report=`
/// (`invoice`, `sales`, `sales-10k`, `dashboard`, `customers`); any other key opens the gallery.
pub fn app_with(report: Option<&str>) -> impl View + use<> {
    let designer = Designer::new();
    match report {
        Some(key) => designer.sample(key),
        None => designer,
    }
}

/// The report the address asks for: `report` in a query such as `?report=invoice`.
pub fn report_in_query(query: &str) -> Option<String> {
    query
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == "report")
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
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
            let options = ravenrs::web::WebOptions::new("raven-canvas").follow_system_theme();
            // `designer/?report=invoice` opens that report: the pages of the home fan link here.
            let query = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
            let report = super::report_in_query(&query);
            match ravenrs::web::start(super::app_with(report.as_deref()), options).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }
}
