//! Headless word-processing engine for RavenRS.
//!
//! The layers, from the bottom up:
//!
//! - [`geom`]: lengths, points, rectangles and colours shared by the others.
//! - [`doc`]: the document model: stories of paragraphs and tables, runs, styles, lists, sections,
//!   fields, notes, comments and tracked changes, with cheap `Arc`-shared clones.
//! - [`metafile`]: WMF and EMF pictures read into vector paths.
//! - [`docx`]: DOCX (WordprocessingML) reading and writing, including password-protected files.
//! - [`docbin`]: the binary Word 97-2003 `.doc` format, read-only.
//! - [`formats`]: plain text, Markdown, HTML and MHT, RTF, OpenDocument Text and LaTeX, and
//!   spreadsheet cells for mail-merge recipients.
//!
//! ```rust
//! // Markdown in, DOCX out, and back again.
//! let document = ravenrs_word::formats::import("md", b"# Letter\n\nDear *reader*,").unwrap().unwrap();
//! let docx = ravenrs_word::docx::write(&document).unwrap();
//! let again = ravenrs_word::docx::read(&docx).unwrap();
//! let markdown = ravenrs_word::formats::export("md", &again).unwrap().unwrap();
//! assert!(String::from_utf8(markdown).unwrap().contains("Letter"));
//! ```
//!
//! Nothing here lays out pages, draws or depends on a window, so it also runs on servers and in
//! WebAssembly. The engine comes from WordCraft (MIT OR Apache-2.0); see `NOTICE`.
#![allow(missing_docs)]
// The engine's docs point at private helpers to explain how things fit together.
#![allow(rustdoc::private_intra_doc_links)]

pub mod doc;
pub mod docbin;
pub mod docx;
pub mod formats;
pub mod geom;
pub mod metafile;
