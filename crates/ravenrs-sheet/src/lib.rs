//! Headless spreadsheet engine for RavenRS.
//!
//! The layers, from the bottom up:
//!
//! - [`base`]: cell addresses, values, errors, date serials and input parsing.
//! - [`numfmt`]: number format codes, General display and `TEXT()`.
//! - [`formula`]: the formula language: lexer, parser, printer and reference adjustment.
//! - [`functions`]: the worksheet function library.
//! - [`math`]: equations laid out from LaTeX, and their OMML form for XLSX.
//! - [`model`]: workbooks, sheets, copy-on-write cells, styles, tables and charts.
//! - [`calc`]: dependency graph, recalculation and dynamic arrays.
//! - [`xlsx`]: XLSX and CSV import and export.
//! - [`chart`]: chart data and layout into toolkit-free primitives, and sparklines.
//! - [`pdf`]: a small PDF writer for printing sheets.
//! - [`craft`]: the file-format crates of craft-libs the readers use: compound files, OPC
//!   packages, `.xls`, encryption, CSV and SYLK, ZIP and XML with limits.
//! - [`engine`]: sessions: every action is a command with an id and JSON parameters, with undo,
//!   clipboard, fill, sort and filter.
//!
//! Nothing here draws or depends on a window, so it also runs on servers and in WebAssembly.
//! The engine comes from GridCraft (MIT OR Apache-2.0); see `NOTICE`.
#![allow(missing_docs)]
// The engine's docs point at private helpers to explain how things fit together.
#![allow(rustdoc::private_intra_doc_links)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod base;
pub mod calc;
pub mod chart;
pub mod craft;
pub mod engine;
pub mod formula;
pub mod functions;
pub mod math;
pub mod model;
pub mod numfmt;
pub mod pdf;
pub mod xlsx;
