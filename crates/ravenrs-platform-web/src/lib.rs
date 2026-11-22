#![doc = include_str!("../README.md")]
// The crate documentation is the README, so docs.rs and GitHub show the same text. It is
// built for `wasm32-unknown-unknown` only (`package.metadata.docs.rs`); on other targets
// only the ARIA of each node is compiled, so it is tested everywhere.

#[cfg(target_arch = "wasm32")]
mod access;
#[cfg(target_arch = "wasm32")]
mod app;
#[allow(dead_code, reason = "only the browser uses it; it is here to be tested on every target")]
mod aria;
#[cfg(target_arch = "wasm32")]
mod error;
#[cfg(target_arch = "wasm32")]
mod storage;

#[cfg(target_arch = "wasm32")]
pub use app::{WebApp, WebBackend, WebOptions, start};
#[cfg(target_arch = "wasm32")]
pub use error::WebError;
#[cfg(target_arch = "wasm32")]
pub use storage::IndexedDbStorage;
