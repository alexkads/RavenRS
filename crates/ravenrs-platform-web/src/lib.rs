//! Browser backend for RavenRS.
//!
//! The UI is drawn by RavenRS into a `<canvas>` with WebGPU or WebGL2; the DOM is used only
//! to host the canvas, receive input and mirror the UI for screen readers. Frames are scheduled with
//! `requestAnimationFrame` only when something changed, so an idle page costs
//! nothing. Nothing blocks the main thread: adapter and device creation are
//! awaited, and no threads or `SharedArrayBuffer` are required.
//!
//! [`WebBackend::Auto`] uses WebGPU where the browser has it and WebGL2 otherwise;
//! [`WebOptions::backend`] can ask for one of them. When none starts, `start`
//! returns the error and shows a short explanation next to the canvas.
//!
//! This crate is empty on non-wasm targets.

#![cfg(target_arch = "wasm32")]

mod access;
mod app;
mod error;

pub use app::{WebApp, WebBackend, WebOptions, start};
pub use error::WebError;
