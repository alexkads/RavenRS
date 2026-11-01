//! The document viewer: renders the report with its data and shows the pages.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ravenrs::core::Primitive;
use ravenrs::prelude::*;
use ravenrs_report::{Document, RenderOptions, paint_page};

use crate::editor::Editor;

/// The last render, reused while the report and parameters do not change.
#[derive(Default)]
pub struct RenderCache {
    key: Option<(u64, String)>,
    pub doc: Option<Rc<Document>>,
    pub millis: f64,
    pages: HashMap<(usize, u32), Rc<Scene>>,
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
    params: &HashMap<String, String>,
) -> Rc<Document> {
    let version = editor.version.get();
    let mut sorted: Vec<_> = params.iter().collect();
    sorted.sort();
    let key = (version, format!("{sorted:?}"));
    if let Some(doc) = cache.borrow().doc.clone().filter(|_| cache.borrow().key.as_ref() == Some(&key)) {
        return doc;
    }
    let report = editor.report.get();
    let data = report.embedded_data().cloned().unwrap_or(serde_json::Value::Null);
    let mut options = RenderOptions::default();
    for p in &report.parameters {
        if let Some(text) = params.get(&p.name).filter(|t| !t.trim().is_empty()) {
            let value = text
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .map_or_else(|_| serde_json::json!(text), |n| serde_json::json!(n));
            options.parameters.insert(p.name.clone(), value);
        }
    }
    let start = now_ms();
    let doc = Rc::new(editor.engine.borrow_mut().render_with(&report, &data, &options));
    let mut c = cache.borrow_mut();
    c.millis = now_ms() - start;
    c.key = Some(key);
    c.doc = Some(doc.clone());
    c.pages.clear();
    doc
}

/// The scene of one page at a zoom, cached.
pub fn page_scene(
    editor: &Editor,
    cache: &Rc<RefCell<RenderCache>>,
    doc: &Document,
    index: usize,
    zoom: f32,
) -> Rc<Scene> {
    let key = (index, zoom.to_bits());
    if let Some(s) = cache.borrow().pages.get(&key) {
        return s.clone();
    }
    let mut scene = Scene::new();
    if let Some(page) = doc.pages.get(index) {
        let mut engine = editor.engine.borrow_mut();
        paint_page(page, engine.text_system(), &mut scene, Point::ZERO, zoom);
    }
    let scene = Rc::new(scene);
    cache.borrow_mut().pages.insert(key, scene.clone());
    scene
}

/// Copies a cached page into the frame being painted, moved to `origin`.
pub fn draw_at(source: &Scene, origin: Point, scene: &mut Scene) {
    let shift = |r: Rect| Rect::from_origin_size(Point::new(r.x() + origin.x, r.y() + origin.y), r.size);
    for p in source.primitives() {
        match p {
            Primitive::Quad(q) => scene.push_quad(Quad { rect: shift(q.rect), ..*q }),
            Primitive::Glyphs(run) => {
                let mut run = run.clone();
                for g in &mut run.glyphs {
                    g.x += origin.x;
                    g.y += origin.y;
                }
                scene.push_glyphs(run);
            }
            Primitive::Image(img) => scene.push_image(ImageQuad { rect: shift(img.rect), ..img.clone() }),
            Primitive::PushClip(r) => scene.push_clip(shift(*r)),
            Primitive::PopClip => scene.pop_clip(),
            _ => {}
        }
    }
}

/// One page on screen.
pub fn page_view(scene: Rc<Scene>, width: f32, height: f32) -> Canvas {
    Canvas::new(move |frame, out| draw_at(&scene, frame.origin, out)).size(width, height)
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
    let opener = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&path).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&path).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&path).spawn()
    };
    if let Err(e) = opener {
        tracing::warn!(error = %e, "could not open the print page");
    }
}
