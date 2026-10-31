use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

use crate::Observer;
use crate::observer::Subscribers;

struct DerivedInner<T> {
    compute: Box<dyn Fn() -> T>,
    cache: RefCell<Option<T>>,
    observer: Observer,
    subscribers: Subscribers,
}

/// A cached value computed from other reactive values. It recomputes lazily, on
/// the first read after a dependency changed, and notifies its own readers when
/// that happens.
pub struct Derived<T> {
    inner: Rc<DerivedInner<T>>,
}

impl<T: Clone + 'static> Derived<T> {
    pub fn new(compute: impl Fn() -> T + 'static) -> Self {
        let inner = Rc::new_cyclic(|weak: &Weak<DerivedInner<T>>| {
            let weak = weak.clone();
            DerivedInner {
                compute: Box::new(compute),
                cache: RefCell::new(None),
                observer: Observer::new(move || {
                    if let Some(inner) = weak.upgrade() {
                        inner.subscribers.notify();
                    }
                }),
                subscribers: Subscribers::default(),
            }
        });
        Self { inner }
    }

    pub fn get(&self) -> T {
        self.inner.subscribers.track_read();
        if self.inner.observer.is_dirty() || self.inner.cache.borrow().is_none() {
            let value = self.inner.observer.track(|| (self.inner.compute)());
            *self.inner.cache.borrow_mut() = Some(value.clone());
            return value;
        }
        match &*self.inner.cache.borrow() {
            Some(v) => v.clone(),
            // Unreachable: the branch above fills the cache when it is empty.
            None => (self.inner.compute)(),
        }
    }
}

impl<T> Clone for Derived<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for Derived<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Derived")
            .field("dirty", &self.inner.observer.is_dirty())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::{Derived, Observer, State};

    #[test]
    fn recomputes_lazily_and_caches() {
        let runs = Rc::new(Cell::new(0));
        let count = State::new(2);
        let (r, c) = (runs.clone(), count.clone());
        let doubled = Derived::new(move || {
            r.set(r.get() + 1);
            c.get() * 2
        });
        assert_eq!(doubled.get(), 4);
        assert_eq!(doubled.get(), 4);
        assert_eq!(runs.get(), 1);
        count.set(5);
        assert_eq!(runs.get(), 1, "not recomputed until read");
        assert_eq!(doubled.get(), 10);
        assert_eq!(runs.get(), 2);
    }

    #[test]
    fn propagates_to_its_readers() {
        let count = State::new(1);
        let c = count.clone();
        let label = Derived::new(move || format!("Clicks: {}", c.get()));
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        let ui = Observer::new(move || h.set(h.get() + 1));
        ui.track(|| label.get());
        count.set(2);
        assert_eq!(hits.get(), 1);
        assert_eq!(ui.track(|| label.get()), "Clicks: 2");
    }
}
