//! Property tests: an observer is notified exactly when a value it read changes.

use std::cell::Cell;
use std::rc::Rc;

use proptest::prelude::*;
use ravenrs_reactive::{Derived, Observer, State};

proptest! {
    /// Random states, a random subset read by an observer, then random writes:
    /// the observer fires iff some written state was read (and only once).
    #[test]
    fn notifications_follow_dependencies(
        n in 1usize..8,
        reads in proptest::collection::vec(any::<bool>(), 8),
        writes in proptest::collection::vec(0usize..8, 0..12),
    ) {
        let states: Vec<State<i32>> = (0..n).map(|i| State::new(i as i32)).collect();
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        let observer = Observer::new(move || h.set(h.get() + 1));
        observer.track(|| {
            for (i, s) in states.iter().enumerate() {
                if reads[i] {
                    s.get();
                }
            }
        });
        let mut expected = 0;
        for &w in writes.iter().filter(|&&w| w < n) {
            states[w].update(|v| *v += 1);
            if reads[w] && expected == 0 {
                expected = 1;
            }
        }
        prop_assert_eq!(hits.get(), expected);
        prop_assert_eq!(observer.is_dirty(), expected == 1);
    }

    /// A derived value always equals recomputing from scratch.
    #[test]
    fn derived_matches_recomputation(values in proptest::collection::vec(-1000i64..1000, 1..20), edits in proptest::collection::vec((0usize..20, -50i64..50), 0..20)) {
        let states: Rc<Vec<State<i64>>> = Rc::new(values.iter().map(|v| State::new(*v)).collect());
        let s = states.clone();
        let sum = Derived::new(move || s.iter().map(State::get).sum::<i64>());
        for (i, d) in edits {
            if let Some(st) = states.get(i) {
                st.update(|v| *v += d);
            }
            let fresh: i64 = states.iter().map(|s| *s.peek()).sum();
            prop_assert_eq!(sum.get(), fresh);
        }
    }
}
