//! Native entry point: `cargo run -p family-finance`.

use std::path::PathBuf;

use family_finance::themes;
use family_finance::ui::Opening;
use family_finance::update::{self, Updater};
use family_finance::{icon, paths, today};
use ravenrs::movie::RecentErrors;
use ravenrs::native::{AppIdentity, PlatformError, SystemTray, WindowOptions, diagnose, run_as};
use ravenrs::prelude::*;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

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
    let errors = RecentErrors::default();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,wgpu=warn,naga=warn".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(errors.clone())
        .init();
    // Another file is another app, as when testing beside the installed one.
    let everyday = first.is_none();
    let path = first.map_or_else(paths::database, PathBuf::from);
    if everyday {
        family_finance::reminders::watch(path.clone());
    }
    let options = WindowOptions::new("Raven Finanças").size(1360.0, 860.0).theme(themes::remembered());
    let diagnostics = Diagnostics::default();
    let identity = AppIdentity::new(paths::APP_ID)
        .labels(Labels::pt_br())
        .diagnostics(diagnostics.clone())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new");
    let identity = match icon() {
        Some(icon) => identity.icon(icon),
        None => identity,
    };
    let identity = if everyday {
        identity.close_to_tray(SystemTray::new("Raven Finanças").open("Abrir o Raven Finanças").quit("Sair"))
    } else {
        identity
    };
    run_as(Opening::new(path, today(), Updater::idle(), diagnostics, errors), options, identity)
}
