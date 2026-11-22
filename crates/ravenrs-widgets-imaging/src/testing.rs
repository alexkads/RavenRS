//! [`CropDriver`]: what a test does to a `CropOverlay` (or any `TransformHandles`), by value,
//! as assistive technology does it, never by dragging pixels.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_core::Rect;
//! use ravenrs_widgets_imaging::testing::CropDriver;
//! # fn run(e2e: &mut E2e) -> ravenrs_e2e::E2eResult {
//! CropDriver::new(e2e, "corte").set(Rect::new(10.0, 20.0, 300.0, 200.0), 0.0)?;
//! # Ok(()) }
//! ```

use ravenrs_core::Rect;
use ravenrs_e2e::{E2e, E2eResult};

/// Sets the box of a `CropOverlay` or `TransformHandles` in an [`E2e`] session.
pub struct CropDriver<'a> {
    e2e: &'a mut E2e,
    id: String,
}

impl std::fmt::Debug for CropDriver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CropDriver").field("id", &self.id).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<'a> CropDriver<'a> {
    /// Drives the crop the app gave this test id.
    pub fn new(e2e: &'a mut E2e, id: impl Into<String>) -> Self {
        Self { e2e, id: id.into() }
    }

    /// Gives the crop this rectangle, in the units of its content, turned `angle` degrees.
    pub fn set(&mut self, rect: Rect, angle: f32) -> E2eResult {
        let text = format!("{}, {}, {} × {}, {angle}", rect.x(), rect.y(), rect.width(), rect.height());
        self.e2e.by_test_id(self.id.clone()).set_value(&text)
    }

    /// The box as the crop says it: `x, y, width × height, angle`.
    pub fn value(&mut self) -> E2eResult<String> {
        Ok(self.e2e.by_test_id(self.id.clone()).node()?.value.unwrap_or_default())
    }
}
