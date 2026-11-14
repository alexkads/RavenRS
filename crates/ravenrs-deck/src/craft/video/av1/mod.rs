//! AV1, written from the *AV1 Bitstream & Decoding Process Specification* v1.0.0 with Errata 1.
//!
//! | Feature | Module | What |
//! |---|---|---|
//! | `av1-decode` | [`decode`] | Decoder |
//! | `av1-encode` | this module | Encoder ([`Encoder`]) |
//!
//! Feature `av1` turns on both.

pub mod decode;

#[cfg(any())]
mod encode;
#[cfg(any())]
pub use encode::*;
