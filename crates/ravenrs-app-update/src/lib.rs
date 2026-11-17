//! Installing, updating and versioning a RavenRS app by compiling it on the
//! user's machine.
//!
//! The app reads `novidades.json` from the main branch. When that version is
//! newer than the running one, it says what changed and runs the installer,
//! which downloads the code and compiles it. Nothing that works gets touched:
//!
//! | Step | If it fails |
//! |---|---|
//! | download the installer to a file and check it is whole | nothing was touched |
//! | compile in a folder of its own | nothing was touched |
//! | run the new binary with `--versao` before swapping | nothing was touched |
//! | the installed app becomes `.anterior`, the new one moves in | the installer puts the old one back |
//! | the app checks the version of the installed binary | the status bar says so; the open app goes on |
//!
//! Releasing is one commit: the `version` in `Cargo.toml`, `novidades.json` and
//! the same entry on top of `historico-de-novidades.json`, with `contribuidores`
//! naming who made it and `issues` the issues it closes.
//! [`assert_release_files_agree`] holds the three together.
//!
//! The installer is saved to a file and never piped into a shell: a failed
//! `curl | sh` hands `sh` an empty script, which exits successfully having done nothing.
//!
//! The app looks when it opens and every hour after that, silently: [`notice`]
//! only speaks when there is a version, or when someone asked and is waiting for an answer.
//!
//! Who the app is, its folders and its addresses, is an [`InstalledApp`]; nothing
//! here knows any app. The README says how an app adopts it. With no features the
//! crate draws nothing; the feature `ui` adds [`ui`], the news and the notice drawn the same
//! way in every app.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

mod app;
mod installer;
mod news;
mod release;
mod template;
mod tray;
#[cfg(feature = "ui")]
pub mod ui;

pub use app::{InstallKind, InstalledApp};
pub use installer::{
    LOCK_EXPIRES, LOG_QUIET_FOR, Memory, installer_command, installer_is_whole, installer_stage, is_locked,
    lock_is_live, read_memory,
};
use installer::{Setup, download, run_installer, seconds_now};
pub use news::{
    News, Notice, NoticeAction, UpdateStatus, is_newer, join_history, notice, parse_history, parse_news,
};
pub use release::{assert_release_files_agree, check_release_files};
pub use template::{INSTALLER_TEMPLATE, WRITE_INSTALLER_ENV, assert_installer_is_current, check_installer};
pub use tray::{TRAY_NEWS, TRAY_UPDATE, TrayChoice, tray_choice, tray_entries};

/// How often an open app looks for a new version.
pub const CHECK_EVERY: Duration = Duration::from_secs(3600);

/// How long before a version that failed is tried again without being asked.
pub const RETRY_AFTER: Duration = Duration::from_secs(24 * 3600);

/// What to do on learning the latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Up to date, or no way of knowing.
    Nothing,
    /// There is a new version; `start` says whether compiling begins without a click.
    Announce {
        /// The new version.
        news: News,
        /// Compile now, at low priority.
        start: bool,
    },
}

/// The whole rule, with no network or disk. A version that failed recently, or
/// an installer already running, still shows the banner but does not start by itself.
pub fn decide(
    kind: InstallKind,
    current: &str,
    news: Option<News>,
    memory: &Memory,
    busy: bool,
    now: u64,
) -> Decision {
    let Some(news) = news.filter(|n| is_newer(&n.versao, current)) else {
        return Decision::Nothing;
    };
    let failed_recently = memory.failed.as_deref() == Some(news.versao.as_str())
        && memory.when.is_some_and(|when| now.saturating_sub(when) < RETRY_AFTER.as_secs());
    let start = kind == InstallKind::Installed && !busy && !failed_recently;
    Decision::Announce { news, start }
}

/// The hour has passed, and nothing is under way that a check would get in the way of.
pub fn check_is_due(since_last: Option<Duration>, status: &UpdateStatus) -> bool {
    let waiting =
        matches!(status, UpdateStatus::Compiling(_) | UpdateStatus::Ready(_) | UpdateStatus::Checking);
    !waiting && since_last.is_none_or(|elapsed| elapsed >= CHECK_EVERY)
}

/// Checks for updates and runs the installer on background threads. When one
/// finishes it wakes the UI, whose next build calls [`Updater::poll`].
#[derive(Clone, Debug)]
pub struct Updater {
    app: &'static InstalledApp,
    status: Rc<RefCell<UpdateStatus>>,
    inbox: Rc<RefCell<Option<Receiver<Message>>>>,
    waker: Rc<RefCell<Option<Rebuild>>>,
    /// A check asked for before the UI existed, which runs once it does.
    check_when_connected: Rc<Cell<bool>>,
    /// Looks again every hour. Only the real app does; tests look when told to.
    watching: Rc<Cell<bool>>,
    /// The thread that wakes the UI every hour is running.
    clock_started: Rc<Cell<bool>>,
    last_check: Rc<Cell<Option<Instant>>>,
    /// The step the installer is on.
    stage: Rc<RefCell<Option<String>>>,
    /// The version hidden for this session.
    dismissed: Rc<RefCell<Option<String>>>,
    /// What the repository says about versions this copy does not know.
    remote_history: Rc<RefCell<Vec<News>>>,
}

#[derive(Debug)]
enum Message {
    Checked {
        decision: Decision,
        /// Why the network gave no answer, if it did not.
        failure: Option<String>,
        /// The repository's history, read only when there is a new version to tell about.
        history: Vec<News>,
    },
    Stage(String),
    Finished(News, Result<(), String>),
}

impl Updater {
    /// An updater for `app` that does nothing until asked; tests use this one,
    /// and the app calls [`Self::watch`] on it once its window is open.
    pub fn idle(app: &'static InstalledApp) -> Self {
        Self {
            app,
            status: Rc::new(RefCell::new(UpdateStatus::Quiet)),
            inbox: Rc::new(RefCell::new(None)),
            waker: Rc::new(RefCell::new(None)),
            check_when_connected: Rc::new(Cell::new(false)),
            watching: Rc::new(Cell::new(false)),
            clock_started: Rc::new(Cell::new(false)),
            last_check: Rc::new(Cell::new(None)),
            stage: Rc::new(RefCell::new(None)),
            dismissed: Rc::new(RefCell::new(None)),
            remote_history: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Gives it what asks the screen to build again, from any thread (with RavenRS,
    /// `cx.ui_waker()` and its `wake`), and runs a check that was waiting for it.
    /// Only the first one given is kept.
    pub fn connect(&self, rebuild: impl Fn() + Send + Sync + 'static) {
        if self.waker.borrow().is_none() {
            *self.waker.borrow_mut() = Some(Rebuild(Arc::new(rebuild)));
        }
        if self.check_when_connected.take() {
            self.check();
        }
        if self.watching.get() && !self.clock_started.replace(true) {
            let waker = waker_for_clock(&self.waker);
            // Sleeps between wakes, so an idle app stays idle. [`Self::poll`] does the looking.
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(CHECK_EVERY);
                    waker.wake();
                }
            });
        }
    }

    /// Looks now and every hour from now on, silently unless there is a version.
    pub fn watch(&self) {
        self.watching.set(true);
        self.check();
    }

    /// Someone asked: says it is looking, and answers even when there is nothing new.
    /// Asking again brings back a version hidden with "Depois". An update under
    /// way already is the answer, so it stays.
    pub fn ask(&self) {
        if matches!(
            self.status(),
            UpdateStatus::Compiling(_) | UpdateStatus::Ready(_) | UpdateStatus::Checking
        ) {
            return;
        }
        *self.dismissed.borrow_mut() = None;
        self.set_status(UpdateStatus::Checking);
        self.check();
        self.refresh();
    }

    /// Rebuilds the UI after a click. The status is plain cells, which a click's
    /// handler changes with nobody else being told.
    fn refresh(&self) {
        if let Some(waker) = self.waker.borrow().as_ref() {
            waker.wake();
        }
    }

    /// The app it updates.
    pub fn app(&self) -> &'static InstalledApp {
        self.app
    }

    /// The step the installer was on, as it last said.
    pub fn stage(&self) -> Option<String> {
        self.stage.borrow().clone()
    }

    /// What the status bar says now.
    pub fn notice(&self) -> Option<Notice> {
        notice(
            &self.status(),
            self.app.version,
            self.stage.borrow().as_deref(),
            self.dismissed.borrow().as_deref(),
        )
    }

    /// Hides the notice. A version stays hidden for this session; an answer is just gone.
    pub fn dismiss(&self) {
        let status = self.status();
        match status.news() {
            Some(news) => *self.dismissed.borrow_mut() = Some(news.versao.clone()),
            None => self.set_status(UpdateStatus::Quiet),
        }
        self.refresh();
    }

    /// Every version known, newest first: the announced one, the repository's and this copy's.
    pub fn timeline(&self) -> Vec<News> {
        let announced: Vec<News> = self.status().news().cloned().into_iter().collect();
        join_history([announced, self.remote_history.borrow().clone(), self.app.history()])
    }

    /// Asks the network in the background.
    pub fn check(&self) {
        let Some(waker) = self.waker.borrow().clone() else {
            self.check_when_connected.set(true);
            return;
        };
        self.last_check.set(Some(Instant::now()));
        let (send, receive) = channel();
        *self.inbox.borrow_mut() = Some(receive);
        let app = self.app;
        std::thread::spawn(move || {
            let _wake_when_done = WakeOnDrop(waker);
            let (news, failure) = match latest_news(app) {
                Ok(news) => (Some(news), None),
                Err(reason) => (None, Some(reason)),
            };
            let home = app.home();
            let now = seconds_now();
            let decision = decide(
                app.install_kind(),
                app.version,
                news,
                &read_memory(&home.join(installer::MEMORY)),
                is_locked(&home.join(installer::LOCK), now),
                now,
            );
            let history = match decision {
                Decision::Nothing => Vec::new(),
                Decision::Announce { .. } => app
                    .history_urls
                    .iter()
                    .find_map(|url| {
                        download(url).ok().map(|text| parse_history(&text)).filter(|h| !h.is_empty())
                    })
                    .unwrap_or_default(),
            };
            // The receiver going away only means the window closed.
            let _ = send.send(Message::Checked { decision, failure, history });
        });
    }

    /// Where the update stands.
    pub fn status(&self) -> UpdateStatus {
        self.status.borrow().clone()
    }

    /// Replaces the status, for tests and previews.
    pub fn set_status(&self, status: UpdateStatus) {
        *self.status.borrow_mut() = status;
    }

    /// A background thread is working.
    pub fn is_running(&self) -> bool {
        self.inbox.borrow().is_some()
    }

    /// Takes in what the background thread said, and looks again when the hour
    /// has passed. Called while building, so it writes plain cells: the view that
    /// called it reads them right after.
    pub fn poll(&self) {
        self.receive();
        let since_last = self.last_check.get().map(|when| when.elapsed());
        if self.watching.get() && !self.is_running() && check_is_due(since_last, &self.status()) {
            self.check();
        }
    }

    fn receive(&self) {
        loop {
            let received = match self.inbox.borrow().as_ref() {
                Some(inbox) => inbox.try_recv(),
                None => return,
            };
            let message = match received {
                Ok(Message::Stage(stage)) => {
                    *self.stage.borrow_mut() = Some(stage);
                    continue;
                }
                Ok(message) => message,
                Err(TryRecvError::Empty) => return,
                // The thread died without answering. Waiting on would wake the UI forever.
                Err(TryRecvError::Disconnected) => {
                    *self.inbox.borrow_mut() = None;
                    match self.status() {
                        UpdateStatus::Compiling(news) => {
                            self.set_status(UpdateStatus::Failed(
                                news,
                                "a atualização parou no meio.".to_owned(),
                            ));
                        }
                        UpdateStatus::Checking => {
                            self.set_status(UpdateStatus::NoAnswer("a procura parou no meio.".to_owned()));
                        }
                        _ => {}
                    }
                    return;
                }
            };
            *self.inbox.borrow_mut() = None;
            self.take_in(message);
            return;
        }
    }

    fn take_in(&self, message: Message) {
        match message {
            Message::Checked { decision, failure, history } => {
                if !history.is_empty() {
                    *self.remote_history.borrow_mut() = history;
                }
                let status = self.status();
                match decision {
                    Decision::Nothing if status == UpdateStatus::Checking => self.set_status(match failure {
                        Some(reason) => UpdateStatus::NoAnswer(reason),
                        None => UpdateStatus::UpToDate,
                    }),
                    Decision::Nothing => {}
                    Decision::Announce { news, start } => {
                        // A failure on screen says more than the same version on offer again.
                        let failed_already = !start
                            && matches!(&status, UpdateStatus::Failed(failed, _) if failed.versao == news.versao);
                        if self.app.install_kind() == InstallKind::Development {
                            self.set_status(UpdateStatus::Development(news));
                        } else if start {
                            self.install(news);
                        } else if !failed_already {
                            self.set_status(UpdateStatus::Available(news));
                        }
                    }
                }
            }
            Message::Finished(news, Ok(())) => self.set_status(UpdateStatus::Ready(news)),
            Message::Finished(news, Err(reason)) => self.set_status(UpdateStatus::Failed(news, reason)),
            // [`Self::receive`] takes these before they get here.
            Message::Stage(_) => {}
        }
    }

    /// Downloads the installer and runs it in the background.
    pub fn install(&self, news: News) {
        self.set_status(UpdateStatus::Compiling(news.clone()));
        *self.stage.borrow_mut() = None;
        self.refresh();
        let (send, receive) = channel();
        *self.inbox.borrow_mut() = Some(receive);
        let waker = self.waker.borrow().clone().unwrap_or_default();
        let app = self.app;
        std::thread::spawn(move || {
            // Dropping this wakes the UI however the thread ends, even in a panic,
            // so a thread that died cannot leave it waiting.
            let _wake_when_done = WakeOnDrop(waker.clone());
            let stages = send.clone();
            let result = run_installer(&Setup::of(app), &news.versao, &move |stage| {
                let _ = stages.send(Message::Stage(stage));
                waker.wake();
            });
            let _ = send.send(Message::Finished(news, result));
        });
    }

    /// Opens the installed version and closes this one.
    pub fn reopen(&self) {
        match self.app.installed_binary().map(|bin| std::process::Command::new(bin).spawn()) {
            Some(Ok(_)) => std::process::exit(0),
            Some(Err(e)) => tracing::warn!(error = %e, "could not open the new version"),
            None => tracing::warn!("no installed binary on this system"),
        }
    }
}

fn waker_for_clock(waker: &Rc<RefCell<Option<Rebuild>>>) -> Rebuild {
    waker.borrow().clone().unwrap_or_default()
}

/// What asks the screen to build again; by default, nothing.
#[derive(Clone)]
struct Rebuild(Arc<dyn Fn() + Send + Sync>);

impl Rebuild {
    fn wake(&self) {
        (self.0)();
    }
}

impl Default for Rebuild {
    fn default() -> Self {
        Self(Arc::new(|| {}))
    }
}

impl std::fmt::Debug for Rebuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Rebuild")
    }
}

struct WakeOnDrop(Rebuild);

impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        self.0.wake();
    }
}

/// Asks now, on this thread, what the window's check asks in the background: the latest
/// version the repository announces and what to do about it, by the same rule ([`decide`]).
/// For a command line, or another program that asks the app (a pane beside a chat), with no
/// window and no [`Updater`]: the news read, which names the latest version even when it is
/// the one running, and the decision. `Err` says why the network gave no answer.
pub fn check_now(app: &InstalledApp) -> Result<(News, Decision), String> {
    let news = latest_news(app)?;
    let home = app.home();
    let now = seconds_now();
    let decision = decide(
        app.install_kind(),
        app.version,
        Some(news.clone()),
        &read_memory(&home.join(installer::MEMORY)),
        is_locked(&home.join(installer::LOCK), now),
        now,
    );
    Ok((news, decision))
}

/// Installs `news` now, on this thread, with the installer the window runs: the same lock,
/// the same memory of a failed version and the same check that the installed copy answers
/// with the new version. `on_stage` hears each stage the installer reports, to show it.
pub fn install_now(app: &InstalledApp, news: &News, on_stage: &dyn Fn(String)) -> Result<(), String> {
    run_installer(&Setup::of(app), &news.versao, on_stage)
}

/// The latest version the repository announces, or why it could not be known.
fn latest_news(app: &InstalledApp) -> Result<News, String> {
    let mut reason = "a resposta não veio completa.".to_owned();
    for url in app.news_urls {
        match download(url) {
            Ok(text) => {
                if let Some(news) = parse_news(&text) {
                    return Ok(news);
                }
            }
            Err(error) => reason = error,
        }
    }
    Err(reason)
}

#[cfg(test)]
mod tests;
