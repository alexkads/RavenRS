//! [`SchedulerDriver`]: what a test does to a `Scheduler` as a person does it with the
//! keyboard, by the ids of events and resources, never by pixels or by reading its state.
//!
//! It only uses `ravenrs-e2e`'s public locators and actions on the ids the scheduler gives a
//! test: `event:<id>` on each bar and `resource:<id>` on each resource's row. The keyboard is
//! exact where a drag is not: one arrow is one step of the scheduler's snap.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_widgets_scheduler::testing::SchedulerDriver;
//! # fn run(e2e: &mut E2e) -> ravenrs_e2e::E2eResult {
//! let mut plan = SchedulerDriver::new(e2e);
//! plan.move_event(7, 2)?; // two steps later
//! plan.resize_event(7, -1)?; // one step shorter
//! plan.move_to_resource(7, 3)?;
//! # Ok(()) }
//! ```

use ravenrs_core::{Key, Modifiers};
use ravenrs_e2e::{E2e, E2eResult, ErrorCode};

/// How many rows [`SchedulerDriver::move_to_resource`] walks before it gives up.
const MOST_ROWS: usize = 500;

/// Moves and resizes the events of a `Scheduler` in an [`E2e`] session.
pub struct SchedulerDriver<'a> {
    e2e: &'a mut E2e,
}

impl std::fmt::Debug for SchedulerDriver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchedulerDriver").finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<'a> SchedulerDriver<'a> {
    /// Drives the scheduler on the screen.
    pub fn new(e2e: &'a mut E2e) -> Self {
        Self { e2e }
    }

    /// Moves the event `steps` steps of the snap later (or earlier, when negative), with the
    /// arrow keys, keeping its length.
    pub fn move_event(&mut self, event: u64, steps: i32) -> E2eResult {
        self.arrows(event, steps, Modifiers::default())
    }

    /// Makes the event `steps` steps of the snap longer (or shorter), with Shift and the arrow
    /// keys. An event that is not resizable fails with `E2E_EXPECTATION`.
    pub fn resize_event(&mut self, event: u64, steps: i32) -> E2eResult {
        self.arrows(event, steps, Modifiers { shift: true, ..Modifiers::default() })
    }

    /// Takes the event to the row of `resource`, keeping its time, with the up and down arrows.
    /// A resource the scheduler does not show fails with `E2E_NOT_FOUND`, before anything moves.
    pub fn move_to_resource(&mut self, event: u64, resource: u64) -> E2eResult {
        let row = format!("resource:{resource}");
        self.e2e.by_test_id(row.clone()).node()?;
        self.focus(event)?;
        let id = format!("event:{event}");
        for _ in 0..MOST_ROWS {
            let (bar, band) = (
                self.e2e.by_test_id(id.clone()).node()?.bounds,
                self.e2e.by_test_id(row.clone()).node()?.bounds,
            );
            let y = bar.y() + bar.height() / 2.0;
            if y >= band.y() && y <= band.y() + band.height() {
                return Ok(());
            }
            self.e2e.press(if y < band.y() { Key::ArrowDown } else { Key::ArrowUp });
            if self.e2e.by_test_id(id.clone()).node()?.bounds == bar {
                return Err(self.stuck(event));
            }
        }
        Err(self.stuck(event))
    }

    /// Clicks the event so it takes the focus the arrows need.
    fn focus(&mut self, event: u64) -> E2eResult {
        let id = format!("event:{event}");
        self.e2e.by_test_id(id.clone()).click()?;
        let node = self.e2e.by_test_id(id.clone()).node()?;
        if self.e2e.tree().focus == Some(node.id) {
            return Ok(());
        }
        Err(self
            .e2e
            .failure(ErrorCode::Expectation, format!("#{id}: a click did not give it the focus"))
            .hint("give the Scheduler a selection binding, so a clicked event is the one the keys move"))
    }

    fn arrows(&mut self, event: u64, steps: i32, modifiers: Modifiers) -> E2eResult {
        if steps == 0 {
            return Ok(());
        }
        self.focus(event)?;
        let id = format!("event:{event}");
        let before = self.e2e.by_test_id(id.clone()).node()?.description;
        let key = if steps < 0 { Key::ArrowLeft } else { Key::ArrowRight };
        for _ in 0..steps.unsigned_abs() {
            self.e2e.press_with(key.clone(), modifiers);
        }
        if self.e2e.by_test_id(id).node()?.description == before {
            return Err(self.stuck(event));
        }
        Ok(())
    }

    fn stuck(&self, event: u64) -> ravenrs_e2e::E2eError {
        self.e2e
            .failure(ErrorCode::Expectation, format!("#event:{event}: it did not move"))
            .hint("a locked event, one that is not resizable, or a scheduler that is not editable stays where it is")
    }
}
