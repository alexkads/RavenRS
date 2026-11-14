//! Headless presentation engine for RavenRS.
//!
//! The layers, from the bottom up:
//!
//! - [`geom`] and [`color`]: lengths, points, transforms, colours and theme colour slots.
//! - [`model`]: the presentation: slides, layouts and masters, shapes, text bodies, tables,
//!   charts, media and notes, with cheap `Arc`-shared clones.
//! - [`fonts`] and [`text`]: font lookup and shaping, line breaking, bidi and date fields.
//! - [`anim`]: animations and slide transitions, evaluated at a point in time.
//! - [`render`]: slides drawn to pixels on the CPU.
//! - [`pptx`] and [`format`]: PowerPoint packages and the native format, read and written.
//! - [`pdf`]: slides, notes pages and handouts exported to PDF.
//! - [`media`]: embedded audio and video: probing and poster frames.
//! - [`engine`]: the commands, selection and undo history an editor drives.
//! - [`mcp`]: the engine's commands as tools for an agent.
//!
//! ```rust
//! use ravenrs_deck::{model, pptx, render};
//!
//! // A new presentation, saved as PPTX, read back and its first slide drawn to a PNG.
//! let deck = model::defaults::new_presentation(None);
//! let bytes = pptx::export(&deck).unwrap();
//! let again = pptx::import(&bytes).unwrap();
//! assert_eq!(again.slides.len(), deck.slides.len());
//! let opts = render::RenderOpts { scale: 0.25, ..Default::default() };
//! let png = render::render_slide(&again, 0, &opts).to_png();
//! assert!(png.starts_with(b"\x89PNG"));
//! ```
//!
//! Nothing here depends on a window, so it also runs on servers and in WebAssembly. The engine
//! comes from DeckCraft (MIT OR Apache-2.0), with the craft-libs crates it uses and krilla copied
//! in as modules; see `NOTICE`.
#![allow(missing_docs)]
// The engine's docs point at private helpers to explain how things fit together.
#![allow(rustdoc::private_intra_doc_links)]

pub mod anim;
pub mod color;
pub mod engine;
pub mod fonts;
pub mod format;
pub mod geom;
pub mod mcp;
pub mod media;
pub mod model;
pub mod pdf;
pub mod pptx;
pub mod render;
pub mod text;

/// The craft-libs crates the engine uses, copied in as modules.
pub mod craft {
    pub mod audio;
    pub mod media;
    pub mod metafile;
    pub mod pdf;
    pub mod video;
}

/// Third-party code the engine carries patched.
pub mod third_party {
    pub mod krilla;
}
