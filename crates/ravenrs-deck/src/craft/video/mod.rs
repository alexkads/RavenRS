#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
//! Clean-room pure-Rust video codecs for the Craft apps, one feature per codec. Input and output
//! are byte slices and planar sample buffers (no filesystem access of its own); builds for
//! `wasm32-unknown-unknown`.
//!
//! | Feature | Module | Codec |
//! |---|---|---|
//! | `dnx` | [`dnx`] | Avid DNxHD / DNxHR (SMPTE ST 2019-1): decoder and encoder |
//! | `apv` | [`apv`] | APV (RFC 9924): decoder (`apv-decode`) and encoder (`apv-encode`) |
//! | `vp8` | [`vp8`] | VP8 (RFC 6386): decoder (`vp8-decode`) and key-frame encoder (`vp8-encode`) |
//! | `mpeg2` | [`mpeg2`] | MPEG-2 video (ITU-T H.262) and MPEG-1 video: decoder |
//! | `mpeg4` | [`mpeg4`] | MPEG-4 Part 2 video (ISO/IEC 14496-2, Xvid/DivX): Simple profile encoder (`mpeg4-encode`) |
//! | `prores` | [`prores`] | Apple ProRes (SMPTE RDD 36): decoder and encoder |
//! | `h264` | [`h264`] | ITU-T H.264 / AVC: decoder (`h264-decode`) and encoder (`h264-encode`) |
//! | `hevc` | [`hevc`] | ITU-T H.265 / HEVC: decoder (`hevc-decode`) and encoder (`hevc-encode`) |
//! | `vp9` | [`vp9`] | VP9: decoder (`vp9-decode`) and encoder (`vp9-encode`) |
//! | `av1` | [`av1`] | AV1 (AOMedia Video 1): decoder (`av1-decode`) and encoder (`av1-encode`) |
//!
//! No codec is on by default. Feature `threads` spreads the work of the codecs that support it
//! over rayon worker threads.
//!
//! The decoders bound the pictures they hold with a memory budget ([`budget`]).
//!
//! Containers and bit-level I/O are in `craft-media`; the codecs here share its `bitstream`
//! module and describe their streams with `crate::craft::media::CodecParams`.

pub mod budget;

#[cfg(any())]
pub mod apv;

#[cfg(any())]
pub mod dnx;

#[cfg(any())]
pub mod vp8;

#[cfg(any())]
pub mod mpeg2;

#[cfg(any())]
pub mod mpeg4;

#[cfg(any())]
pub mod prores;

pub mod h264;

pub mod hevc;

pub mod vp9;

pub mod av1;
