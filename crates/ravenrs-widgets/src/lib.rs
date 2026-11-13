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

// The base lives in its own crate; its modules keep their names here, so the components
// write `crate::context::BuildCx` as before.
// The grid family lives in its own crate, and keeps its module names here.
pub use ravenrs_widgets_grid::grid;

// The lists family lives in its own crate, and keeps its module names here.
#[allow(unused_imports)]
use ravenrs_widgets_lists::lists;
#[allow(unused_imports)]
use ravenrs_widgets_lists::tree;

// The scheduler family lives in its own crate, and keeps its module names here.
#[allow(unused_imports)]
use ravenrs_widgets_scheduler::event_calendar;
#[allow(unused_imports)]
use ravenrs_widgets_scheduler::scheduler;
#[allow(unused_imports)]
use ravenrs_widgets_scheduler::timeline;

// The dock family lives in its own crate, and keeps its module names here.
#[allow(unused_imports)]
use ravenrs_widgets_dock::dock;
#[allow(unused_imports)]
use ravenrs_widgets_dock::ribbon;

// The imaging family lives in its own crate, and keeps its module names here.
#[allow(unused_imports)]
use ravenrs_widgets_imaging::imaging;
#[cfg(feature = "lottie")]
#[allow(unused_imports)]
use ravenrs_widgets_imaging::lottie;

// The charts family lives in its own crate, and keeps its module names here.
#[cfg(feature = "charts")]
#[allow(unused_imports)]
use ravenrs_widgets_charts::chart;
#[cfg(feature = "flow")]
#[allow(unused_imports)]
use ravenrs_widgets_charts::flow;
#[cfg(feature = "charts")]
#[allow(unused_imports)]
use ravenrs_widgets_charts::sankey;

// The chat family lives in its own crate, and keeps its module names here.
#[allow(unused_imports)]
use ravenrs_widgets_chat::chat;
#[cfg(feature = "code")]
#[allow(unused_imports)]
use ravenrs_widgets_chat::code_editor;
#[allow(unused_imports)]
use ravenrs_widgets_chat::conversation;
#[cfg(feature = "code")]
#[allow(unused_imports)]
use ravenrs_widgets_chat::playground;
#[allow(unused_imports)]
use ravenrs_widgets_chat::rich_text;

// The components of general use live in their own crate, and keep their module names here.
#[allow(unused_imports)]
use ravenrs_widgets_base::{
    button, calendar, canvas, card, choice, clipboard, color_picker, context, date, display, dock_side,
    extras, file_picker, fill, floating, full_screen, hotkey, icon, icon_set, ids, image, inputs, kbd,
    labels, link_service, links, localized, modifiers, motion, motion_spec, overlay, page_motion, recurrence,
    remote, reorder, scroll_link, scroll_scene, selection_mode, sound, stack, summary, text, text_field,
    time_link, ui, util, view, zoom,
};
#[allow(unused_imports)]
use ravenrs_widgets_kit::{
    commands, counter, data, diagnostics, editors, expression_editor, files, filter_builder, form_layout,
    lookup, model_picker, nav, quad_field, quick_edit, release_notes, shortcuts, status, tag_box, theme,
    tiles, wizard,
};

#[cfg(feature = "live")]
mod live;
#[cfg(feature = "live")]
mod live_list;

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
pub use chat::{Chat, ChatAuthor, ChatMedia, ChatMessage, FloatingPanel};
pub use choice::{
    AnchorPoint, AspectRatio, ColorPair, ColorSwatch, FavoriteToggle, Label, NinePointAnchor, Pagination,
    SwatchGrid, ToggleButton, ToggleGroup,
};
pub use clipboard::{Clipboard, ClipboardProvider, MemoryClipboard};
#[cfg(feature = "code")]
pub use code_editor::{CodeDiagnostic, CodeEditor, CodeLanguage, highlight_rust};
pub use color_picker::ColorPicker;
pub use commands::{ConfirmDialog, FloatingToolbar, SplitButton, Toolbar, ToolbarAction};
pub use context::{
    ActivityObserver, BuildCx, Drag, DragHandler, DragPhase, DrawFn, KeyHandler, LiveFn, NodeId, Paint,
    PaintCx, Placement, QuadPaint, RichFn, Semantics, UiActivity, WidgetId,
};
pub use conversation::{Attachment, AttachmentStrip, AudioPlayerButton, SelectionBar};
pub use counter::{KeyBar, KeyBarItem, Scan, ScanField};
pub use data::{Column, List, ListItem, Table};
pub use date::{Date, days_in_month, is_leap_year};
pub use diagnostics::{Diagnostics, DiagnosticsHost};
pub use display::{Alert, AlertAction, Avatar, Badge, Divider, ProgressBar, Section, Skeleton, Stat, Tone};
pub use dock::{
    DockArea, DockChild, DockDrop, DockEdge, DockLayout, DockLayoutError, DockNode, DockPanel, DockTabBar,
};
pub use dock_side::DockSide;
pub use editors::{DateTime, DateTimePicker, MaskedField, NumberField, TextArea, Time, TimePicker};
pub use event_calendar::{CalendarEvent, CalendarView, EventCalendar, RecurrenceEditor};
pub use expression_editor::ExpressionEditor;
pub use extras::{
    AccessibleControl, Carousel, FormField, InputOtp, Menubar, Rule, Spinner, SplitHandle, SplitPane,
    SplitPanes, Splitter, validate,
};
pub use file_picker::{FilePicker, FilePickerProvider, FileRequest, NoFilePicker, PickedFile};
#[cfg(feature = "decode")]
pub use files::ImageField;
pub use files::{DropZone, FileUpload};
pub use fill::Fill;
pub use filter_builder::{FilterBuilder, FilterField, FilterFieldKind};
pub use floating::{
    Combobox, Command, CommandPalette, ContextMenu, FloatingCorner, HoverCard, Sheet, Side, SplashScreen,
    Toast, ToastExtras, ToastPosition, Toaster, Toasts,
};
#[cfg(feature = "flow")]
pub use flow::{
    EdgeShape, FlowCanvas, FlowEdge, FlowEndpoint, FlowGraph, FlowKind, FlowNode, FlowNodeStatus, FlowPort,
    FlowSelection, FlowValidator, FlowViewport, PortDirection, PortSide,
};
pub use form_layout::{FormGroup, FormLayout};
pub use full_screen::{FullScreen, FullScreenProvider};
pub use grid::{
    CellAlign, CellEdit, ColumnLayout, DataGrid, FilterLayout, GridCellStyle, GridColumn, GridCurrent,
    GridData, GridExportFormat, GridLayout, GridPage, GridRequest, GridResponder, GridRowAction,
    GridScenario, GridTable, GridTableColumn, GridTableKind, NumberFormat, RemoteRows, SortDirection,
    SortLayout, Value,
};
pub use hotkey::Hotkey;
pub use icon::Icon;
pub use icon_set::IconName;
#[cfg(feature = "decode")]
pub use image::decode_image;
pub use image::{ContentMode, DecodeError, Image};
pub use imaging::{
    BoxHandle, BoxTransform, ColorWheel, CompareMode, CompareSide, CompareView, CompositionGuide,
    ContentSpace, CropArea, CropOverlay, CurveEditor, Eyedropper, EyedropperArea, EyedropperState, Histogram,
    HistogramChannel, HistogramScale, HueSaturation, ImageLightbox, LightboxItem, Marquee, Minimap,
    Protractor, RubberBand, RuleOfThirdsGrid, Ruler, TransformHandles, ZoomPanPaint, ZoomPanView,
};
pub use inputs::{Checkbox, RadioGroup, SegmentedControl, Slider, Stepper, ThemeSwitcher, Toggle};
pub use kbd::Kbd;
pub use labels::Labels;
pub use link_service::{LinkOpener, Links};
pub use links::Link;
pub use lists::{
    DayMark, DocumentTab, DocumentTabs, FileBrowser, FileEntry, FileSort, FileSortColumn, Filmstrip,
    FilmstripCell, PagedDocumentView, SortableList, VirtualList, YearView,
};
#[cfg(feature = "live")]
#[doc(hidden)]
pub use live::view as __live_view;
#[cfg(feature = "live")]
pub use live::{LiveRunner, live_registry};
pub use localized::Localized;
pub use lookup::{LookupField, LookupItem, LookupRequest, LookupResponder};
#[cfg(feature = "lottie")]
pub use lottie::Lottie;
pub use model_picker::{ModelChoice, ModelPicker, ModelPickerLabels, ModelStatus};
pub use modifiers::{Faded, Styled, ViewExt};
pub use motion::Collapse;
pub use motion_spec::Motion;
pub use nav::{
    Accordion, Breadcrumb, Disclosure, Sidebar, SidebarBrand, SidebarCollapse, SidebarGroup, SidebarInset,
    SidebarItem, SidebarParts, SidebarSection, SidebarSkeleton, SidebarState, SidebarTrigger, SidebarVariant,
    Tabs, TabsVariant,
};
pub use overlay::{Dialog, DropdownMenu, HelpTip, MenuItem, Popover, Select, Tooltip};
pub use page_motion::{Anchor, CountUp, Ticker};
#[cfg(feature = "code")]
pub use playground::{Playground, RunOutput, Runner};
pub use quad_field::QuadField;
pub use quick_edit::{InlineRename, ValueSlider};
#[cfg(feature = "lottie")]
pub use ravenrs_lottie::{Animation as LottieAnimation, LottieError};
pub use recurrence::{
    Frequency, RecurrenceEnd, RecurrenceRule, RecurrenceSet, exceptions_text, parse_exceptions,
};
pub use release_notes::{CodeBlock, ReleaseNote, ReleaseNotes};
pub use remote::UiWaker;
pub use ribbon::{Ribbon, RibbonGroup, RibbonItem, RibbonPage, RibbonSize};
pub use rich_text::RichText;
#[cfg(feature = "charts")]
pub use sankey::{SankeyAlign, SankeyChart};
pub use scheduler::{
    Resource, Scheduler, SchedulerChange, SchedulerEvent, SchedulerRange, TimeAxis, TimePoint, TimeScale,
};
pub use scroll_link::{ScrollLink, ScrollRange};
pub use scroll_scene::{ScrollLinked, ScrollScene, Sticky};
pub use selection_mode::SelectionMode;
pub use shortcuts::{ShortcutHintBar, ShortcutSheet};
pub use sound::{Silence, Sound, SoundClip, SoundProvider};
pub use stack::{
    Footer, Grid, HStack, PageHeader, PropertyRow, ScrollEdgeArrow, ScrollView, Spacer, StatusBar, TitleBar,
    ToolRail, ToolRailItem, VStack, ZStack,
};
pub use status::{
    DaySeparator, Dot, EmptyState, HeroBanner, IconTile, MultiProgress, OverlayBadge, Rating, ShimmerText,
};
pub use summary::Summary;
pub use tag_box::TagBox;
pub use text::Text;
pub use text_field::TextField;
pub use theme::{ThemeEditor, ThemePicker};
pub use tiles::{CardView, Tile, TileLayout, TileSpot};
pub use time_link::TimeLink;
pub use timeline::{Timeline, TimelineEdit, TimelineItem, TimelineMarker, TimelineTrack};
pub use tree::{TreeNode, TreeView};
pub use ui::{Ui, UiStats};
pub use view::{AnyView, Component, FnView, Keyed, Responsive, View, ViewSeq, responsive, view_fn};
pub use wizard::{IndicatorStep, StepBadge, StepIndicator, StepState, Wizard, WizardStep};
pub use zoom::{ZoomControl, ZoomView};
