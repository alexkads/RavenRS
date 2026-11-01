//! Printing a document, on the desktop and in the browser.

use ravenrs_report::Document;

/// Prints the document: through a hidden frame and the browser's print dialog
/// on the web, or by opening the page in the default browser, with its print
/// dialog up, on the desktop.
pub fn print(document: &Document) {
    let html = document.print_html();
    #[cfg(target_arch = "wasm32")]
    print_in_browser(&html);
    #[cfg(not(target_arch = "wasm32"))]
    print_on_desktop(&html);
}

/// The page, asking for the print dialog once its embedded fonts are ready.
#[cfg_attr(target_arch = "wasm32", allow(dead_code, reason = "the web build prints from outside the page"))]
fn with_print_dialog(html: &str) -> String {
    let script = "<script>document.fonts.ready.then(function(){window.print()})</script></body>";
    html.replacen("</body>", script, 1)
}

#[cfg(not(target_arch = "wasm32"))]
fn print_on_desktop(html: &str) {
    let path = std::env::temp_dir().join("ravenrs-report-print.html");
    if let Err(e) = std::fs::write(&path, with_print_dialog(html)) {
        tracing::warn!(error = %e, "could not write the print page");
        return;
    }
    if let Err(e) = ravenrs_platform_native::open_with_system(&path) {
        tracing::warn!(error = %e, "could not open the print page");
    }
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

#[cfg(test)]
mod tests {
    use super::with_print_dialog;

    #[test]
    fn the_page_asks_for_the_dialog_inside_its_body() {
        let page = with_print_dialog("<html><body><svg></svg></body></html>");
        assert!(page.contains("window.print()"));
        assert!(page.ends_with("</script></body></html>"), "{page}");
    }
}
