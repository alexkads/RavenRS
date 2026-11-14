//! Shared optional PDF reading, rendering and writing for Craft applications.
//!
//! Applications retain document models, layout, shaping, font policy and scheduling.
//! Advanced backend re-exports are coupled to the pinned backend, not a universal document model.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(any())]
pub mod cos;
#[cfg(any())]
pub mod crypt;
#[cfg(any())]
pub mod filters;
#[cfg(any())]
pub use hayro_interpret as interpret;
mod link;
#[cfg(any())]
pub use hayro_syntax as syntax;
pub use link::safe_url;

#[cfg(any())]
mod read;
#[cfg(any())]
pub use read::*;
#[cfg(any())]
pub mod raster;
pub mod write;
