//! Headless PDF engine for RavenRS.
//!
//! The layers, from the bottom up:
//!
//! - [`geom`]: rectangles, matrices and quadrilaterals in PDF user space.
//! - [`crypt`]: the standard security handler: RC4 and AES, passwords and permissions.
//! - [`filters`]: stream filters: Flate, LZW, ASCII85 and hex, run length, CCITT fax and JBIG2.
//! - [`cos`]: the object layer: parsing, cross-reference repair, incremental and full saves,
//!   object streams and encryption.
//! - [`model`]: pages, the page tree and the document catalogue over `cos`.
//! - [`content`]: content streams parsed into operators.
//! - [`fonts`]: font metrics, encodings, shaping and subsetting for text written into a PDF.
//! - [`render`]: pages rasterized to pixels, text extraction and document inspection.
//! - [`organize`]: merging, splitting, page order and rotation, attachments and metadata.
//! - [`third_party`]: the renderer's own stack (hayro, vello_cpu, lopdf) at the revisions
//!   PdfCraft patches; not an API of this crate.
//!
//! ```rust
//! use std::sync::Arc;
//! use ravenrs_pdf::cos::Document;
//! use ravenrs_pdf::render::{PageRenderer, RenderConfig, RenderRequest};
//!
//! // A one-page PDF drawing a red square; the reader rebuilds the missing cross-reference table.
//! let bytes = Arc::new(
//!     b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n\
//!       2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n\
//!       3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >> endobj\n\
//!       4 0 obj << /Length 27 >> stream\n1 0 0 rg 25 25 50 50 re f\nendstream endobj\n\
//!       trailer << /Root 1 0 R >>\n%%EOF\n"
//!         .to_vec(),
//! );
//!
//! // Pages to pixels: premultiplied RGBA, one point per pixel at scale 1.
//! let mut renderer = PageRenderer::new(bytes.clone(), RenderConfig::default());
//! assert_eq!(renderer.page_count(), 1);
//! let page = renderer.render(RenderRequest { page: 0, scale: 1.0, ..Default::default() });
//! assert_eq!((page.width, page.height), (100, 100));
//! let centre = ((50 * 100 + 50) * 4) as usize;
//! assert_eq!(&page.rgba[centre..centre + 3], &[255, 0, 0]);
//!
//! // The same file twice, as one document of two pages.
//! let doc = Document::open(bytes)?;
//! let both = ravenrs_pdf::organize::combine(&[("a.pdf", &doc), ("b.pdf", &doc)])?;
//! assert_eq!(ravenrs_pdf::organize::page_count(&both)?, 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Nothing here draws to a window, so it also runs on servers and in WebAssembly. The engine comes
//! from PdfCraft (MIT OR Apache-2.0); see `NOTICE`.
#![allow(missing_docs)]
// The engine's docs point at private helpers to explain how things fit together.
#![allow(rustdoc::private_intra_doc_links)]

// The vendored crates were `no_std` with `alloc`; as modules they reach it through the root.
extern crate alloc;

pub mod content;
pub mod cos;
pub mod crypt;
pub mod filters;
pub mod fonts;
pub mod geom;
pub mod model;
pub mod organize;
pub mod render;
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod testing;
#[doc(hidden)]
pub mod third_party;
