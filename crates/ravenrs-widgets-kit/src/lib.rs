//! The components of general use in RavenRS: navigation (`Tabs`, `Sidebar`), commands and toolbars,
//! form editors and layouts, lookups, filters and expressions, status pieces, tiles, the theme
//! editor, diagnostics and the counter's keys. The families of components (grid, lists, dock,
//! scheduler, imaging, charts, chat) are built on them and on `ravenrs-widgets-base`.
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names. The modules are public only for the crates of the families; they are hidden from
//! the documentation and are not a stable API.

// The base, by the names the components use: `crate::Button`, `crate::context::BuildCx`.
#[allow(unused_imports)]
use ravenrs_widgets_base::*;

#[doc(hidden)]
pub mod commands;
#[doc(hidden)]
pub mod counter;
#[doc(hidden)]
pub mod data;
#[doc(hidden)]
pub mod diagnostics;
#[doc(hidden)]
pub mod editors;
#[doc(hidden)]
pub mod expression_editor;
#[doc(hidden)]
pub mod files;
#[doc(hidden)]
pub mod filter_builder;
#[doc(hidden)]
pub mod form_layout;
#[doc(hidden)]
pub mod lookup;
#[doc(hidden)]
pub mod model_picker;
#[doc(hidden)]
pub mod nav;
#[doc(hidden)]
pub mod quad_field;
#[doc(hidden)]
pub mod quick_edit;
#[doc(hidden)]
pub mod release_notes;
#[doc(hidden)]
pub mod shortcuts;
#[doc(hidden)]
pub mod status;
#[doc(hidden)]
pub mod tag_box;
#[doc(hidden)]
pub mod theme;
#[doc(hidden)]
pub mod tiles;
#[doc(hidden)]
pub mod wizard;

pub use commands::{ConfirmDialog, FloatingToolbar, SplitButton, Toolbar, ToolbarAction};
pub use counter::{KeyBar, KeyBarItem, Scan, ScanField};
pub use data::{Column, List, ListItem, Table};
pub use diagnostics::{Diagnostics, DiagnosticsHost};
pub use editors::{DateTime, DateTimePicker, MaskedField, NumberField, TextArea, Time, TimePicker};
pub use expression_editor::ExpressionEditor;
#[cfg(feature = "decode")]
pub use files::ImageField;
pub use files::{DropZone, FileUpload};
pub use filter_builder::{FilterBuilder, FilterField, FilterFieldKind};
pub use form_layout::{FormGroup, FormLayout};
pub use lookup::{LookupField, LookupItem, LookupRequest, LookupResponder};
pub use model_picker::{ModelChoice, ModelPicker, ModelPickerLabels, ModelStatus};
pub use nav::{
    Accordion, Breadcrumb, Disclosure, Sidebar, SidebarBrand, SidebarCollapse, SidebarGroup, SidebarInset,
    SidebarItem, SidebarParts, SidebarSection, SidebarSkeleton, SidebarState, SidebarTrigger, SidebarVariant,
    Tabs, TabsVariant,
};
pub use quad_field::QuadField;
pub use quick_edit::{InlineRename, ValueSlider};
pub use release_notes::{CodeBlock, ReleaseNote, ReleaseNotes};
pub use shortcuts::{ShortcutHintBar, ShortcutSheet};
pub use status::{
    DaySeparator, Dot, EmptyState, HeroBanner, IconTile, MultiProgress, OverlayBadge, Rating, ShimmerText,
};
pub use tag_box::TagBox;
pub use theme::{ThemeEditor, ThemePicker};
pub use tiles::{CardView, Tile, TileLayout, TileSpot};
pub use wizard::{IndicatorStep, StepBadge, StepIndicator, StepState, Wizard, WizardStep};
