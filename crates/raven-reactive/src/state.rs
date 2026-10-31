use std::cell::{Ref, RefCell};
use std::fmt;
use std::rc::Rc;

use crate::Binding;
use crate::observer::Subscribers;

struct StateInner<T> {
    value: RefCell<T>,
    subscribers: Subscribers,
}

/// A mutable value that notifies the observers that read it. Cloning a `State`
/// clones the handle, not the value: all clones share one value.
pub struct State<T> {
    inner: Rc<StateInner<T>>,
}

impl<T: 'static> State<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(StateInner {
                value: RefCell::new(value),
                subscribers: Subscribers::default(),
            }),
        }
    }

    /// Returns a clone of the value and records the read.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    /// Reads the value by reference and records the read.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.inner.subscribers.track_read();
        f(&self.inner.value.borrow())
    }

    /// Borrows the value without recording a dependency.
    pub fn peek(&self) -> Ref<'_, T> {
        self.inner.value.borrow()
    }

    /// Replaces the value and notifies dependents.
    pub fn set(&self, value: T) {
        *self.inner.value.borrow_mut() = value;
        self.inner.subscribers.notify();
    }

    /// Replaces the value only if it differs, notifying only on change.
    pub fn set_if_changed(&self, value: T)
    where
        T: PartialEq,
    {
        let changed = *self.inner.value.borrow() != value;
        if changed {
            self.set(value);
        }
    }

    /// Mutates the value in place and notifies dependents.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        f(&mut self.inner.value.borrow_mut());
        self.inner.subscribers.notify();
    }

    /// A read/write view of this state for child components.
    pub fn binding(&self) -> Binding<T>
    where
        T: Clone,
    {
        let (read, write) = (self.clone(), self.clone());
        Binding::new(move || read.get(), move |v| write.set(v))
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count(&self) -> usize {
        self.inner.subscribers.len()
    }
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.value.try_borrow() {
            Ok(v) => f.debug_tuple("State").field(&*v).finish(),
            Err(_) => f.write_str("State(<borrowed>)"),
        }
    }
}

impl<T: Default + 'static> Default for State<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::Observer;

    fn counting_observer() -> (Observer, Rc<Cell<u32>>) {
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        (Observer::new(move || h.set(h.get() + 1)), hits)
    }

    #[test]
    fn notifies_only_observers_that_read() {
        let a = State::new(1);
        let b = State::new(10);
        let (obs, hits) = counting_observer();
        obs.track(|| a.get());
        b.set(11);
        assert_eq!(hits.get(), 0, "b was never read");
        a.set(2);
        assert_eq!(hits.get(), 1);
        assert!(obs.is_dirty());
    }

    #[test]
    fn callback_fires_once_until_retracked() {
        let a = State::new(0);
        let (obs, hits) = counting_observer();
        obs.track(|| a.get());
        a.set(1);
        a.set(2);
        assert_eq!(hits.get(), 1);
        obs.track(|| a.get());
        a.update(|v| *v += 1);
        assert_eq!(hits.get(), 2);
    }

    #[test]
    fn dependencies_are_replaced_on_each_run() {
        let a = State::new(0);
        let b = State::new(0);
        let (obs, hits) = counting_observer();
        obs.track(|| a.get());
        obs.track(|| b.get());
        a.set(5);
        assert_eq!(hits.get(), 0, "a is no longer a dependency");
        b.set(5);
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn set_if_changed_skips_equal_values() {
        let a = State::new(3);
        let (obs, hits) = counting_observer();
        obs.track(|| a.get());
        a.set_if_changed(3);
        assert_eq!(hits.get(), 0);
        a.set_if_changed(4);
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn dropped_observers_are_pruned() {
        let a = State::new(0);
        {
            let (obs, _) = counting_observer();
            obs.track(|| a.get());
            assert_eq!(a.subscriber_count(), 1);
        }
        a.set(1);
        assert_eq!(a.subscriber_count(), 0);
    }

    #[test]
    fn reads_are_deduplicated_and_untracked_reads_ignored() {
        let a = State::new(0);
        let (obs, hits) = counting_observer();
        obs.track(|| {
            a.get();
            a.get();
            Observer::untracked(|| a.get());
        });
        assert_eq!(a.subscriber_count(), 1);
        a.set(1);
        assert_eq!(hits.get(), 1);
    }
}
