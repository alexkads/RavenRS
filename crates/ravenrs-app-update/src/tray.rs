//! The update in the menu of the icon beside the clock (#643): with the window closed, the
//! status bar is not seen, so the menu says that a version is ready and installs it.

use ravenrs_core::TrayEntry;

use crate::{News, UpdateStatus, Updater};

/// The id of the entry that installs, retries or reopens on the new version.
pub const TRAY_UPDATE: &str = "ravenrs:update";
/// The id of the entry that shows the news of the version; it starts with
/// [`ravenrs_core::OPENS_WINDOW`], so the window opens first.
pub const TRAY_NEWS: &str = "open:ravenrs:news";

/// What choosing an entry of the update does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayChoice {
    /// Download and compile this version in the background.
    Install(News),
    /// Open the version already installed and close this one.
    Reopen,
    /// Show the news of the version, in the window.
    News,
}

/// The entries the menu shows for `status`, with the installer's `stage` while it compiles.
pub fn tray_entries(status: &UpdateStatus, stage: Option<&str>) -> Vec<TrayEntry> {
    let news = |n: &News| TrayEntry::item(TRAY_NEWS, format!("Novidades da versão {}", n.versao));
    match status {
        UpdateStatus::Available(n) => {
            vec![TrayEntry::item(TRAY_UPDATE, format!("Atualizar para a versão {}", n.versao)), news(n)]
        }
        UpdateStatus::Compiling(n) => vec![
            TrayEntry::status(format!("Atualizando para a versão {}…", n.versao), stage.map(str::to_owned)),
            news(n),
        ],
        UpdateStatus::Ready(n) => vec![
            TrayEntry::item(TRAY_UPDATE, "Reiniciar para atualizar")
                .detail(format!("Versão {} pronta", n.versao)),
            news(n),
        ],
        UpdateStatus::Failed(n, _) => vec![
            TrayEntry::item(TRAY_UPDATE, "Tentar atualizar de novo")
                .detail(format!("A versão {} não instalou", n.versao)),
            news(n),
        ],
        _ => Vec::new(),
    }
}

/// What choosing `id` does with the update as it stands; `None` for an id that is not the
/// update's, or for one that has nothing to do now.
pub fn tray_choice(status: &UpdateStatus, id: &str) -> Option<TrayChoice> {
    match (id, status) {
        (TRAY_NEWS, s) if s.news().is_some() => Some(TrayChoice::News),
        (TRAY_UPDATE, UpdateStatus::Available(n) | UpdateStatus::Failed(n, _)) => {
            Some(TrayChoice::Install(n.clone()))
        }
        (TRAY_UPDATE, UpdateStatus::Ready(_)) => Some(TrayChoice::Reopen),
        _ => None,
    }
}

impl Updater {
    /// The entries for the menu beside the clock: the version to install, the one installing
    /// with its step, the one ready ("Reiniciar para atualizar / Versão X pronta"), and its
    /// news. Empty when there is nothing to say. Put them in the app's `TrayMenu` on each build.
    pub fn tray_entries(&self) -> Vec<TrayEntry> {
        tray_entries(&self.status(), self.stage().as_deref())
    }

    /// Does what an entry of [`Self::tray_entries`] says, as the status bar's buttons do, and
    /// says whether `id` was one of them. [`TRAY_NEWS`] only opens the window (its id starts
    /// with `open:`) and returns [`TrayChoice::News`], for the app to show the news there.
    pub fn tray_select(&self, id: &str) -> Option<TrayChoice> {
        let choice = tray_choice(&self.status(), id)?;
        match &choice {
            TrayChoice::Install(news) => self.install(news.clone()),
            TrayChoice::Reopen => self.reopen(),
            TrayChoice::News => {}
        }
        Some(choice)
    }
}
