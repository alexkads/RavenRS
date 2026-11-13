//! [`Hotkey`]: a key combination that runs a command.

use std::rc::Rc;

use ravenrs_core::Key;

use crate::context::BuildCx;

/// A key combination that runs a command: a key, with Cmd (macOS) or Ctrl, and Shift.
///
/// ```
/// use ravenrs_widgets::Hotkey;
///
/// assert_eq!(Hotkey::command("s").shift().label(), Hotkey::command("S").shift().label());
/// assert!(Hotkey::key(ravenrs_core::Key::Delete).label().contains("Del"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Hotkey {
    key: Key,
    command: bool,
    #[doc(hidden)]
    pub shift: bool,
    alt: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Hotkey {
    /// This key alone.
    pub fn key(key: Key) -> Self {
        Self { key, command: false, shift: false, alt: false }
    }

    /// This letter or digit with Cmd or Ctrl.
    pub fn command(character: &str) -> Self {
        Self { key: Key::Character(character.to_lowercase()), command: true, shift: false, alt: false }
    }

    /// This letter or digit with Alt, or Option on a Mac.
    pub fn alt(character: &str) -> Self {
        Self { key: Key::Character(character.to_lowercase()), command: false, shift: false, alt: true }
    }

    #[doc(hidden)]
    pub fn of(key: Key, command: bool, shift: bool) -> Self {
        Self { key, command, shift, alt: false }
    }

    /// Needs Shift too.
    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// How it is written next to a command, such as `⌘S` or `Ctrl+S`.
    pub fn label(&self) -> String {
        let key = match &self.key {
            Key::Character(c) => c.to_uppercase(),
            Key::Delete => "Del".to_owned(),
            Key::Enter => "Enter".to_owned(),
            Key::Escape => "Esc".to_owned(),
            Key::Space => "Space".to_owned(),
            Key::Function(number) => format!("F{number}"),
            other => format!("{other:?}"),
        };
        if cfg!(target_os = "macos") {
            let alt = if self.alt { "⌥" } else { "" };
            let shift = if self.shift { "⇧" } else { "" };
            let command = if self.command { "⌘" } else { "" };
            format!("{alt}{shift}{command}{key}")
        } else {
            let alt = if self.alt { "Alt+" } else { "" };
            let shift = if self.shift { "Shift+" } else { "" };
            let command = if self.command { "Ctrl+" } else { "" };
            format!("{command}{alt}{shift}{key}")
        }
    }

    /// Makes it run `handler` for as long as the view building now is built.
    pub fn register(&self, cx: &mut BuildCx<'_>, handler: Rc<dyn Fn()>) {
        if self.alt {
            cx.shortcut_with_alt(self.key.clone(), handler);
        } else if self.shift {
            cx.shortcut_with_shift(self.key.clone(), self.command, handler);
        } else {
            cx.shortcut(self.key.clone(), self.command, handler);
        }
    }
}
