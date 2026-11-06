//! Drawing logic shared across RavenRS. It depends on no engine, so reports, the
//! UI and any later module draw the same charts and the same barcodes.
//!
//! * [`vector`] holds shapes that PDF and SVG can write as they are, and a
//!   rasterizer for the outputs that need pictures.
//! * [`chart`] lays numbers out as shapes and labels.
//! * [`plot`] lays them out the way dashboards show them, in a richer picture that
//!   says what is under the pointer.
//! * [`sankey`] lays a graph of flows out as nodes and ribbons, or says why it cannot.
//! * [`barcode`] turns text into the modules a scanner reads.

pub mod barcode;
pub mod chart;
pub mod plot;
pub mod sankey;
pub mod vector;
