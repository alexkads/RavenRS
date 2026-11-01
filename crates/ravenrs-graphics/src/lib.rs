//! Drawing logic shared across RavenRS. It depends on no engine, so reports, the
//! UI and any later module draw the same charts and the same barcodes.
//!
//! * [`vector`] holds shapes that PDF and SVG can write as they are, and a
//!   rasterizer for the outputs that need pictures.
//! * [`chart`] lays numbers out as shapes and labels.
//! * [`barcode`] turns text into the modules a scanner reads.

pub mod barcode;
pub mod chart;
pub mod vector;
