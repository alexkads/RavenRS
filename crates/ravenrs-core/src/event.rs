use crate::Point;

/// Platform-neutral input event, already converted to logical pixels.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// The pointer moved without a button change.
    PointerMove {
        /// Pointer position in logical pixels.
        position: Point,
    },
    /// A pointer button was pressed.
    PointerDown {
        /// Pointer position in logical pixels.
        position: Point,
        /// Button that was pressed.
        button: PointerButton,
    },
    /// A pointer button was released.
    PointerUp {
        /// Pointer position in logical pixels.
        position: Point,
        /// Button that was released.
        button: PointerButton,
    },
    /// A second primary click in quick succession, sent after its `PointerUp`.
    DoubleClick {
        /// Pointer position in logical pixels.
        position: Point,
    },
    /// The pointer left the window or canvas.
    PointerLeave,
    /// Wheel or trackpad scrolling, in logical pixels. Positive `delta.y` means
    /// the user scrolled up (content should move down), as on every platform.
    Scroll {
        /// Pointer position in logical pixels.
        position: Point,
        /// Scroll distance in logical pixels on each axis.
        delta: Point,
    },
    /// A pinch on a trackpad or Ctrl with the wheel: a zoom gesture.
    Pinch {
        /// Pointer position in logical pixels.
        position: Point,
        /// How much larger the user asked for, relative to now: 0.02 is 2% larger,
        /// and a negative value is smaller.
        delta: f32,
    },
    /// A key was pressed (including auto-repeat).
    KeyDown {
        /// Key that was pressed.
        key: Key,
        /// Modifier keys held at the time of the press.
        modifiers: Modifiers,
    },
    /// A key was released.
    KeyUp {
        /// Key that was released.
        key: Key,
        /// Modifier keys held at the time of the release.
        modifiers: Modifiers,
    },
    /// The window or canvas lost keyboard focus.
    FocusLost,
}

/// Mouse button or equivalent touch/pen action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PointerButton {
    /// Left mouse button, or a primary touch/pen contact.
    Primary,
    /// Right mouse button (context menu).
    Secondary,
    /// Middle mouse button or wheel click.
    Auxiliary,
    /// Any other button, by platform index.
    Other(u16),
}

/// The subset of keys the framework itself reacts to; everything else is
/// forwarded as `Character` or `Other` for widgets to interpret.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// Enter or Return.
    Enter,
    /// Space bar.
    Space,
    /// Tab (focus traversal).
    Tab,
    /// Escape.
    Escape,
    /// Backspace (delete before the caret).
    Backspace,
    /// Forward delete (delete after the caret).
    Delete,
    /// Home (start of line).
    Home,
    /// End (end of line).
    End,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Text produced by the key, already composed with modifiers and layout.
    Character(String),
    /// Any other named key, by its platform name.
    Other(String),
}

/// Modifier keys held during a key event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift key held.
    pub shift: bool,
    /// Control key held.
    pub ctrl: bool,
    /// Alt / Option key held.
    pub alt: bool,
    /// Meta key held (Command on macOS, Windows key elsewhere).
    pub meta: bool,
}

/// Cursor the UI asks the platform to show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CursorIcon {
    /// Platform default arrow.
    #[default]
    Default,
    /// Hand cursor for clickable elements.
    Pointer,
    /// I-beam cursor for editable text.
    Text,
    /// Horizontal resize, e.g. a column edge.
    ColResize,
    /// Vertical resize, e.g. a horizontal splitter bar.
    RowResize,
}
