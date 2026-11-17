//! What the update says on screen: the status bar and the news.

use ravenrs::movie::BugReporter;
use ravenrs::prelude::*;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use ravenrs_app_update::ui;

use crate::update::{self, Updater};

/// Connects the updater to the UI and takes in what its threads said. They only
/// wake the UI when there is something to read, so nothing runs while they work.
pub(crate) struct UpdatePump(pub Updater);

impl View for UpdatePump {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let waker = cx.ui_waker();
        self.0.connect(move || waker.wake());
        self.0.poll();
        cx.empty()
    }
}

/// Runs the paired Telegram bot beside this Raven instance and refreshes the UI on incoming messages.
pub(crate) struct TelegramPump {
    path: Option<PathBuf>,
    paired: Arc<AtomicBool>,
}

impl TelegramPump {
    pub(crate) fn new(path: Option<PathBuf>, paired: Arc<AtomicBool>) -> Self {
        Self { path, paired }
    }
}

impl View for TelegramPump {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        static STARTED: OnceLock<AtomicBool> = OnceLock::new();
        if let Some(path) = self.path.clone()
            && crate::telegram::token_configured()
            && !STARTED.get_or_init(AtomicBool::default).swap(true, Ordering::AcqRel)
        {
            let waker = cx.ui_waker();
            let paired = self.paired.clone();
            thread::spawn(move || {
                loop {
                    if let Err(error) = crate::telegram::serve_with_waker(
                        path.clone(),
                        Some(waker.clone()),
                        Some(paired.clone()),
                    ) {
                        tracing::warn!(error = %error, "Telegram listener stopped; retrying");
                        thread::sleep(Duration::from_secs(5));
                    }
                }
            });
        }
        if self.paired.swap(false, Ordering::AcqRel) {
            cx.sound().play(&SoundClip::click());
        }
        cx.empty()
    }
}

/// The news dialog, the same in every app that updates with `ravenrs-app-update`; here it
/// also opens the installer's log with the system.
pub(crate) use ravenrs_app_update::ui::NewsDialog;

/// A closed news dialog that opens the installer's log with the system.
pub(crate) fn news_dialog() -> NewsDialog {
    NewsDialog::new().on_open_file(|log| {
        if let Err(error) = ravenrs::native::open_with_system(log) {
            tracing::warn!(%error, "could not open the installer's log");
        }
    })
}

/// The line at the foot of the window: the version, always, and a notice in the
/// middle when there is one. Clicking the version shows what it brought.
pub(crate) fn status_bar(updater: &Updater, news: &NewsDialog, reporter: &BugReporter) -> impl View + use<> {
    let report = reporter.clone();
    let bar = StatusBar::new()
        .right((StatusBar::action("Relatar problema")
            .icon(IconName::CircleAlert)
            .on_click(move || report.open()),))
        .left((ui::version_button(format!("Raven Finanças · versão {}", update::CURRENT), updater, news),));
    ui::with_notice(bar, updater, news)
}
