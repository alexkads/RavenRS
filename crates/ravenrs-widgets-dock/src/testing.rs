//! [`DockDriver`]: what a test does to a `DockArea` as a person does it with the keyboard, by
//! the panels' ids, never by pixels or by reading the layout.
//!
//! It only uses `ravenrs-e2e`'s public locators and actions on the ids the area gives a test:
//! `panel:<id>` on each tab, `floating:<id>` on a floating panel and `floating-move:<id>` on
//! its bar.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_widgets_dock::testing::DockDriver;
//! # fn run(e2e: &mut E2e) -> ravenrs_e2e::E2eResult {
//! let mut dock = DockDriver::new(e2e);
//! dock.float("output")?;
//! dock.dock_back("output")?;
//! # Ok(()) }
//! ```

use ravenrs_core::{Key, Modifiers};
use ravenrs_e2e::{E2e, E2eResult, ErrorCode};

/// Floats and docks the panels of a `DockArea` in an [`E2e`] session.
pub struct DockDriver<'a> {
    e2e: &'a mut E2e,
}

impl std::fmt::Debug for DockDriver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockDriver").finish_non_exhaustive()
    }
}

/// The key that floats a panel or docks it back: Ctrl+Alt+F, or Cmd+Alt+F with Apple's keys.
fn float_keys() -> Modifiers {
    let apple = crate::apple_keys();
    Modifiers { meta: apple, ctrl: !apple, alt: true, ..Modifiers::default() }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<'a> DockDriver<'a> {
    /// Drives the dock area on the screen.
    pub fn new(e2e: &'a mut E2e) -> Self {
        Self { e2e }
    }

    /// Lifts the panel out of its group to float, with its tab's key, and checks it floats. A
    /// panel of an area that does not float fails with `E2E_EXPECTATION`.
    pub fn float(&mut self, panel: &str) -> E2eResult {
        self.e2e.by_test_id(format!("panel:{panel}")).click()?;
        self.e2e.press_with(Key::Character("f".into()), float_keys());
        if self.e2e.by_test_id(format!("floating:{panel}")).count()? == 1 {
            return Ok(());
        }
        Err(self
            .e2e
            .failure(ErrorCode::Expectation, format!("#panel:{panel}: it did not float"))
            .hint("the area floats panels only when it is floatable"))
    }

    /// Docks a floating panel back where it came from, with its bar's key, and checks its tab
    /// is back.
    pub fn dock_back(&mut self, panel: &str) -> E2eResult {
        self.e2e.by_test_id(format!("floating-move:{panel}")).click()?;
        self.e2e.press_with(Key::Character("f".into()), float_keys());
        if self.e2e.by_test_id(format!("floating:{panel}")).count()? == 0 {
            return Ok(());
        }
        Err(self.e2e.failure(ErrorCode::Expectation, format!("#floating:{panel}: it is still floating")))
    }
}
