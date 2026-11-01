//! The document viewer: renders the report with its data and shows the pages.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_report::{Document, RenderOptions};

use crate::editor::Editor;

/// The last render, reused while the report and parameters do not change.
#[derive(Default)]
pub struct RenderCache {
    key: Option<(u64, String)>,
    pub doc: Option<Rc<Document>>,
    pub millis: f64,
}

/// Milliseconds from a monotonic clock, on desktop and in the browser.
pub fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::OnceLock;
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 1000.0
    }
}

/// Renders the report if it changed since the last call.
pub fn document(
    editor: &Editor,
    cache: &Rc<RefCell<RenderCache>>,
    params: &HashMap<String, serde_json::Value>,
) -> Rc<Document> {
    let version = editor.version.get();
    let mut sorted: Vec<_> = params.iter().collect();
    sorted.sort_by_key(|(name, _)| *name);
    let key = (version, format!("{sorted:?}"));
    if let Some(doc) = cache.borrow().doc.clone().filter(|_| cache.borrow().key.as_ref() == Some(&key)) {
        return doc;
    }
    let report = editor.report.get();
    let data = report.embedded_data().cloned().unwrap_or(serde_json::Value::Null);
    let options = RenderOptions { parameters: params.clone(), ..RenderOptions::default() };
    let start = now_ms();
    let doc = Rc::new(editor.engine.borrow_mut().render_with(&report, &data, &options));
    let mut c = cache.borrow_mut();
    c.millis = now_ms() - start;
    c.key = Some(key);
    c.doc = Some(doc.clone());
    doc
}

pub use ravenrs_report_export::{ExportFormat as Export, ExportedFile};

/// Exports the document and hands the file to the user: a download on the web,
/// or the program that opens the format on the desktop.
pub fn export(document: &Document, format: Export) {
    match ravenrs_report_export::export(document, format, None) {
        Ok(file) => {
            #[cfg(target_arch = "wasm32")]
            download(&file);
            #[cfg(not(target_arch = "wasm32"))]
            open_on_desktop(&file);
        }
        Err(e) => tracing::warn!(error = %e, "could not export the report"),
    }
}

#[cfg(target_arch = "wasm32")]
fn download(file: &ExportedFile) {
    use wasm_bindgen::JsCast;

    let Some(page) = web_sys::window().and_then(|w| w.document()) else { return };
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(file.bytes.as_slice()));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(file.mime);
    let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options) else { return };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
    if let Some(link) =
        page.create_element("a").ok().and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok())
    {
        link.set_href(&url);
        link.set_download(&file.name);
        link.click();
    }
    web_sys::Url::revoke_object_url(&url).ok();
}

#[cfg(not(target_arch = "wasm32"))]
fn open_on_desktop(file: &ExportedFile) {
    let path = std::env::temp_dir().join(&file.name);
    if let Err(e) = std::fs::write(&path, &file.bytes) {
        tracing::warn!(error = %e, "could not write the exported file");
        return;
    }
    if let Err(e) = ravenrs::native::open_with_system(&path) {
        tracing::warn!(error = %e, "could not open the file");
    }
}

/// Asks the user for a PNG or JPEG and hands it over as a `data:` address. In
/// the browser this is the file dialog; the desktop build has no file dialog
/// yet, so a picture there comes from a field instead.
pub fn pick_image(on_pick: impl FnOnce(String) + 'static) {
    #[cfg(target_arch = "wasm32")]
    pick_image_in_browser(on_pick);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = on_pick;
        tracing::warn!("choosing an image from a file is only available in the browser");
    }
}

#[cfg(target_arch = "wasm32")]
fn pick_image_in_browser(on_pick: impl FnOnce(String) + 'static) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(page) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(input) =
        page.create_element("input").ok().and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    else {
        return;
    };
    input.set_type("file");
    input.set_accept("image/png,image/jpeg");
    let chosen = input.clone();
    let on_change = Closure::once_into_js(move || {
        let Some(file) = chosen.files().and_then(|files| files.get(0)) else { return };
        let Ok(reader) = web_sys::FileReader::new() else { return };
        let done = reader.clone();
        let on_load = Closure::once_into_js(move || {
            if let Some(source) = done.result().ok().and_then(|r| r.as_string()) {
                on_pick(source);
            }
        });
        reader.set_onload(Some(on_load.unchecked_ref()));
        reader.read_as_data_url(&file).ok();
    });
    input.set_onchange(Some(on_change.unchecked_ref()));
    input.click();
}

pub use ravenrs_report_viewer::print;
