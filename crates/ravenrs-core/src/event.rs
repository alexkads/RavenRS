use crate::Point;

/// Platform-neutral input event, already converted to logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    PointerMove {
        position: Point,
    },
    PointerDown {
        position: Point,
        button: PointerButton,
    },
    PointerUp {
        position: Point,
        button: PointerButton,
    },
    /// A second primary click in quick succession, sent after its `PointerUp`.
    DoubleClick {
        position: Point,
    },
    /// The pointer left the window or canvas.
    PointerLeave,
    /// Wheel or trackpad scrolling, in logical pixels. Positive `delta.y` means
    /// the user scrolled up (content should move down), as on every platform.
    Scroll {
        position: Point,
        delta: Point,
    },
    KeyDown {
        key: Key,
        modifiers: Modifiers,
    },
    KeyUp {
        key: Key,
        modifiers: Modifiers,
    },
    /// The window or canvas lost keyboard focus.
    FocusLost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Secondary,
    Auxiliary,
    Other(u16),
}

/// The subset of keys the framework itself reacts to; everything else is
/// forwarded as `Character` or `Other` for widgets to interpret.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Enter,
    Space,
    Tab,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Character(String),
    Other(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Cursor the UI asks the platform to show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorIcon {
    #[default]
    Default,
    Pointer,
    Text,
    /// Horizontal resize, e.g. a column edge.
    ColResize,
    /// Vertical resize, e.g. a horizontal splitter bar.
    RowResize,
}
