use std::fmt;

use raven_render::GpuError;

#[derive(Debug)]
pub enum WebError {
    /// `navigator.gpu` is missing: the browser has no WebGPU or it is disabled.
    WebGpuUnavailable,
    /// No element with the given id, or it is not a `<canvas>`.
    CanvasNotFound(String),
    Gpu(GpuError),
    Js(String),
}

impl fmt::Display for WebError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WebGpuUnavailable => f.write_str(
                "This app needs WebGPU, which this browser does not provide. Use a current version of \
                 Chrome, Edge, Safari or Firefox, and check that hardware acceleration is turned on.",
            ),
            Self::CanvasNotFound(id) => write!(f, "No <canvas id=\"{id}\"> was found on the page."),
            Self::Gpu(e) => write!(f, "The graphics device could not start: {e}."),
            Self::Js(e) => write!(f, "Browser error: {e}"),
        }
    }
}

impl std::error::Error for WebError {}

impl From<GpuError> for WebError {
    fn from(e: GpuError) -> Self {
        Self::Gpu(e)
    }
}

impl From<wasm_bindgen::JsValue> for WebError {
    fn from(v: wasm_bindgen::JsValue) -> Self {
        Self::Js(v.as_string().unwrap_or_else(|| format!("{v:?}")))
    }
}
