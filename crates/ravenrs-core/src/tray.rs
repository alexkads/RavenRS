//! What the menu of the icon beside the clock shows: a plain value that crosses threads, so the
//! screen, a background thread (an update that finished) and the platform all read the same.

use std::sync::{Arc, Mutex, PoisonError};

/// An id that starts with this opens the app's window before the choice reaches the app, for an
/// entry whose answer is on screen, such as "Novo lançamento" or the news of an update.
pub const OPENS_WINDOW: &str = "open:";

/// One entry of a [`TrayMenu`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum TrayEntry {
    /// The app's state at the top of the menu: a line that cannot be chosen, and a second,
    /// quieter one when there is a detail.
    Status {
        /// What it says.
        title: String,
        /// A second line, such as the last time it ran.
        detail: Option<String>,
    },
    /// Something to do. The app hears its `id` when it is chosen.
    Item {
        /// What the app hears; one that starts with [`OPENS_WINDOW`] opens the window first.
        id: String,
        /// What it says.
        label: String,
        /// A line below it that cannot be chosen, such as "Versão 0.1.8 pronta".
        detail: Option<String>,
        /// Whether it can be chosen.
        enabled: bool,
        /// A key combination shown beside it, such as `CmdOrCtrl+N`, where the system shows one.
        shortcut: Option<String>,
    },
    /// A choice that is on or off, with a check mark.
    Check {
        /// What the app hears.
        id: String,
        /// What it says.
        label: String,
        /// Whether it is on.
        checked: bool,
    },
    /// Entries one level down, behind an arrow.
    Submenu {
        /// What it says.
        label: String,
        /// What it holds.
        entries: Vec<TrayEntry>,
    },
    /// A line between groups.
    Separator,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TrayEntry {
    /// The state at the top, with an optional second line.
    pub fn status(title: impl Into<String>, detail: Option<String>) -> Self {
        Self::Status { title: title.into(), detail }
    }

    /// Something to do, enabled, with no detail or shortcut.
    pub fn item(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Item { id: id.into(), label: label.into(), detail: None, enabled: true, shortcut: None }
    }

    /// A choice that is on or off.
    pub fn check(id: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self::Check { id: id.into(), label: label.into(), checked }
    }

    /// Entries one level down.
    pub fn submenu(label: impl Into<String>, entries: Vec<TrayEntry>) -> Self {
        Self::Submenu { label: label.into(), entries }
    }

    /// The same item, enabled or not; other entries are left as they are.
    pub fn enabled(mut self, on: bool) -> Self {
        if let Self::Item { enabled, .. } = &mut self {
            *enabled = on;
        }
        self
    }

    /// The same item, with a line below it.
    pub fn detail(mut self, text: impl Into<String>) -> Self {
        if let Self::Item { detail, .. } = &mut self {
            *detail = Some(text.into());
        }
        self
    }

    /// The same item, with a key combination beside it, such as `CmdOrCtrl+N`.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        if let Self::Item { shortcut, .. } = &mut self {
            *shortcut = Some(keys.into());
        }
        self
    }
}

type Changed = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Shared {
    entries: Mutex<Vec<TrayEntry>>,
    changed: Mutex<Option<Changed>>,
}

/// The app's own entries of the menu beside the clock, between "Abrir" and "Sair", which stay
/// first and last. Clones share the same menu: the app keeps one and sets it whenever its state
/// changes, from any thread, and the icon shows the new menu at once.
///
/// ```
/// use ravenrs_core::{TrayEntry, TrayMenu};
///
/// let menu = TrayMenu::new(vec![TrayEntry::status("Autoteste: 18 de 18", None)]);
/// menu.set(vec![TrayEntry::status("Rodando…", None), TrayEntry::item("run", "Rodar autoteste")]);
/// assert_eq!(menu.entries().len(), 2);
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TrayMenu(Arc<Shared>);

impl std::fmt::Debug for TrayMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TrayMenu").field(&self.entries()).finish()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TrayMenu {
    /// A menu with these entries.
    pub fn new(entries: Vec<TrayEntry>) -> Self {
        let menu = Self::default();
        *menu.0.entries.lock().unwrap_or_else(PoisonError::into_inner) = entries;
        menu
    }

    /// The entries as they are now.
    pub fn entries(&self) -> Vec<TrayEntry> {
        self.0.entries.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Replaces the entries. The icon hears it only when they differ, so a screen may set
    /// them on every build.
    pub fn set(&self, entries: Vec<TrayEntry>) {
        {
            let mut now = self.0.entries.lock().unwrap_or_else(PoisonError::into_inner);
            if *now == entries {
                return;
            }
            *now = entries;
        }
        let changed = self.0.changed.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(changed) = changed {
            changed();
        }
    }

    /// What runs when the entries change: the platform redraws the menu. Only the last one
    /// given is kept.
    pub fn on_change(&self, changed: impl Fn() + Send + Sync + 'static) {
        *self.0.changed.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(changed));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn the_icon_hears_only_a_menu_that_changed() {
        let menu = TrayMenu::new(vec![TrayEntry::Separator]);
        let heard = Arc::new(AtomicUsize::new(0));
        let h = heard.clone();
        menu.on_change(move || {
            h.fetch_add(1, Ordering::SeqCst);
        });
        menu.set(vec![TrayEntry::Separator]);
        assert_eq!(heard.load(Ordering::SeqCst), 0, "the same entries say nothing");
        let other = menu.clone();
        std::thread::spawn(move || other.set(vec![TrayEntry::item("a", "A").enabled(false)])).join().ok();
        assert_eq!(heard.load(Ordering::SeqCst), 1);
        assert_eq!(menu.entries(), [TrayEntry::item("a", "A").enabled(false)]);
    }

    #[test]
    fn the_builders_touch_only_items() {
        let item = TrayEntry::item("x", "X").detail("d").shortcut("CmdOrCtrl+N").enabled(false);
        assert_eq!(
            item,
            TrayEntry::Item {
                id: "x".into(),
                label: "X".into(),
                detail: Some("d".into()),
                enabled: false,
                shortcut: Some("CmdOrCtrl+N".into())
            }
        );
        assert_eq!(TrayEntry::Separator.enabled(false).detail("d"), TrayEntry::Separator);
    }
}
