//! The stories of the components of this crate, for RavenComponentBook: see
//! `ravenrs_widgets_base::book`.

mod nav;
mod receipt;

use ravenrs_widgets_base::book::Meta;

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
pub fn stories() -> Vec<Meta> {
    use ravenrs_widgets_base::book::ComponentBook;
    let mut all = vec![crate::Tabs::book(), crate::Receipt::book()];
    all.extend(generated());
    all
}

// rcb-scaffold: begin
mod accordion;
mod breadcrumb;
mod card_view;
mod code_block;
mod confirm_dialog;
mod date_time_picker;
mod day_separator;
mod disclosure;
mod dot;
mod drop_zone;
mod empty_state;
mod expression_editor;
mod file_upload;
mod filter_builder;
mod floating_toolbar;
mod form_layout;
mod hero_banner;
mod icon_tile;
#[cfg(feature = "decode")]
mod image_field;
mod inline_rename;
mod key_bar;
mod list;
mod lookup_field;
mod masked_field;
mod model_picker;
mod multi_progress;
mod number_field;
mod overlay_badge;
mod parameter_form;
mod quad_field;
mod rating;
mod release_notes;
mod scan_field;
mod shimmer_text;
mod shortcut_hint_bar;
mod shortcut_sheet;
mod sidebar;
mod sidebar_brand;
mod split_button;
mod step_badge;
mod step_indicator;
mod table;
mod tag_box;
mod text_area;
mod theme_editor;
mod theme_picker;
mod tile_layout;
mod time_picker;
mod toolbar;
mod value_slider;
mod wizard;
#[allow(unused_imports)]
use ravenrs_widgets_base::AnyView;

/// The stories written by scripts/rcb-scaffold.py, in name order.
#[allow(
    unused_mut,
    clippy::vec_init_then_push,
    reason = "an entry may sit behind a feature, as its component does"
)]
pub(crate) fn generated() -> Vec<Meta> {
    let mut all = Vec::new();
    all.push(<crate::Accordion as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Breadcrumb as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::CardView as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::CodeBlock as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ConfirmDialog as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::DateTimePicker as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::DaySeparator as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Disclosure as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Dot as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::DropZone<AnyView> as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::EmptyState as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ExpressionEditor as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::FileUpload as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::FilterBuilder as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::FloatingToolbar<AnyView> as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::FormLayout as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::HeroBanner as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::IconTile as ravenrs_widgets_base::book::ComponentBook>::book());
    #[cfg(feature = "decode")]
    all.push(<crate::ImageField as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::InlineRename as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::KeyBar as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::List<(AnyView,)> as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::LookupField as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::MaskedField as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ModelPicker as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::MultiProgress as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::NumberField as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::OverlayBadge<AnyView, AnyView> as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ParameterForm as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::QuadField as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Rating as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ReleaseNotes as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ScanField as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ShimmerText as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ShortcutHintBar as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ShortcutSheet as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Sidebar<(AnyView,)> as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::SidebarBrand as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::SplitButton as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::StepBadge as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::StepIndicator as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Table as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::TagBox as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::TextArea as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ThemeEditor as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ThemePicker as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::TileLayout as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::TimePicker as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Toolbar as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::ValueSlider as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Wizard as ravenrs_widgets_base::book::ComponentBook>::book());
    all
}
// rcb-scaffold: end
