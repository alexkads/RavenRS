//! Screens of real applications: what people actually look at.

use component_gallery::Gallery;
use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::Updater;
use ravenrs_core::Point;
use ravenrs_widgets::{AnyView, Date};

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
        scene("app-finance-dashboard", |interaction| finance(0, interaction)),
        scene("app-finance-entries", |interaction| finance(1, interaction)),
        scene("app-finance-reports", |interaction| finance(5, interaction)),
    ]
}
