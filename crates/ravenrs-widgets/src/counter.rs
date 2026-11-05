//! What a shop counter needs: a bar of keys always in sight, and a field that takes a scanner.
//! Neither knows anything about shops: anything used from the keyboard all day can have them.

use std::fmt;
use std::rc::Rc;

use ravenrs_reactive::{Binding, State};

use crate::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, HStack, Hotkey, NodeId, TextField, View, ViewExt,
};

/// How long a scanner takes between characters at most, in seconds: much faster than typing.
const SCANNER_GAP: f64 = 0.05;

/// One key of a [`KeyBar`].
pub struct KeyBarItem {
    hotkey: Hotkey,
    label: String,
    primary: bool,
    disabled: bool,
    on_press: Rc<dyn Fn()>,
}

impl fmt::Debug for KeyBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyBarItem").field("label", &self.label).finish_non_exhaustive()
    }
}

impl KeyBarItem {
    /// A key that does this.
    pub fn new(hotkey: Hotkey, label: impl Into<String>, on_press: impl Fn() + 'static) -> Self {
        Self { hotkey, label: label.into(), primary: false, disabled: false, on_press: Rc::new(on_press) }
    }

    /// The main thing to do here.
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    /// Faded, and its key does nothing.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A row of keys and what they do, always in sight, as on a point of sale. They work wherever the
/// focus is, for as long as the bar is built.
#[derive(Debug, Default)]
pub struct KeyBar {
    items: Vec<KeyBarItem>,
}

impl KeyBar {
    /// A bar with no keys yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a key.
    pub fn item(mut self, item: KeyBarItem) -> Self {
        self.items.push(item);
        self
    }
}

impl View for KeyBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let mut row: Vec<AnyView> = Vec::with_capacity(self.items.len());
        for item in &self.items {
            if !item.disabled {
                item.hotkey.register(cx, item.on_press.clone());
            }
            let handler = item.on_press.clone();
            let style = if item.primary { ButtonStyle::Primary } else { ButtonStyle::Secondary };
            row.push(Box::new(
                Button::new(format!("{}  {}", item.hotkey.label(), item.label))
                    .size(ButtonSize::Large)
                    .style(style)
                    .disabled(item.disabled)
                    .full_width()
                    .on_click(move || handler())
                    .grow(if item.primary { 1.4 } else { 1.0 }),
            ));
        }
        cx.child(&HStack::new(row).spacing(8.0))
    }
}

/// What a [`ScanField`] took in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scan {
    /// The text.
    pub text: String,
    /// It came too fast for fingers: a scanner read it.
    pub from_reader: bool,
}

/// A field for what a scanner reads. A scanner is a keyboard that types fast and presses Enter, so
/// this keeps the focus whenever nothing else has it, tells the two apart and is empty again after each code.
pub struct ScanField {
    placeholder: String,
    disabled: bool,
    on_scan: Rc<dyn Fn(Scan)>,
}

impl fmt::Debug for ScanField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScanField").field("placeholder", &self.placeholder).finish_non_exhaustive()
    }
}

impl ScanField {
    /// A field that calls this with each code.
    pub fn new(on_scan: impl Fn(Scan) + 'static) -> Self {
        Self { placeholder: String::new(), disabled: false, on_scan: Rc::new(on_scan) }
    }

    /// What it says while empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Takes nothing in.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// When the characters of what is in the field came.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Pace {
    length: usize,
    first: f64,
    last: f64,
}

impl Pace {
    fn is_a_scanner(&self) -> bool {
        self.length >= 4 && (self.last - self.first) / (self.length - 1) as f64 <= SCANNER_GAP
    }
}

impl View for ScanField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let text: State<String> = cx.use_state(String::new);
        let pace: State<Pace> = cx.use_state(Pace::default);
        let now = cx.now();
        let length = text.peek().chars().count();
        // Reading the pace here, since a build follows each change of the text.
        let known = *pace.peek();
        if length != known.length {
            pace.set(match (length, known.length) {
                (0, _) => Pace::default(),
                (_, 0) => Pace { length, first: now, last: now },
                _ => Pace { length, last: now, ..known },
            });
        }
        let (read, write) = (text.clone(), text.clone());
        let binding = Binding::new(move || read.get(), move |value: String| write.set(value));
        let (clear, pace_now, on_scan) = (text.clone(), pace.clone(), self.on_scan.clone());
        let field = TextField::new(binding)
            .placeholder(self.placeholder.clone())
            .autofocus()
            .keeps_focus()
            .disabled(self.disabled)
            .on_submit(move |value: String| {
                let code = value.trim().to_owned();
                let from_reader = pace_now.peek().is_a_scanner();
                clear.set(String::new());
                pace_now.set(Pace::default());
                if !code.is_empty() {
                    on_scan(Scan { text: code, from_reader });
                }
            });
        cx.child(&field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pace_of_a_scanner_is_told_from_fingers() {
        let scanner = Pace { length: 13, first: 1.0, last: 1.2 };
        assert!(scanner.is_a_scanner());
        let fingers = Pace { length: 13, first: 1.0, last: 4.0 };
        assert!(!fingers.is_a_scanner());
        let too_short = Pace { length: 2, first: 1.0, last: 1.01 };
        assert!(!too_short.is_a_scanner());
    }
}
