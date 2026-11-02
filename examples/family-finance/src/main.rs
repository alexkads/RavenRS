//! Native entry point: `cargo run -p family-finance`.

use std::path::{Path, PathBuf};

use family_finance::db::{Database, DbResult};
use family_finance::ui::App;
use family_finance::update::{self, Updater};
use family_finance::{icon, paths, seed, today};
use ravenrs::native::{AppIdentity, PlatformError, WindowOptions, diagnose, run, run_as};
use ravenrs::prelude::*;

fn open(path: &Path) -> DbResult<Database> {
    let db = Database::open(path)?;
    // The first run opens on a family's sample data, so there is something to
    // explore; Settings starts over for real use.
    if db.is_empty()? {
        seed::load_demo(&db, today())?;
    }
    Ok(db)
}

fn main() -> Result<(), PlatformError> {
    let mut arguments = std::env::args().skip(1);
    let first = arguments.next();
    // The installer proves a new binary opens with this, before any window or database.
    if first.as_deref() == Some("--versao") {
        println!("{}", update::CURRENT);
        return Ok(());
    }
    // Sem janela nem banco, para responder mesmo quando a app não abre.
    if first.as_deref() == Some("--diagnostico") {
        print!("{}", diagnose());
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .init();
    let path = first.map_or_else(paths::database, PathBuf::from);
    let options = WindowOptions::new("Raven Finanças").size(1360.0, 860.0).theme(Theme::raven_light());
    match open(&path) {
        Ok(db) => {
            let copies = family_finance::backup::copies_folder(&path);
            if let Err(e) = family_finance::backup::keep_daily_copy(&db, &copies, today()) {
                tracing::warn!(error = %e, "could not keep the copy of the day");
            }
            let updater = Updater::idle();
            updater.watch();
            let app = App::new(db, today(), Some(path), updater);
            let identity = AppIdentity::new(paths::APP_ID)
                .labels(Labels::pt_br())
                .diagnostics(app.diagnostics())
                .new_issue_url("https://github.com/alexkads/RavenRS/issues/new");
            let identity = match icon() {
                Some(icon) => identity.icon(icon),
                None => identity,
            };
            run_as(app, options, identity)
        }
        Err(e) => {
            tracing::error!(error = %e, path = %path.display(), "could not open the database");
            run(
                VStack::new((
                    Alert::new("Não foi possível abrir o banco de dados", e.to_string()).tone(Tone::Danger),
                ))
                .padding(32.0),
                options.size(640.0, 200.0),
            )
        }
    }
}
