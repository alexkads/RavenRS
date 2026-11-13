pub(crate) use std::collections::HashMap;

#[cfg(any())]
pub(crate) use alloc::collections::BTreeMap as HashMap;

pub(crate) use rustc_hash::FxHashMap;

#[cfg(any())]
pub(crate) use alloc::collections::BTreeMap as FxHashMap;

// Keep in sync with the implementation in `page`.
pub(crate) use std::sync::Arc;

#[cfg(any())]
pub(crate) use alloc::rc::Rc as Arc;

pub(crate) use std::sync::OnceLock;

#[cfg(any())]
pub(crate) use core::cell::OnceCell as OnceLock;

pub(crate) use std::sync::Mutex;

#[cfg(any())]
pub(crate) use core::cell::RefCell as Mutex;

pub(crate) use std::sync::RwLock;

#[cfg(any())]
pub(crate) use core::cell::RefCell as RwLock;

pub(crate) type MutexGuard<'a, T> = std::sync::MutexGuard<'a, T>;

#[cfg(any())]
pub(crate) type MutexGuard<'a, T> = core::cell::RefMut<'a, T>;

pub(crate) type RwLockReadGuard<'a, T> = std::sync::RwLockReadGuard<'a, T>;

#[cfg(any())]
pub(crate) type RwLockReadGuard<'a, T> = core::cell::Ref<'a, T>;

pub(crate) type RwLockWriteGuard<'a, T> = std::sync::RwLockWriteGuard<'a, T>;

#[cfg(any())]
pub(crate) type RwLockWriteGuard<'a, T> = core::cell::RefMut<'a, T>;

pub(crate) trait MutexExt<T> {
    fn get(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn get(&self) -> MutexGuard<'_, T> {
        // PdfCraft patch: (#307) cached values remain owned after an unwinding caller; do not cascade a
        // worker panic into unrelated readers. Initialization has its own failure state.
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(any())]
impl<T> MutexExt<T> for Mutex<T> {
    fn get(&self) -> MutexGuard<'_, T> {
        self.borrow_mut()
    }
}

pub(crate) trait RwLockExt<T> {
    fn get(&self) -> RwLockReadGuard<'_, T>;
    fn put(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    // PdfCraft patch: (#307) poisoned locks are recovered rather than unwrapped, and the
    // non-blocking try_get/try_put (which repair asserted could not fail) become blocking.
    fn get(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn put(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(any())]
impl<T> RwLockExt<T> for RwLock<T> {
    fn get(&self) -> RwLockReadGuard<'_, T> {
        self.borrow()
    }

    fn put(&self) -> RwLockWriteGuard<'_, T> {
        self.borrow_mut()
    }
}
