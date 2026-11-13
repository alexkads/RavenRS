//! The crates PdfCraft vendors with patches of its own, at the versions it patches: hayro and its
//! parts (Apache-2.0 OR MIT), vello_cpu and vello_common (Apache-2.0 OR MIT) and lopdf (MIT).
//! Their licences are in `LICENSES/`; see `NOTICE`. They are here for `render` and are not an API
//! of this crate.
// Their own lints, not PdfCraft's: the vendored crates never went through its clippy settings.
#![allow(missing_docs, clippy::all, clippy::pedantic, clippy::cognitive_complexity, unused_qualifications, unused_imports)]

pub mod hayro;
pub mod hayro_cmap;
pub mod hayro_interpret;
pub mod hayro_jbig2;
pub mod hayro_postscript;
pub mod hayro_syntax;
pub mod lopdf;
pub mod vello_common;
pub mod vello_cpu;
