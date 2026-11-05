//! From a report to a label printer. A [`Document`](ravenrs_report::Document) becomes a 1-bit bitmap
//! at the printer's resolution, and the bitmap travels inside the printer's language. The layout is
//! the one made in the designer, so it comes out the same on every brand, accents included.
//!
//! Barcode bars are laid on whole dots, never scaled. A code that cannot keep its minimum module on
//! the label gives a [`LabelError`] instead of a label no scanner reads.

mod bitmap;
mod error;
mod language;
mod output;
mod preset;
mod raster;

pub use bitmap::{Bitmap, bitmap_to_png};
pub use error::LabelError;
pub use language::{LabelJob, Language, compile, compile_with, encode_bitmap};
pub use output::LabelOutput;
#[cfg(windows)]
pub use output::WindowsShareOutput;
#[cfg(not(target_arch = "wasm32"))]
pub use output::{CupsOutput, FileOutput, TcpOutput};
pub use preset::{PrinterPreset, presets};
pub use raster::rasterize_page;
