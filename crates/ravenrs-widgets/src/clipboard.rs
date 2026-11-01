//! The system clipboard, as a service the platform layer provides.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Reads and writes the system clipboard. Platform layers install one with
/// [`crate::Ui::set_clipboard`]; until then, an in-memory clipboard is used.
pub trait ClipboardProvider {
    /// The clipboard's text, or `None` when it is empty, not text, or cannot
    /// be read synchronously (in the browser, pasting arrives as typed text).
    fn read_text(&self) -> Option<String>;
    /// Replaces the clipboard's content with `text`.
    fn write_text(&self, text: &str);
}

/// A clipboard that lives only inside the app (tests, headless use).
#[derive(Debug, Default)]
pub struct MemoryClipboard(RefCell<Option<String>>);

impl ClipboardProvider for MemoryClipboard {
    fn read_text(&self) -> Option<String> {
        self.0.borrow().clone()
    }

    fn write_text(&self, text: &str) {
        *self.0.borrow_mut() = Some(text.to_owned());
    }
}

/// A shared handle to the clipboard, from [`crate::BuildCx::clipboard`] or
/// [`crate::Ui::clipboard`].
///
/// ```
/// use ravenrs_widgets::{Clipboard, MemoryClipboard};
///
/// let clipboard = Clipboard::new(MemoryClipboard::default());
/// clipboard.write("hello");
/// assert_eq!(clipboard.read().as_deref(), Some("hello"));
/// ```
#[derive(Clone)]
pub struct Clipboard(Rc<dyn ClipboardProvider>);

impl Clipboard {
    /// Wraps a provider.
    pub fn new(provider: impl ClipboardProvider + 'static) -> Self {
        Self(Rc::new(provider))
    }

    /// The clipboard's text, if it can be read now.
    pub fn read(&self) -> Option<String> {
        self.0.read_text()
    }

    /// Replaces the clipboard's content with `text`.
    pub fn write(&self, text: &str) {
        self.0.write_text(text);
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new(MemoryClipboard::default())
    }
}

impl fmt::Debug for Clipboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Clipboard")
    }
}
