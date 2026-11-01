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
