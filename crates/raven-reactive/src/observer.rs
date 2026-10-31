use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

pub(crate) struct ObserverInner {
    epoch: Cell<u64>,
    dirty: Cell<bool>,
    on_invalidate: Box<dyn Fn()>,
}

thread_local! {
    static TRACKING: RefCell<Vec<Rc<ObserverInner>>> = const { RefCell::new(Vec::new()) };
}

/// Runs a closure while recording which reactive values it reads, and calls
/// `on_invalidate` (once per run) when any of them changes.
pub struct Observer {
    inner: Rc<ObserverInner>,
}

impl Observer {
    pub fn new(on_invalidate: impl Fn() + 'static) -> Self {
        Self {
            inner: Rc::new(ObserverInner {
                epoch: Cell::new(0),
                dirty: Cell::new(true),
                on_invalidate: Box::new(on_invalidate),
            }),
        }
    }

    /// Runs `f`, replacing the previous dependency set with what `f` reads.
    pub fn track<R>(&self, f: impl FnOnce() -> R) -> R {
        self.inner.epoch.set(self.inner.epoch.get().wrapping_add(1));
        self.inner.dirty.set(false);
        TRACKING.with(|t| t.borrow_mut().push(self.inner.clone()));
        // Pops even if `f` panics, so a caught panic cannot leave a stale observer on the stack.
        struct Pop;
        impl Drop for Pop {
            fn drop(&mut self) {
                TRACKING.with(|t| t.borrow_mut().pop());
            }
        }
        let _pop = Pop;
        f()
    }

    /// Runs `f` without recording dependencies for any observer.
    pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
        let saved = TRACKING.with(|t| std::mem::take(&mut *t.borrow_mut()));
        let result = f();
        TRACKING.with(|t| *t.borrow_mut() = saved);
        result
    }

    /// True until the first `track` and after any dependency changed.
    pub fn is_dirty(&self) -> bool {
        self.inner.dirty.get()
    }

    /// Marks the observer dirty and fires its callback, as if a dependency changed.
    pub fn invalidate(&self) {
        self.inner.invalidate();
    }
}

impl fmt::Debug for Observer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observer")
            .field("epoch", &self.inner.epoch.get())
            .field("dirty", &self.inner.dirty.get())
            .finish()
    }
}

impl ObserverInner {
    fn invalidate(&self) {
        if !self.dirty.replace(true) {
            (self.on_invalidate)();
        }
    }
}

/// The set of observers that read a reactive value during their latest run.
#[derive(Default)]
pub(crate) struct Subscribers {
    list: RefCell<Vec<(Weak<ObserverInner>, u64)>>,
}

impl Subscribers {
    /// Subscribes the innermost tracking observer, if any.
    pub(crate) fn track_read(&self) {
        let Some(current) = TRACKING.with(|t| t.borrow().last().cloned()) else {
            return;
        };
        let epoch = current.epoch.get();
        let mut list = self.list.borrow_mut();
        let already = list.iter().any(|(w, e)| *e == epoch && w.as_ptr() == Rc::as_ptr(&current));
        if !already {
            list.push((Rc::downgrade(&current), epoch));
        }
    }

    /// Invalidates every live, current subscriber and clears the list. Observers
    /// re-subscribe the next time they run.
    pub(crate) fn notify(&self) {
        let list = std::mem::take(&mut *self.list.borrow_mut());
        for (weak, epoch) in list {
            if let Some(observer) = weak.upgrade()
                && observer.epoch.get() == epoch
            {
                observer.invalidate();
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.list.borrow().len()
    }
}
