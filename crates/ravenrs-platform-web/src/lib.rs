//! Browser backend for RavenRS.
//!
//! The UI is drawn by RavenRS into a `<canvas>` with WebGPU; the DOM is used only
//! to host the canvas and receive input. Frames are scheduled with
//! `requestAnimationFrame` only when something changed, so an idle page costs
//! nothing. Nothing blocks the main thread: adapter and device creation are
//! awaited, and no threads or `SharedArrayBuffer` are required.
//!
//! When WebGPU is unavailable, `start` returns `WebError::WebGpuUnavailable`
//! and shows a short explanation next to the canvas. There is no WebGL fallback.
//!
//! This crate is empty on non-wasm targets.

#![cfg(target_arch = "wasm32")]

mod app;
mod error;

pub use app::{WebApp, WebOptions, start};
pub use error::WebError;
