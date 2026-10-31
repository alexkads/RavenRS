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
    /// The pointer left the window or canvas.
    PointerLeave,
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
}
