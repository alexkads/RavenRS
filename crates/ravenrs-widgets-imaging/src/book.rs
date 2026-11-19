//! The stories of the components of this crate, for RavenComponentBook: see
//! `ravenrs_widgets_base::book`.

#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ComponentBook, Meta};

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
pub fn stories() -> Vec<Meta> {
    generated()
}

// rcb-scaffold: begin
mod color_wheel;
mod crop_overlay;
mod curve_editor;
mod eyedropper;
mod histogram;
mod image_lightbox;
mod marquee;
mod minimap;
mod protractor;
mod rubber_band;
mod rule_of_thirds_grid;
mod ruler;
mod transform_handles;
mod zoom_pan_view;
#[allow(unused_imports)]
use ravenrs_widgets_base::AnyView;

/// The stories written by scripts/rcb-scaffold.py, in name order.
#[allow(clippy::vec_init_then_push, reason = "an entry may sit behind a feature, as its component does")]
pub(crate) fn generated() -> Vec<Meta> {
    let mut all = Vec::new();
    all.push(<crate::ColorWheel as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::CropOverlay as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::CurveEditor as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Eyedropper as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Histogram as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ImageLightbox as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Marquee as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Minimap as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Protractor as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::RubberBand as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::RuleOfThirdsGrid as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Ruler as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::TransformHandles as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ZoomPanView as ravenrs_widgets_base::book::ComponentBook>::book());
    all
}
// rcb-scaffold: end
