//! The base of the RavenRS widgets: the runtime ([`Ui`], [`BuildCx`], [`View`]), motion, the
//! services the platform provides, and the basic controls every other component is made of.
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names. The modules are public only so the crates of the components can reach the parts
//! they share; they are hidden from the documentation and are not a stable API.

#[doc(hidden)]
pub mod button;
#[doc(hidden)]
pub mod calendar;
#[doc(hidden)]
pub mod canvas;
#[doc(hidden)]
pub mod card;
#[doc(hidden)]
pub mod choice;
#[doc(hidden)]
pub mod clipboard;
#[doc(hidden)]
pub mod color_picker;
#[doc(hidden)]
pub mod context;
#[doc(hidden)]
pub mod date;
#[doc(hidden)]
pub mod display;
#[doc(hidden)]
pub mod dock_side;
#[doc(hidden)]
pub mod extras;
#[doc(hidden)]
pub mod file_picker;
#[doc(hidden)]
pub mod fill;
#[doc(hidden)]
pub mod floating;
#[doc(hidden)]
pub mod full_screen;
#[doc(hidden)]
pub mod hotkey;
#[doc(hidden)]
pub mod icon;
#[doc(hidden)]
pub mod icon_set;
#[doc(hidden)]
pub mod ids;
#[doc(hidden)]
pub mod image;
#[doc(hidden)]
pub mod inputs;
#[doc(hidden)]
pub mod kbd;
#[doc(hidden)]
pub mod labels;
#[doc(hidden)]
pub mod link_service;
#[doc(hidden)]
pub mod links;
#[cfg(feature = "live")]
#[doc(hidden)]
pub mod live_view;
#[doc(hidden)]
pub mod localized;
#[doc(hidden)]
pub mod modifiers;
#[doc(hidden)]
pub mod motion;
#[doc(hidden)]
pub mod motion_spec;
#[doc(hidden)]
pub mod overlay;
#[doc(hidden)]
pub mod page_motion;
#[doc(hidden)]
pub mod recurrence;
#[doc(hidden)]
pub mod remote;
#[doc(hidden)]
pub mod reorder;
#[doc(hidden)]
pub mod scroll_link;
#[doc(hidden)]
pub mod scroll_scene;
#[doc(hidden)]
pub mod selection_mode;
#[doc(hidden)]
pub mod sound;
#[doc(hidden)]
pub mod stack;
#[doc(hidden)]
pub mod summary;
#[doc(hidden)]
pub mod text;
#[doc(hidden)]
pub mod text_field;
#[doc(hidden)]
pub mod time_link;
#[doc(hidden)]
pub mod ui;
#[doc(hidden)]
pub mod util;
#[doc(hidden)]
pub mod view;
#[doc(hidden)]
pub mod zoom;

pub use button::{Button, ButtonGroup, ButtonSize, ButtonStyle, IconButton};
pub use calendar::{Calendar, DatePicker, DateRange, DateRangePicker};
pub use canvas::Canvas;
pub use card::{Card, GlassCard};
pub use choice::{
    AnchorPoint, AspectRatio, ColorPair, ColorSwatch, FavoriteToggle, Label, NinePointAnchor, Pagination,
    SwatchGrid, ToggleButton, ToggleGroup,
};
pub use clipboard::{Clipboard, ClipboardProvider, MemoryClipboard};
pub use color_picker::ColorPicker;
pub use context::{
    ActivityObserver, BuildCx, Drag, DragHandler, DragPhase, DrawFn, KeyHandler, LiveFn, NodeId, Paint,
    PaintCx, Placement, QuadPaint, RichFn, Semantics, UiActivity, WidgetId,
};
pub use date::{Date, days_in_month, is_leap_year};
pub use display::{Alert, AlertAction, Avatar, Badge, Divider, ProgressBar, Section, Skeleton, Stat, Tone};
pub use dock_side::DockSide;
pub use extras::{
    AccessibleControl, Carousel, FormField, InputOtp, Menubar, Rule, Spinner, SplitHandle, SplitPane,
    SplitPanes, Splitter, validate,
};
pub use file_picker::{FilePicker, FilePickerProvider, FileRequest, NoFilePicker, PickedFile};
pub use fill::Fill;
pub use floating::{
    Combobox, Command, CommandPalette, ContextMenu, FloatingCorner, HoverCard, Sheet, Side, SplashScreen,
    Toast, ToastExtras, ToastPosition, Toaster, Toasts,
};
pub use full_screen::{FullScreen, FullScreenProvider};
pub use hotkey::Hotkey;
pub use icon::Icon;
pub use icon_set::IconName;
#[cfg(feature = "decode")]
pub use image::decode_image;
pub use image::{ContentMode, DecodeError, Image};
pub use inputs::{Checkbox, RadioGroup, SegmentedControl, Slider, Stepper, ThemeSwitcher, Toggle};
pub use kbd::Kbd;
pub use labels::Labels;
pub use link_service::{LinkOpener, Links};
pub use links::Link;
pub use localized::Localized;
pub use modifiers::{Faded, Styled, ViewExt};
pub use motion::Collapse;
pub use motion_spec::Motion;
pub use overlay::{Dialog, DropdownMenu, HelpTip, MenuItem, Popover, Select, Tooltip};
pub use page_motion::{Anchor, CountUp, Ticker};
pub use recurrence::{
    Frequency, RecurrenceEnd, RecurrenceRule, RecurrenceSet, exceptions_text, parse_exceptions,
};
pub use remote::UiWaker;
pub use scroll_link::{ScrollLink, ScrollRange};
pub use scroll_scene::{ScrollLinked, ScrollScene, Sticky};
pub use selection_mode::SelectionMode;
pub use sound::{Silence, Sound, SoundClip, SoundProvider};
pub use stack::{
    Footer, Grid, HStack, PageHeader, PropertyRow, ScrollEdgeArrow, ScrollView, Spacer, StatusBar, TitleBar,
    ToolRail, ToolRailItem, VStack, ZStack,
};
pub use summary::Summary;
pub use text::Text;
pub use text_field::TextField;
pub use time_link::TimeLink;
pub use ui::{Ui, UiStats};
pub use view::{AnyView, Component, FnView, Keyed, Responsive, View, ViewSeq, responsive, view_fn};
pub use zoom::{ZoomControl, ZoomView};
