//! RavenRS instruments for pictures, fed by the caller's data: `Histogram`, `Ruler`, `ColorWheel`,
//! `CurveEditor`, `ZoomPanView`, `Minimap`, `CropOverlay`, `Eyedropper`, `CompareView`, `ImageLightbox`
//! and more, and `Lottie` (feature `lottie`).
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names.

// The base and the kit, by the names the components use: `crate::Button`, `crate::Tabs`,
// `crate::context::BuildCx`.
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;

#[doc(hidden)]
pub mod imaging;
#[cfg(feature = "lottie")]
#[doc(hidden)]
pub mod lottie;

pub use imaging::{
    BoxHandle, BoxTransform, ColorWheel, CompareMode, CompareSide, CompareView, CompositionGuide,
    ContentSpace, CropArea, CropOverlay, CurveEditor, Eyedropper, EyedropperArea, EyedropperState, Histogram,
    HistogramChannel, HistogramScale, HueSaturation, ImageLightbox, LightboxItem, Marquee, Minimap,
    Protractor, RubberBand, RuleOfThirdsGrid, Ruler, TransformHandles, ZoomPanPaint, ZoomPanView,
};
#[cfg(feature = "lottie")]
pub use lottie::Lottie;
