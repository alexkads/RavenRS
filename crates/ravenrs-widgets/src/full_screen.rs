//! Full screen, as a service the platform layer provides.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

/// Puts the app in full screen and takes it out. Platform layers install one with
/// [`crate::Ui::set_full_screen`]: the browser asks for the page's full screen, the
/// desktop turns the window into a borderless full screen.
pub trait FullScreenProvider {
    /// Whether the app fills the screen now.
    fn is_on(&self) -> bool;

    /// Asks for full screen, or to leave it. The answer may come later, as a resize.
    fn set(&self, on: bool);

    /// Whether this place can show the app in full screen at all; a page inside a
    /// frame that does not allow it cannot.
    fn available(&self) -> bool {
        true
    }
}

/// Only remembers the request, for tests and headless use.
#[derive(Debug, Default)]
struct Remembered(Cell<bool>);

impl FullScreenProvider for Remembered {
    fn is_on(&self) -> bool {
        self.0.get()
    }

    fn set(&self, on: bool) {
        self.0.set(on);
    }
}

/// A shared handle to the full screen service, from [`crate::BuildCx::full_screen`] or
/// [`crate::Ui::full_screen`].
///
/// ```
/// use ravenrs_widgets::FullScreen;
///
/// let screen = FullScreen::default();
/// screen.toggle();
/// assert!(screen.is_on());
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FullScreen {
    provider: Rc<dyn FullScreenProvider>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FullScreen {
    /// The service backed by `provider`.
    pub fn new(provider: impl FullScreenProvider + 'static) -> Self {
        Self { provider: Rc::new(provider) }
    }

    /// Whether the app fills the screen now.
    pub fn is_on(&self) -> bool {
        self.provider.is_on()
    }

    /// Asks for full screen, or to leave it.
    pub fn set(&self, on: bool) {
        self.provider.set(on);
    }

    /// Enters full screen when out of it, and leaves it when in.
    pub fn toggle(&self) {
        self.set(!self.is_on());
    }

    /// Whether full screen can be asked for here.
    pub fn available(&self) -> bool {
        self.provider.available()
    }
}

impl Default for FullScreen {
    fn default() -> Self {
        Self::new(Remembered::default())
    }
}

impl fmt::Debug for FullScreen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FullScreen").field("on", &self.is_on()).finish_non_exhaustive()
    }
}
