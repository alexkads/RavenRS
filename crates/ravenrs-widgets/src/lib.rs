//! Declarative views and the UI runtime.
//!
//! # Writing components
//!
//! There are three ways to make your own component, from simplest to most powerful:
//!
//! 1. **A function** returning `impl View`, composing existing views. Good for
//!    stateless pieces of UI.
//! 2. **A [`Component`]**: a struct holding its state ([`State`](ravenrs_reactive::State)
//!    fields, bindings, callbacks) with a `body()` that composes views. This is the
//!    SwiftUI-style building block and covers most needs.
//! 3. **A [`View`]** implemented directly: full control over layout boxes,
//!    painting (including arbitrary [`Scene`](ravenrs_core::Scene) drawing through
//!    [`Paint::Custom`]), hit-testing, focus and click handling via [`BuildCx`].
//!    The built-in widgets are written this way, with no private API.
//!
//! [`Canvas`] is the shortcut for custom drawing inside a normal layout.
//!
//! # Features
//!
//! * `decode` (default): PNG and JPEG decoding for [`Image::from_bytes`].
//! * `charts` (default): `Chart`, `BarChart`, `AreaChart` and `Barcode`. They draw
//!   the same charts and codes as reports, through `ravenrs-graphics`.
//!
//! # Runtime
//!
//! [`Ui`] owns the root view and turns it into a [`Scene`](ravenrs_core::Scene):
//! *build* (views to elements, tracked by a reactive observer), *layout* (Taffy),
//! *paint*. It only rebuilds when state the last build read has changed or the
//! interaction state (hover, press, focus) changed, and it never touches the GPU.
//!
//! # Motion
//!
//! A view declares how a node moves with [`BuildCx::motion`] and the runtime
//! plays it. While only animations run, frames repeat the paint phase alone, so
//! views are not rebuilt. When nothing moves, no frames are requested.

mod button;
mod calendar;
mod canvas;
mod card;
#[cfg(feature = "charts")]
mod chart;
mod chat;
mod choice;
mod clipboard;
mod color_picker;
mod commands;
mod context;
mod data;
mod date;
mod diagnostics;
mod display;
mod editors;
mod event_calendar;
mod extras;
mod files;
mod fill;
mod filter_builder;
mod floating;
mod form_layout;
pub mod grid;
mod icon;
mod icon_set;
mod image;
mod inputs;
mod labels;
mod lookup;
#[cfg(feature = "lottie")]
mod lottie;
mod model_picker;
mod modifiers;
mod motion;
mod nav;
mod overlay;
mod recurrence;
mod release_notes;
mod remote;
mod ribbon;
mod scheduler;
mod shortcuts;
mod sound;
mod stack;
mod status;
mod tag_box;
mod text;
mod text_field;
mod theme;
mod tiles;
mod timeline;
mod tree;
pub(crate) mod ui;
mod util;
mod view;
mod wizard;

pub use button::{Button, ButtonGroup, ButtonSize, ButtonStyle, IconButton};
pub use calendar::{Calendar, DatePicker, DateRange, DateRangePicker};
pub use canvas::Canvas;
pub use card::{Card, GlassCard};
#[cfg(feature = "charts")]
pub use chart::{
    AreaChart, BarChart, Barcode, Chart, ChartBarFrom, ChartCard, ChartCurve, ChartGrid, ChartIndicator,
    ChartKind, ChartLabelAt, ChartLegend, ChartPolarGrid, ChartReference, ChartStack, ChartTooltip,
    Symbology,
};
pub use chat::{Chat, ChatAuthor, ChatMessage, FloatingCorner, FloatingPanel};
pub use choice::{
    AnchorPoint, AspectRatio, ColorPair, ColorSwatch, FavoriteToggle, Label, NinePointAnchor, Pagination,
    SwatchGrid, ToggleButton, ToggleGroup,
};
pub use clipboard::{Clipboard, ClipboardProvider, MemoryClipboard};
pub use color_picker::ColorPicker;
pub use commands::{ConfirmDialog, Hotkey, SplitButton, Toolbar, ToolbarAction};
pub use context::{
    ActivityObserver, BuildCx, Drag, DragHandler, DragPhase, DrawFn, KeyHandler, LiveFn, NodeId, Paint,
    PaintCx, Placement, QuadPaint, RichFn, Semantics, UiActivity, WidgetId,
};
pub use data::{Column, List, ListItem, Table};
pub use date::{Date, days_in_month, is_leap_year};
pub use diagnostics::{Diagnostics, DiagnosticsHost};
pub use display::{
    Alert, AlertAction, Avatar, Badge, Divider, Kbd, ProgressBar, Section, Skeleton, Stat, Tone,
};
pub use editors::{DateTime, DateTimePicker, MaskedField, NumberField, TextArea, Time, TimePicker};
pub use event_calendar::{CalendarEvent, EventCalendar, RecurrenceEditor};
pub use extras::{Carousel, FormField, InputOtp, Menubar, Rule, Spinner, Splitter, validate};
#[cfg(feature = "decode")]
pub use files::ImageField;
pub use files::{FilePicker, FilePickerProvider, FileRequest, FileUpload, NoFilePicker, PickedFile};
pub use fill::Fill;
pub use filter_builder::{FilterBuilder, FilterField, FilterFieldKind};
pub use floating::{
    Combobox, Command, CommandPalette, ContextMenu, HoverCard, Sheet, Side, SplashScreen, Toast, ToastExtras,
    Toaster, Toasts,
};
pub use form_layout::{FormGroup, FormLayout};
pub use grid::{
    CellAlign, CellEdit, ColumnLayout, DataGrid, FilterLayout, GridCellStyle, GridColumn, GridData,
    GridExportFormat, GridLayout, GridPage, GridRequest, GridResponder, GridRowAction, GridScenario,
    GridTable, GridTableColumn, GridTableKind, NumberFormat, RemoteRows, SelectionMode, SortDirection,
    SortLayout, Summary, Value,
};
pub use icon::Icon;
pub use icon_set::IconName;
#[cfg(feature = "decode")]
pub use image::decode_image;
pub use image::{ContentMode, DecodeError, Image};
pub use inputs::{Checkbox, RadioGroup, SegmentedControl, Slider, Stepper, ThemeSwitcher, Toggle};
pub use labels::{Labels, Localized};
pub use lookup::{LookupField, LookupItem, LookupRequest, LookupResponder};
#[cfg(feature = "lottie")]
pub use lottie::Lottie;
pub use model_picker::{ModelChoice, ModelPicker, ModelPickerLabels, ModelStatus};
pub use modifiers::{Styled, ViewExt};
pub use motion::{Collapse, Motion};
pub use nav::{
    Accordion, Breadcrumb, Disclosure, Sidebar, SidebarBrand, SidebarCollapse, SidebarGroup, SidebarInset,
    SidebarItem, SidebarParts, SidebarSection, SidebarSkeleton, SidebarState, SidebarTrigger, SidebarVariant,
    Tabs,
};
pub use overlay::{Dialog, DropdownMenu, HelpTip, MenuItem, Popover, Select, Tooltip};
#[cfg(feature = "lottie")]
pub use ravenrs_lottie::{Animation as LottieAnimation, LottieError};
pub use recurrence::{Frequency, RecurrenceEnd, RecurrenceRule};
pub use release_notes::{CodeBlock, ReleaseNote, ReleaseNotes};
pub use remote::UiWaker;
pub use ribbon::{Ribbon, RibbonGroup, RibbonItem, RibbonPage, RibbonSize};
pub use scheduler::{
    Resource, Scheduler, SchedulerChange, SchedulerEvent, SchedulerRange, TimeAxis, TimePoint, TimeScale,
};
pub use shortcuts::{ShortcutHintBar, ShortcutSheet};
pub use sound::{Silence, Sound, SoundClip, SoundProvider};
pub use stack::{
    Footer, Grid, HStack, PageHeader, PropertyRow, ScrollView, Spacer, StatusBar, ToolRail, VStack, ZStack,
};
pub use status::{
    DaySeparator, Dot, EmptyState, HeroBanner, IconTile, MultiProgress, OverlayBadge, Rating, ShimmerText,
};
pub use tag_box::TagBox;
pub use text::Text;
pub use text_field::TextField;
pub use theme::{ThemeEditor, ThemePicker};
pub use tiles::{CardView, Tile, TileLayout, TileSpot};
pub use timeline::{Timeline, TimelineEdit, TimelineItem, TimelineMarker, TimelineTrack};
pub use tree::{TreeNode, TreeView};
pub use ui::{Ui, UiStats};
pub use view::{AnyView, Component, FnView, Keyed, Responsive, View, ViewSeq, responsive, view_fn};
pub use wizard::{Wizard, WizardStep};
