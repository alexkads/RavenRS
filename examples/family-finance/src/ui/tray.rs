//! The menu beside the clock (#643): the bills that are overdue, the entries of the day and an
//! update that is ready, with the window closed.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs::core::{TrayEntry, TrayMenu};

/// "Nova despesa": opens the window and the form.
pub const NEW_EXPENSE: &str = "open:nova-despesa";
/// "Nova receita": opens the window and the form.
pub const NEW_INCOME: &str = "open:nova-receita";

/// What the window and the icon share: the menu the icon shows and the choices made on it,
/// which the window acts on in its next build.
#[derive(Clone, Debug, Default)]
pub struct TrayLink {
    menu: TrayMenu,
    chosen: Rc<RefCell<Vec<String>>>,
}

impl TrayLink {
    /// A link that sets `menu`; give the same menu to the `SystemTray`.
    pub fn new(menu: TrayMenu) -> Self {
        Self { menu, chosen: Rc::default() }
    }

    /// The menu the icon shows.
    pub fn menu(&self) -> &TrayMenu {
        &self.menu
    }

    /// Keeps a choice of the menu for the window; give it to `SystemTray::on_select`.
    pub fn choose(&self, id: &str) {
        self.chosen.borrow_mut().push(id.to_owned());
    }

    pub(crate) fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.chosen.borrow_mut())
    }
}

/// The menu for `overdue` bills and the update's entries.
pub(crate) fn entries(overdue: usize, update: Vec<TrayEntry>) -> Vec<TrayEntry> {
    let state = match overdue {
        0 => TrayEntry::status("Contas em dia", None),
        1 => TrayEntry::status("1 conta vencida", Some("Veja em Contas a pagar".to_owned())),
        n => TrayEntry::status(format!("{n} contas vencidas"), Some("Veja em Contas a pagar".to_owned())),
    };
    let mut out = vec![
        state,
        TrayEntry::item(NEW_EXPENSE, "Nova despesa").shortcut("CmdOrCtrl+N"),
        TrayEntry::item(NEW_INCOME, "Nova receita"),
    ];
    if !update.is_empty() {
        out.push(TrayEntry::Separator);
        out.extend(update);
    }
    out
}
