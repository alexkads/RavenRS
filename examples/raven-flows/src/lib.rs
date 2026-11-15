//! Raven Flows: an automation app where flows of nodes are drawn on a `FlowCanvas`, run for
//! real by `ravenrs-flows` and kept by `ravenrs-flows-runtime`.
//!
//! It is made only of RavenRS components and is here to test them: it creates no component
//! and wraps none. What a screen needs that no component does is a gap of the library, with
//! an issue under #523, and stays out until the component has it.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`casa`] | The app's folder: flows as JSON files, runs in SQLite, the worker thread |
//! | [`marcas`] | A run drawn on the diagram with what the canvas already has |
//! | [`nos`] | The kinds of node in Portuguese, with their icons, and the toolbox |
//! | [`versao`] | Who the app is to the installer and the updater it shares with Raven Finanças |
//! | `ui` | The screens: flows, editor, runs and settings |

pub mod casa;
pub mod marcas;
pub mod nos;
mod ui;
pub mod versao;

use std::path::Path;
use std::rc::Rc;

use ravenrs::prelude::*;

pub use casa::{Casa, Worker};
pub use ui::{RavenFlows, apply_parameters, release_notes};

/// The app's version, the one its news speak of.
pub const VERSION: &str = versao::APP.version;

/// The app over the folder `dir`, its runs worked as `worker` says. The real app
/// ([`Worker::Thread`]) also looks for a new version when it opens and every hour after.
pub fn app_in(dir: impl AsRef<Path>, worker: Worker) -> Result<Localized<RavenFlows>, String> {
    let real = matches!(worker, Worker::Thread);
    let casa = Casa::open(dir, worker)?;
    let updater = versao::idle();
    if real {
        updater.watch();
    }
    Ok(Localized::new(Labels::pt_br(), RavenFlows::new(Rc::new(casa), updater)))
}
