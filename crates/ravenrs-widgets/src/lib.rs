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
mod choice;
mod clipboard;
mod context;
mod data;
mod date;
mod display;
mod extras;
mod fill;
mod floating;
pub mod grid;
mod icon;
mod icon_set;
mod image;
mod inputs;
mod labels;
mod modifiers;
mod motion;
mod nav;
mod overlay;
mod ribbon;
mod stack;
mod text;
mod text_field;
mod ui;
mod util;
mod view;

pub use button::{Button, ButtonSize, ButtonStyle, IconButton};
pub use calendar::{Calendar, DatePicker};
pub use canvas::Canvas;
pub use card::{Card, GlassCard};
#[cfg(feature = "charts")]
pub use chart::{AreaChart, BarChart, Barcode, Chart, ChartKind, Symbology};
pub use choice::{AspectRatio, ColorSwatch, Label, Pagination, ToggleButton, ToggleGroup};
pub use clipboard::{Clipboard, ClipboardProvider, MemoryClipboard};
pub use context::{
    BuildCx, Drag, DragHandler, DragPhase, DrawFn, KeyHandler, LiveFn, NodeId, Paint, PaintCx, Placement,
    QuadPaint, RichFn, Semantics, WidgetId,
};
pub use data::{Column, List, ListItem, Table};
pub use date::{Date, days_in_month, is_leap_year};
pub use display::{Alert, Avatar, Badge, Divider, Kbd, ProgressBar, Section, Skeleton, Stat, Tone};
pub use extras::{Carousel, FormField, InputOtp, Menubar, Rule, Spinner, Splitter, validate};
pub use fill::Fill;
pub use floating::{
    Combobox, Command, CommandPalette, ContextMenu, HoverCard, Sheet, Side, SplashScreen, Toast, Toaster,
    Toasts,
};
pub use grid::{
    CellAlign, DataGrid, GridColumn, GridData, NumberFormat, SelectionMode, SortDirection, Summary, Value,
};
pub use icon::Icon;
pub use icon_set::IconName;
#[cfg(feature = "decode")]
pub use image::decode_image;
pub use image::{ContentMode, DecodeError, Image};
pub use inputs::{Checkbox, RadioGroup, SegmentedControl, Slider, Stepper, ThemeSwitcher, Toggle};
pub use labels::{Labels, Localized};
pub use modifiers::{Styled, ViewExt};
pub use motion::{Collapse, Motion};
pub use nav::{
    Accordion, Breadcrumb, Disclosure, Sidebar, SidebarCollapse, SidebarGroup, SidebarInset, SidebarItem,
    SidebarParts, SidebarSection, SidebarSkeleton, SidebarState, SidebarTrigger, SidebarVariant, Tabs,
};
pub use overlay::{Dialog, DropdownMenu, MenuItem, Popover, Select, Tooltip};
pub use ribbon::{Ribbon, RibbonGroup, RibbonItem, RibbonPage, RibbonSize};
pub use stack::{Footer, Grid, HStack, ScrollView, Spacer, VStack};
pub use text::Text;
pub use text_field::TextField;
pub use ui::Ui;
pub use view::{AnyView, Component, FnView, Keyed, Responsive, View, ViewSeq, responsive, view_fn};
