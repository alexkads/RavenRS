//! [`SortableDriver`]: what a test does to a `SortableList` as a person does it, by the keys of
//! its items, never by pixels or by reading the list's state.
//!
//! It only uses `ravenrs-e2e`'s public locators and actions on the ids the list gives a test
//! when it has a [`key`](crate::lists::SortableList::key): `item:<key>` on each item.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_widgets_lists::testing::SortableDriver;
//! # fn run(e2e: &mut E2e) -> ravenrs_e2e::E2eResult {
//! SortableDriver::new(e2e).move_before(2, 1)?;
//! # Ok(()) }
//! ```

use ravenrs_core::{Key, Modifiers};
use ravenrs_e2e::{E2e, E2eResult, ErrorCode};

/// The most steps a move takes: one per place, in the longest list a test reorders.
const MOST_STEPS: usize = 10_000;

/// Reorders a `SortableList` in an [`E2e`] session.
pub struct SortableDriver<'a> {
    e2e: &'a mut E2e,
}

impl std::fmt::Debug for SortableDriver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SortableDriver").finish_non_exhaustive()
    }
}

impl<'a> SortableDriver<'a> {
    /// Drives the sortable list on the screen.
    pub fn new(e2e: &'a mut E2e) -> Self {
        Self { e2e }
    }

    /// The keys of the items, in the order the list shows them.
    pub fn order(&self) -> Vec<u64> {
        self.e2e
            .snapshot()
            .lines()
            .filter_map(|line| line.split("#item:").nth(1))
            .filter_map(|rest| rest.split_whitespace().next()?.parse().ok())
            .collect()
    }

    /// Moves the item `key` to just before the item `before`, as a person does with the keyboard:
    /// focus on the item, then Alt with an arrow, one place at a time.
    pub fn move_before(&mut self, key: u64, before: u64) -> E2eResult {
        self.e2e.by_test_id(format!("item:{key}")).click()?;
        let alt = Modifiers { alt: true, ..Modifiers::default() };
        for _ in 0..MOST_STEPS {
            let order = self.order();
            let (Some(at), Some(target)) =
                (order.iter().position(|k| *k == key), order.iter().position(|k| *k == before))
            else {
                return Err(self.e2e.failure(
                    ErrorCode::NotFound,
                    format!("item:{key} or item:{before} is not in the list ({order:?})"),
                ));
            };
            if at + 1 == target {
                return Ok(());
            }
            self.e2e.press_with(if at > target { Key::ArrowUp } else { Key::ArrowDown }, alt);
            if self.order() == order {
                return Err(self.e2e.failure(
                    ErrorCode::Expectation,
                    format!("item:{key} did not move with Alt and the arrows ({order:?})"),
                ));
            }
        }
        Ok(())
    }
}
