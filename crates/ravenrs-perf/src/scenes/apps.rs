//! Screens of real applications: what people actually look at.

use component_gallery::Gallery;
use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::Updater;
use std::cell::RefCell;
use std::rc::Rc;

use feature_center::seed::PASSWORD;
use feature_center::{memory_database, watched};
use ravenrs_app_ui::Session;
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Point, Role};
use ravenrs_widgets::{AnyView, Date, Ui};

use crate::{Ceiling, Class, PerfScene, Setup, Step, ceiling, hover_buttons, nothing, resize, scroll_at};

const INTERACTIONS: &[&str] = &["hover", "scroll", "idle", "resize"];

fn step(interaction: &str) -> Step {
    match interaction {
        "hover" => hover_buttons(),
        "scroll" => scroll_at(Point::new(800.0, 400.0)),
        "resize" => resize(),
        _ => nothing(),
    }
}

fn finance(page: usize, interaction: &str) -> Setup {
    let Some(today) = Date::new(2026, 11, 15) else { panic!("a valid date") };
    let Ok(database) = Database::in_memory() else { panic!("a database") };
    if seed::load_demo(&database, today).is_err() {
        panic!("the demo data");
    }
    let app = App::new(database, today, None, Updater::idle());
    app.open_page(page);
    Setup { root: Box::new(app), step: step(interaction) }
}

/// The finance app and what moves it between its screens.
pub fn finance_tour() -> (AnyView, ravenrs_reactive::State<usize>) {
    let Some(today) = Date::new(2026, 11, 15) else { panic!("a valid date") };
    let Ok(database) = Database::in_memory() else { panic!("a database") };
    if seed::load_demo(&database, today).is_err() {
        panic!("the demo data");
    }
    let app = App::new(database, today, None, Updater::idle());
    let page = app.page_state();
    (Box::new(app), page)
}

/// The Feature Center and its session, once someone signs in.
pub fn feature_center() -> (AnyView, Rc<RefCell<Option<Session>>>) {
    let Ok(database) = memory_database() else { panic!("a database") };
    let session = Rc::new(RefCell::new(None));
    let keep = session.clone();
    let app = watched(move || database.space(), move |seen| *keep.borrow_mut() = Some(seen.clone()));
    let Ok(root) = app.build() else { panic!("the app puts itself together") };
    (Box::new(root), session)
}

fn act(ui: &mut Ui, role: Role, name: &str, action: AccessAction) {
    ui.update();
    let tree = ui.access_tree();
    let found = tree
        .nodes
        .iter()
        .find(|node| node.role == role && node.name.as_deref().is_some_and(|text| text.starts_with(name)));
    let Some(id) = found.map(|node| node.id) else { panic!("nothing called {name} on screen") };
    ui.perform_action(id, action);
    ui.update();
}

/// Signs in through the screen, as a person does.
pub fn sign_in(ui: &mut Ui) {
    for (field, text) in [("Nome", "ana"), ("Senha", PASSWORD)] {
        act(ui, Role::TextInput, field, AccessAction::Focus);
        for character in text.chars() {
            ui.handle_event(&Event::KeyDown {
                key: Key::Character(character.to_string()),
                modifiers: Modifiers::default(),
            });
        }
        ui.update();
    }
    act(ui, Role::Button, "Entrar", AccessAction::Click);
    ui.update();
}

/// Lists of the Feature Center that show the most, for the first and later visits.
pub const CENTER_LISTS: [&str; 8] = [
    "ManyRows",
    "EditorsDemo",
    "ListEditorsDemo",
    "VariantsDemo",
    "StateDemo",
    "RelationOwner",
    "AnalyticsDashboard",
    "SecurityAuditEntry",
];

fn center(list: Option<&'static str>, interaction: &str) -> Setup {
    let (root, session) = feature_center();
    let mut then = step(interaction);
    let mut ready = false;
    // The screen has to exist to be signed in to, so this waits for the first round.
    let step: Step = Box::new(move |ui, round| {
        if !ready {
            ready = true;
            sign_in(ui);
            if let (Some(list), Some(session)) = (list, session.borrow().as_ref()) {
                session.open_view(&format!("{list}_ListView"));
            }
            ui.update();
            return true;
        }
        then(ui, round)
    });
    Setup { root, step }
}

fn gallery(interaction: &str) -> Setup {
    Setup { root: Box::new(Gallery::new()), step: step(interaction) }
}

const CEILINGS: &[Ceiling] = &[ceiling("idle", "frames", 0), ceiling("hover", "shapes", 0)];

pub(super) fn scenes() -> Vec<PerfScene> {
    let scene = |name, make| PerfScene {
        name,
        covers: &[],
        class: Class::App,
        interactions: INTERACTIONS,
        make,
        ceilings: CEILINGS,
    };
    vec![
        scene("app-gallery", gallery),
        scene("app-center-start", |interaction| center(None, interaction)),
        scene("app-center-many-rows", |interaction| center(Some("ManyRows"), interaction)),
        scene("app-center-editors", |interaction| center(Some("EditorsDemo"), interaction)),
        scene("app-center-list-editors", |interaction| center(Some("ListEditorsDemo"), interaction)),
        scene("app-finance-dashboard", |interaction| finance(0, interaction)),
        scene("app-finance-entries", |interaction| finance(1, interaction)),
        scene("app-finance-reports", |interaction| finance(5, interaction)),
    ]
}
