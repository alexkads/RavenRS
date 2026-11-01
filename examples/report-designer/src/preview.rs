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

/// A format the document can be exported to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Export {
    /// PDF, vector pages with embedded fonts.
    Pdf,
    /// XLSX, with real numbers and dates in the cells.
    Xlsx,
    /// CSV, with the text as printed.
    Csv,
}

/// A file ready to be saved.
#[derive(Debug)]
pub struct ExportedFile {
    /// The suggested file name.
    pub name: String,
    /// The media type (browsers need it for the download).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub mime: &'static str,
    /// The content.
    pub bytes: Vec<u8>,
}

/// Writes the document in `format`.
pub fn exported_file(document: &Document, format: Export) -> Result<ExportedFile, String> {
    let (extension, mime, bytes) = match format {
        Export::Pdf => {
            ("pdf", "application/pdf", ravenrs_report_pdf::to_pdf(document).map_err(|e| e.to_string())?)
        }
        Export::Xlsx => (
            "xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ravenrs_report_xlsx::to_xlsx(document).map_err(|e| e.to_string())?,
        ),
        // A byte order mark, so Excel reads the accents as UTF-8.
        Export::Csv => {
            ("csv", "text/csv", format!("\u{feff}{}", ravenrs_report_xlsx::to_csv(document)).into_bytes())
        }
    };
    Ok(ExportedFile { name: format!("{}.{extension}", file_stem(&document.title)), mime, bytes })
}

/// A title as a file name: without the characters file systems refuse.
fn file_stem(title: &str) -> String {
    let stem: String =
        title.chars().map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { ' ' } else { c }).collect();
    let stem = stem.trim().trim_end_matches('.').trim();
    if stem.is_empty() { "relatorio".to_owned() } else { stem.to_owned() }
}

/// Exports the document and hands the file to the user: a download on the web,
/// or the program that opens the format on the desktop.
pub fn export(document: &Document, format: Export) {
    match exported_file(document, format) {
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
    open_path(&path);
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

/// Prints the document: through a hidden frame and the browser's print dialog
/// on the web, or by opening the page in the default browser on the desktop.
pub fn print(document: &Document) {
    let html = document.print_html();
    #[cfg(target_arch = "wasm32")]
    print_in_browser(&html);
    #[cfg(not(target_arch = "wasm32"))]
    print_on_desktop(&html);
}

#[cfg(target_arch = "wasm32")]
fn print_in_browser(html: &str) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    const FRAME_ID: &str = "ravenrs-print-frame";
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    if let Some(old) = doc.get_element_by_id(FRAME_ID) {
        old.remove();
    }
    let Some(frame) =
        doc.create_element("iframe").ok().and_then(|e| e.dyn_into::<web_sys::HtmlIFrameElement>().ok())
    else {
        return;
    };
    frame.set_id(FRAME_ID);
    frame.set_attribute("aria-hidden", "true").ok();
    // Not display:none: browsers refuse to print hidden frames.
    let style = frame.style();
    for (k, v) in [
        ("position", "fixed"),
        ("right", "0"),
        ("bottom", "0"),
        ("width", "0"),
        ("height", "0"),
        ("border", "0"),
    ] {
        style.set_property(k, v).ok();
    }
    let target = frame.clone();
    let on_load = Closure::once_into_js(move || {
        let Some(window) = target.content_window() else { return };
        // The embedded fonts must be ready, or the page prints in a fallback font.
        let ready = window.document().and_then(|d| d.fonts().ready().ok());
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(ready) = ready {
                wasm_bindgen_futures::JsFuture::from(ready).await.ok();
            }
            window.focus().ok();
            window.print().ok();
        });
    });
    frame.set_onload(Some(on_load.unchecked_ref()));
    frame.set_srcdoc(html);
    if let Some(body) = doc.body() {
        body.append_child(&frame).ok();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn print_on_desktop(html: &str) {
    let path = std::env::temp_dir().join("ravenrs-report-print.html");
    if let Err(e) = std::fs::write(&path, html) {
        tracing::warn!(error = %e, "could not write the print page");
        return;
    }
    open_path(&path);
}

/// Opens a file with the program the system associates with it.
#[cfg(not(target_arch = "wasm32"))]
fn open_path(path: &std::path::Path) {
    let opener = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(path).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(path).spawn()
    };
    if let Err(e) = opener {
        tracing::warn!(error = %e, "could not open the file");
    }
}

#[cfg(test)]
mod export_tests {
    use ravenrs_report::{ReportEngine, samples};

    use super::*;

    fn invoice() -> Document {
        let report = samples::invoice();
        let data = report.embedded_data().cloned().unwrap_or_default();
        ReportEngine::new().render(&report, &data)
    }

    #[test]
    fn each_format_gives_a_named_file_of_its_kind() -> Result<(), String> {
        let doc = invoice();
        let pdf = exported_file(&doc, Export::Pdf)?;
        assert!(pdf.name == "Fatura.pdf" && pdf.bytes.starts_with(b"%PDF-"));
        let xlsx = exported_file(&doc, Export::Xlsx)?;
        assert!(xlsx.name == "Fatura.xlsx" && xlsx.bytes.starts_with(b"PK"));
        let csv = exported_file(&doc, Export::Csv)?;
        assert!(csv.name == "Fatura.csv" && csv.bytes.starts_with("\u{feff}".as_bytes()));
        Ok(())
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_stem("Vendas: 2026/11"), "Vendas  2026 11");
        assert_eq!(file_stem("  ?? "), "relatorio");
        assert_eq!(file_stem("Relatório."), "Relatório");
    }
}
