//! Declarative views and the UI runtime.
//!
//! # Writing components
//!
//! There are three ways to make your own component, from simplest to most powerful:
//!
//! 1. **A function** returning `impl View`, composing existing views. Good for
//!    stateless pieces of UI.
//! 2. **A [`Component`]**: a struct holding its state ([`State`](raven_reactive::State)
//!    fields, bindings, callbacks) with a `body()` that composes views. This is the
//!    SwiftUI-style building block and covers most needs.
//! 3. **A [`View`]** implemented directly: full control over layout boxes,
//!    painting (including arbitrary [`Scene`](raven_core::Scene) drawing through
//!    [`Paint::Custom`]), hit-testing, focus and click handling via [`BuildCx`].
//!    The built-in widgets are written this way, with no private API.
//!
//! [`Canvas`] is the shortcut for custom drawing inside a normal layout.
//!
//! # Runtime
//!
//! [`Ui`] owns the root view and turns it into a [`Scene`](raven_core::Scene):
//! *build* (views to elements, tracked by a reactive observer), *layout* (Taffy),
//! *paint*. It only rebuilds when state the last build read has changed or the
//! interaction state (hover, press, focus) changed, and it never touches the GPU.

mod button;
mod canvas;
mod card;
mod context;
mod data;
mod display;
mod fill;
pub mod grid;
mod icon;
mod icon_set;
mod image;
mod inputs;
mod modifiers;
mod nav;
mod overlay;
mod stack;
mod text;
mod text_field;
mod ui;
mod util;
mod view;

pub use button::{Button, ButtonSize, ButtonStyle, IconButton};
pub use canvas::Canvas;
pub use card::{Card, GlassCard};
pub use context::{
    BuildCx, Drag, DragHandler, DragPhase, DrawFn, KeyHandler, NodeId, Paint, Placement, QuadPaint, WidgetId,
};
pub use data::{AreaChart, BarChart, Column, List, ListItem, Table};
pub use display::{Alert, Avatar, Badge, Divider, Kbd, ProgressBar, Section, Skeleton, Stat, Tone};
pub use fill::Fill;
pub use grid::{
    CellAlign, DataGrid, GridColumn, GridData, NumberFormat, SelectionMode, SortDirection, Summary, Value,
};
pub use icon::Icon;
pub use icon_set::IconName;
#[cfg(feature = "decode")]
pub use image::decode_image;
pub use image::{ContentMode, DecodeError, Image};
pub use inputs::{Checkbox, RadioGroup, SegmentedControl, Slider, Stepper, ThemeSwitcher, Toggle};
pub use modifiers::{Styled, ViewExt};
pub use nav::{Accordion, Breadcrumb, Disclosure, Sidebar, SidebarItem, SidebarSection, Tabs};
pub use overlay::{Dialog, DropdownMenu, MenuItem, Popover, Select, Tooltip};
pub use stack::{Grid, HStack, ScrollView, Spacer, VStack};
pub use text::Text;
pub use text_field::TextField;
pub use ui::Ui;
pub use view::{AnyView, Component, FnView, Keyed, Responsive, View, ViewSeq, responsive, view_fn};
