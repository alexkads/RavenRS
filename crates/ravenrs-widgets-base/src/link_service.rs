//! The links service: opening addresses and jumping to anchors, as the platform layer
//! provides it.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Opens an address outside the app. Platform layers install one with
/// [`crate::Ui::set_links`]: the browser navigates, the desktop asks the system's browser.
pub trait LinkOpener {
    /// Opens `url`, absolute or relative to the page.
    fn open(&self, url: &str);
}

/// Keeps the addresses it is asked to open, for tests and headless use.
#[derive(Debug, Default)]
struct Recorder;

impl LinkOpener for Recorder {
    fn open(&self, url: &str) {
        tracing::info!(url, "no link opener installed");
    }
}

/// A shared handle to the links service, from [`crate::BuildCx::links`] or
/// [`crate::Ui::links`]. An address that starts with `#` scrolls to the anchor of that
/// name ([`crate::BuildCx::anchor`]) instead of leaving the app.
///
/// ```
/// use ravenrs_widgets::Links;
///
/// let links = Links::default();
/// links.open("https://example.com/docs");
/// assert_eq!(links.opened(), ["https://example.com/docs"]);
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Links {
    opener: Rc<RefCell<Option<Rc<dyn LinkOpener>>>>,
    anchor: Rc<RefCell<Option<String>>>,
    opened: Rc<RefCell<Vec<String>>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Links {
    /// Opens `url`. `#name` scrolls to that anchor on the next frame.
    pub fn open(&self, url: &str) {
        if let Some(name) = url.strip_prefix('#') {
            *self.anchor.borrow_mut() = Some(name.to_owned());
            return;
        }
        self.opened.borrow_mut().push(url.to_owned());
        let opener = self.opener.borrow().clone().unwrap_or_else(|| Rc::new(Recorder));
        opener.open(url);
    }

    /// Every address opened outside the app so far, oldest first.
    pub fn opened(&self) -> Vec<String> {
        self.opened.borrow().clone()
    }

    #[doc(hidden)]
    pub fn set_opener(&self, opener: impl LinkOpener + 'static) {
        *self.opener.borrow_mut() = Some(Rc::new(opener));
    }

    /// The anchor a link asked for, once.
    #[doc(hidden)]
    pub fn take_anchor(&self) -> Option<String> {
        self.anchor.borrow_mut().take()
    }
}

impl fmt::Debug for Links {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Links").field("opened", &self.opened.borrow().len()).finish_non_exhaustive()
    }
}
