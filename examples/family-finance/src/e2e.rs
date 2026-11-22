//! The app on the test channel: what Raven Test Studio drives when it opens Raven Finanças.
//!
//! `raven-financas --e2e-serve` opens no window and no file. The session runs the whole app on
//! the sample family, in memory, on a fixed day, so a scenario sees the same screens and amounts
//! on every run and on every computer, and the database of the person using the app is never
//! opened or created.
//!
//! `raven-financas --e2e-serve --e2e-window` (`--headed` in the tools) shows the real window on
//! the same sample family in memory, and the tool drives the interface the window shows: the
//! person watches the scenario run, and their own database is still never opened.
//!
//! The app brings its own scenarios, the `.raventest` files of `tests/e2e/`, embedded in
//! [`SCENARIOS`] and handed to the tool when it opens the app, so the installed app can be tested
//! without its source. `tests/suite/e2e.rs` runs each of them and checks the folder and the list
//! stay the same.

use ravenrs::prelude::*;
use ravenrs_e2e::E2e;

use crate::db::Database;
use crate::seed;
use crate::ui::App;

/// The option that opens the app on the test channel.
pub const SERVE_FLAG: &str = ravenrs_e2e_wire::SERVE_FLAG;
/// The name the app gives on the channel, which a scenario's Open block uses.
pub const APP_NAME: &str = "Raven Finanças";

/// The scenarios the app brings, one per file of `tests/e2e/`.
pub const SCENARIOS: [&str; 5] = [
    include_str!("../tests/e2e/lancar-despesa.raventest"),
    include_str!("../tests/e2e/lancar-receita.raventest"),
    include_str!("../tests/e2e/pagar-conta.raventest"),
    include_str!("../tests/e2e/trocar-de-mes.raventest"),
    include_str!("../tests/e2e/validar-lancamento.raventest"),
];

/// The day a session lives on, so due dates and the month's totals never move.
pub fn day() -> Date {
    Date::new(2026, 11, 15).unwrap_or_else(|| Date::from_days(0))
}

/// The size of the app's window, with or without it on screen.
const WINDOW: Size = Size::new(1360.0, 860.0);

/// The whole app on the sample family in memory, on [`day`].
fn app() -> Result<App, String> {
    let db = Database::in_memory().map_err(|e| e.to_string())?;
    seed::load_demo(&db, day()).map_err(|e| e.to_string())?;
    Ok(App::new(db, day(), None, crate::update::idle()))
}

/// The app on the sample family in memory, at the size of its window.
pub fn session() -> Result<E2e, String> {
    Ok(E2e::new(app()?, Theme::raven_light()).viewport(WINDOW, 1.0))
}

/// Whether the tool asked for the window, with [`ravenrs_e2e_wire::WINDOW_FLAG`] among `args`.
pub fn wants_window(args: &[String]) -> bool {
    args.iter().any(|a| a == ravenrs_e2e_wire::WINDOW_FLAG)
}

/// Shows the app's real window on the sample family in memory and answers the tool on the
/// interface it shows, until the tool says bye, releases the window and the person closes it, or
/// the person closes it during the test.
pub fn serve_headed() -> Result<(), String> {
    let hello = ravenrs_e2e_wire::Hello::of(APP_NAME).scenarios(SCENARIOS);
    let driver = ravenrs_e2e_wire::Headed::new(hello).filmed(ravenrs::movie::ChannelFilm::default());
    let options = ravenrs::native::WindowOptions::new(format!("{APP_NAME} — teste"))
        .size(WINDOW.width, WINDOW.height)
        .theme(Theme::raven_light());
    ravenrs::native::run_driven(app()?, options, driver).map_err(|e| e.to_string())
}

/// Answers the tool that opened the app, over standard input and output, until it says bye.
pub fn serve() -> std::io::Result<()> {
    let session = session().map_err(std::io::Error::other)?;
    // Filmed when the tool asks (`--film`): the film of a failure shows what led to it.
    let hello = ravenrs_e2e_wire::Hello::of(APP_NAME).scenarios(SCENARIOS);
    ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
}
