use std::fmt;
use std::rc::Rc;

/// A read/write handle to a value owned elsewhere (SwiftUI's `@Binding`).
///
/// Components that edit a value (a toggle, a text field) take a `Binding` so the
/// caller decides where the value lives. Reads are tracked like the underlying
/// state's reads.
pub struct Binding<T> {
    get: Rc<dyn Fn() -> T>,
    set: Rc<dyn Fn(T)>,
}

impl<T: 'static> Binding<T> {
    /// Creates a binding from a getter and a setter.
    pub fn new(get: impl Fn() -> T + 'static, set: impl Fn(T) + 'static) -> Self {
        Self { get: Rc::new(get), set: Rc::new(set) }
    }

    /// A binding that always reads `value` and ignores writes; useful in previews and tests.
    pub fn constant(value: T) -> Self
    where
        T: Clone,
    {
        Self::new(move || value.clone(), |_| {})
    }

    /// Reads the current value through the getter.
    pub fn get(&self) -> T {
        (self.get)()
    }

    /// Writes `value` through the setter.
    pub fn set(&self, value: T) {
        (self.set)(value)
    }

    /// Reads the value, lets `f` modify it, and writes it back.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let mut v = self.get();
        f(&mut v);
        self.set(v);
    }

    /// Projects a binding onto a part of the value, e.g. one field of a struct.
    pub fn map<U: 'static>(
        &self,
        get: impl Fn(&T) -> U + 'static,
        set: impl Fn(&mut T, U) + 'static,
    ) -> Binding<U> {
        let (outer_get, outer_set) = (self.get.clone(), self.set.clone());
        let read = self.get.clone();
        Binding::new(
            move || get(&read()),
            move |u| {
                let mut whole = outer_get();
                set(&mut whole, u);
                outer_set(whole);
            },
        )
    }
}

impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Self { get: self.get.clone(), set: self.set.clone() }
    }
}

impl<T> fmt::Debug for Binding<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Binding")
    }
}

#[cfg(test)]
mod tests {
    use crate::State;

    #[derive(Clone, Debug, PartialEq)]
    struct Settings {
        volume: u8,
        muted: bool,
    }

    #[test]
    fn binding_writes_through_to_state() {
        let s = State::new(1);
        let b = s.binding();
        b.update(|v| *v += 41);
        assert_eq!(s.get(), 42);
    }

    #[test]
    fn mapped_binding_edits_one_field() {
        let s = State::new(Settings { volume: 3, muted: false });
        let muted = s.binding().map(|s| s.muted, |s, m| s.muted = m);
        muted.set(true);
        assert_eq!(s.get(), Settings { volume: 3, muted: true });
    }
}
