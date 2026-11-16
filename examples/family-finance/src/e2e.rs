//! The app on the test channel: what Raven Test Studio drives when it opens Raven Finanças.
//!
//! `raven-financas --e2e-serve` opens no window and no file. The session runs the whole app on
//! the sample family, in memory, on a fixed day, so a scenario sees the same screens and amounts
//! on every run and on every computer, and the database of the person using the app is never
//! opened or created.

use ravenrs::prelude::*;
use ravenrs_e2e::E2e;

use crate::db::Database;
use crate::seed;
use crate::ui::App;

/// The option that opens the app on the test channel.
pub const SERVE_FLAG: &str = ravenrs_e2e_wire::SERVE_FLAG;
/// The name the app gives on the channel, which a scenario's Open block uses.
pub const APP_NAME: &str = "Raven Finanças";

/// The day a session lives on, so due dates and the month's totals never move.
pub fn day() -> Date {
    Date::new(2026, 11, 15).unwrap_or_else(|| Date::from_days(0))
}

/// The app on the sample family in memory, at the size of its window.
pub fn session() -> Result<E2e, String> {
    let db = Database::in_memory().map_err(|e| e.to_string())?;
    seed::load_demo(&db, day()).map_err(|e| e.to_string())?;
    let app = App::new(db, day(), None, crate::update::idle());
    Ok(E2e::new(app, Theme::raven_light()).viewport(Size::new(1360.0, 860.0), 1.0))
}

/// Answers the tool that opened the app, over standard input and output, until it says bye.
pub fn serve() -> std::io::Result<()> {
    let session = session().map_err(std::io::Error::other)?;
    ravenrs_e2e_wire::serve(APP_NAME, session)
}
