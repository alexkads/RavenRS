#![doc = include_str!("../README.md")]
// The crate documentation is the README, so docs.rs and GitHub show the same text. It is
// built for `wasm32-unknown-unknown` only (`package.metadata.docs.rs`); on other targets
// the crate is empty.
#![cfg(target_arch = "wasm32")]

mod access;
mod app;
mod error;
mod storage;

pub use app::{WebApp, WebBackend, WebOptions, start};
pub use error::WebError;
pub use storage::IndexedDbStorage;
