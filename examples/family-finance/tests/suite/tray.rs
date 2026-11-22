//! The menu beside the clock (#643): the overdue bills at the top, the entries of the day,
//! which open the window on their form, and an update that is ready.

use family_finance::db::Database;
use family_finance::ui::{App, NEW_EXPENSE, TrayLink};
use family_finance::{e2e, seed, update};
use ravenrs::core::{TrayEntry, TrayMenu};
use ravenrs::prelude::*;
use ravenrs_e2e::E2e;

#[test]
fn the_menu_says_the_bills_and_opens_a_new_expense() {
    let db = Database::in_memory().unwrap();
    seed::load_demo(&db, e2e::day()).unwrap();
    let link = TrayLink::new(TrayMenu::default());
    let updater = update::idle();
    let app = App::new(db, e2e::day(), None, updater).tray(link.clone());
    let mut session = E2e::new(app, Theme::raven_light()).viewport(Size::new(1360.0, 860.0), 1.0);

    let entries = link.menu().entries();
    assert!(
        matches!(&entries[0], TrayEntry::Status { title, .. } if title.contains("conta") || title.contains("Contas")),
        "{entries:?}"
    );
    assert!(entries.contains(&TrayEntry::item(NEW_EXPENSE, "Nova despesa").shortcut("CmdOrCtrl+N")));
    assert!(NEW_EXPENSE.starts_with(ravenrs::core::OPENS_WINDOW), "it opens the window first");

    link.choose(NEW_EXPENSE);
    session.ui_mut().ui_waker().wake();
    session.advance(0.0);
    session.by_test_id("field:description").expect().to_be_visible().unwrap();
}
