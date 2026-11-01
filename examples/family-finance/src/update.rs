//! Updating by compiling, the way VintageLightbox does it.
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
//! the same entry on top of `historico-de-novidades.json`. Tests hold the three together.
//!
//! The installer is saved to a file and never piped into a shell: a failed
//! `curl | sh` hands `sh` an empty script, which exits successfully having done nothing.
//!
//! The app looks when it opens and every hour after that, silently: [`notice`]
//! only speaks when there is a version, or when someone asked and is waiting for an answer.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use ravenrs::prelude::UiWaker;
use serde::{Deserialize, Serialize};

use crate::paths;

/// The running version.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Where the app reads what is new, in order: `raw` follows the branch at once
/// and Pages is the copy. An installed app only knows the addresses it was
/// compiled with, so addresses are only ever added.
pub const NEWS_URLS: [&str; 2] = [
    "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/novidades.json",
    "https://alexkads.github.io/RavenRS/family-finance/novidades.json",
];

/// Where the versions after this one are told, for skipping more than one at a time.
pub const HISTORY_URLS: [&str; 2] = [
    "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/historico-de-novidades.json",
    "https://alexkads.github.io/RavenRS/family-finance/historico-de-novidades.json",
];

/// The installer the app downloads and runs, the same one the README shows.
pub const INSTALLER_URL: &str =
    "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd";

/// Running the installer by hand, for when updating from inside the app fails.
pub const INSTALL_COMMAND: &str = if cfg!(target_os = "windows") {
    "irm https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd -OutFile \"$env:TEMP\\install.cmd\"; & \"$env:TEMP\\install.cmd\""
} else {
    "curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd"
};

/// How often an open app looks for a new version.
pub const CHECK_EVERY: Duration = Duration::from_secs(3600);

/// How often the installer's log is read for the step it is on.
const STAGE_EVERY: Duration = Duration::from_secs(1);

/// How long before a version that failed is tried again without being asked.
pub const RETRY_AFTER: Duration = Duration::from_secs(24 * 3600);
/// A lock older than this belongs to an installer that died without removing it.
pub const LOCK_EXPIRES: Duration = Duration::from_secs(3 * 3600);

/// What a version brings, written for the people using the app.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct News {
    /// The version.
    pub versao: String,
    /// The release day; only the history has it.
    #[serde(default)]
    pub data: Option<String>,
    /// One line: what people will notice first.
    pub titulo: String,
    /// An update that should not wait.
    #[serde(default)]
    pub importante: bool,
    /// What changed.
    pub novidades: Vec<String>,
    /// Why it is worth updating now.
    pub por_que_atualizar: String,
}

/// Reads `novidades.json`. A broken one is no news: better silent than a blank announcement.
pub fn parse_news(text: &str) -> Option<News> {
    let news: News = serde_json::from_str(text).ok()?;
    (version_numbers(&news.versao).is_some() && !news.titulo.trim().is_empty()).then_some(news)
}

/// Reads a history file, skipping an entry it cannot understand.
pub fn parse_history(text: &str) -> Vec<News> {
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap_or_default();
    items.into_iter().filter_map(|item| serde_json::from_value(item).ok()).collect()
}

/// Every version up to this one, newest first, compiled into the app.
pub fn history() -> Vec<News> {
    parse_history(include_str!("../historico-de-novidades.json"))
}

/// One list of versions, newest first, out of several. When a version is in
/// more than one, the first list's account of it is the one that stays.
pub fn join_history(lists: impl IntoIterator<Item = Vec<News>>) -> Vec<News> {
    let mut all: Vec<News> = Vec::new();
    for news in lists.into_iter().flatten() {
        if !all.iter().any(|known| known.versao == news.versao) {
            all.push(news);
        }
    }
    all.sort_by_key(|news| std::cmp::Reverse(version_numbers(&news.versao)));
    all
}

fn version_numbers(version: &str) -> Option<Vec<u64>> {
    version.trim().split('.').map(|part| part.parse().ok()).collect()
}

/// `candidate` is a later version than `current`, comparing numbers (0.1.10 is after 0.1.9).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (version_numbers(candidate), version_numbers(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// How this copy of the app was installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// By the installer, which is the only thing that can update it.
    Installed,
    /// `cargo run`: whoever develops updates with `git pull`.
    Development,
}

impl InstallKind {
    /// The installer sets `FAMILY_FINANCE_INSTALLED` when it compiles.
    pub fn current() -> Self {
        if option_env!("FAMILY_FINANCE_INSTALLED").is_some() { Self::Installed } else { Self::Development }
    }
}

/// Remembers an update that failed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// The version that failed last.
    pub failed: Option<String>,
    /// When, in seconds since 1970.
    pub when: Option<u64>,
}

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

/// The step the installer announced on a line of its log: `> text`.
pub fn installer_stage(line: &str) -> Option<String> {
    let text = line.trim().strip_prefix("> ")?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The last step announced in the log so far.
fn last_stage(log: &str) -> Option<String> {
    log.lines().rev().find_map(installer_stage)
}

/// The downloaded file really is the whole installer. An error page, an empty
/// file or one cut at any point does not pass, because the end of the file has to be there.
pub fn installer_is_whole(text: &str) -> bool {
    let end = text.trim_end();
    let last_line = end.strip_suffix('}').map(str::trim_end).and_then(|before| before.lines().last());
    text.starts_with(":<<\"::FIM-DO-CMD\"")
        && text.contains("\n#==POWERSHELL==")
        && text.contains("\n#==FIM-POWERSHELL==")
        && last_line.is_some_and(|line| line.contains("Fim do instalador"))
}

/// How to run the installer here: at low priority on Linux and macOS, so
/// compiling does not compete with whoever is using the machine.
pub fn installer_command(file: &Path) -> (String, Vec<String>) {
    let path = file.display().to_string();
    if cfg!(windows) {
        ("cmd".to_owned(), vec!["/c".to_owned(), path])
    } else {
        ("nice".to_owned(), vec!["-n".to_owned(), "10".to_owned(), "sh".to_owned(), path])
    }
}

/// An installer is running. The lock holds the time it began.
pub fn is_locked(lock: &Path, now: u64) -> bool {
    std::fs::read_to_string(lock)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .is_some_and(|began| now.saturating_sub(began) < LOCK_EXPIRES.as_secs())
}

fn seconds_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn read_memory(file: &Path) -> Memory {
    std::fs::read(file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn write_memory(file: &Path, memory: &Memory) {
    if let Ok(text) = serde_json::to_vec_pretty(memory)
        && let Err(e) = std::fs::write(file, text)
    {
        tracing::warn!(error = %e, "could not remember the failed update");
    }
}

/// Where an update stands, for the status bar and the settings page.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    /// Nothing to say yet, or up to date.
    #[default]
    Quiet,
    /// A new version exists and is waiting for a click.
    Available(News),
    /// Compiling it now.
    Compiling(News),
    /// It is installed; reopening uses it.
    Ready(News),
    /// It failed, and the app that is open keeps working.
    Failed(News, String),
    /// A new version exists, but this copy updates with `git pull`.
    Development(News),
    /// Someone asked, and the answer has not come yet.
    Checking,
    /// Someone asked, and there is nothing newer.
    UpToDate,
    /// Someone asked, and the network did not answer.
    NoAnswer(String),
}

impl UpdateStatus {
    /// The version it is about, if any.
    pub fn news(&self) -> Option<&News> {
        match self {
            Self::Available(news)
            | Self::Compiling(news)
            | Self::Ready(news)
            | Self::Failed(news, _)
            | Self::Development(news) => Some(news),
            Self::Quiet | Self::Checking | Self::UpToDate | Self::NoAnswer(_) => None,
        }
    }
}

/// What a notice in the status bar offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeAction {
    /// Opens the news on the announced version.
    News,
    /// Installs the announced version.
    Update,
    /// Installs it again, after a failure.
    Retry,
    /// The news again, where the steps for doing it by hand are.
    HowToUpdate,
    /// Opens the installed version.
    Reopen,
    /// Asks the network again.
    CheckAgain,
    /// Hides the notice for this session.
    Later,
    /// Hides a notice that has been read.
    Close,
}

impl NoticeAction {
    /// The button's words.
    pub fn label(self) -> &'static str {
        match self {
            Self::News => "Ver novidades",
            Self::Update => "Atualizar",
            Self::Retry | Self::CheckAgain => "Tentar de novo",
            Self::HowToUpdate => "Como atualizar",
            Self::Reopen => "Reabrir agora",
            Self::Later => "Depois",
            Self::Close => "Fechar",
        }
    }

    /// The button that moves things forward, drawn as the main one.
    pub fn is_primary(self) -> bool {
        matches!(self, Self::Update | Self::Reopen | Self::Retry | Self::CheckAgain)
    }
}

/// What the status bar says about updates, in one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The sentence.
    pub text: String,
    /// Something is under way.
    pub busy: bool,
    /// Tint this one: an important version that is waiting for a click.
    pub important: bool,
    /// Its buttons, in order.
    pub actions: Vec<NoticeAction>,
}

/// The whole rule of what the status bar says, with nothing on screen, as
/// VintageLightbox words it. `dismissed` is the version hidden for this session;
/// what is under way and what waits for reopening cannot be hidden.
pub fn notice(
    status: &UpdateStatus,
    current: &str,
    stage: Option<&str>,
    dismissed: Option<&str>,
) -> Option<Notice> {
    let said = |text: String, busy, important, actions: &[NoticeAction]| {
        Some(Notice { text, busy, important, actions: actions.to_vec() })
    };
    use NoticeAction::{CheckAgain, Close, HowToUpdate, Later, News, Reopen, Retry, Update};
    match status {
        UpdateStatus::Quiet => None,
        UpdateStatus::Checking => said("Procurando versão nova…".to_owned(), true, false, &[]),
        UpdateStatus::UpToDate => {
            said(format!("Você está na versão mais recente ({current})."), false, false, &[Close])
        }
        UpdateStatus::NoAnswer(reason) => said(
            format!("Não consegui verificar se há versão nova: {reason}"),
            false,
            false,
            &[CheckAgain, Close],
        ),
        UpdateStatus::Available(news) | UpdateStatus::Development(news) | UpdateStatus::Failed(news, _)
            if dismissed == Some(news.versao.as_str()) =>
        {
            None
        }
        UpdateStatus::Available(news) => {
            let opening = if news.importante {
                format!("Atualização importante: versão {}", news.versao)
            } else {
                format!("Versão {} disponível", news.versao)
            };
            said(
                format!("{opening} — {}", news.titulo.trim()),
                false,
                news.importante,
                &[News, Update, Later],
            )
        }
        UpdateStatus::Development(news) => said(
            format!(
                "Versão {} no repositório. Esta cópia foi compilada à mão: atualize com git pull.",
                news.versao
            ),
            false,
            false,
            &[News, Later],
        ),
        UpdateStatus::Compiling(news) => said(
            format!(
                "Atualizando para a versão {} em segundo plano — {}. O app continua funcionando.",
                news.versao,
                stage.unwrap_or("começando"),
            ),
            true,
            false,
            &[News],
        ),
        UpdateStatus::Ready(news) => said(
            format!(
                "Versão {} instalada. Ela entra na próxima vez que o app abrir — ou reabra agora.",
                news.versao
            ),
            false,
            news.importante,
            &[Reopen],
        ),
        UpdateStatus::Failed(_, reason) => said(
            format!("A atualização não deu certo, e a versão {current} continua funcionando. {reason}"),
            false,
            false,
            &[Retry, HowToUpdate, Close],
        ),
    }
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
    status: Rc<RefCell<UpdateStatus>>,
    inbox: Rc<RefCell<Option<Receiver<Message>>>>,
    waker: Rc<RefCell<Option<UiWaker>>>,
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

impl Default for Updater {
    fn default() -> Self {
        Self::idle()
    }
}

impl Updater {
    /// An updater that does nothing until asked; tests use this one.
    pub fn idle() -> Self {
        Self {
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

    /// Gives it what wakes the UI, and runs a check that was waiting for it.
    pub fn connect(&self, waker: UiWaker) {
        if self.waker.borrow().is_none() {
            *self.waker.borrow_mut() = Some(waker);
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

    /// What the status bar says now.
    pub fn notice(&self) -> Option<Notice> {
        notice(&self.status(), CURRENT, self.stage.borrow().as_deref(), self.dismissed.borrow().as_deref())
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
        join_history([announced, self.remote_history.borrow().clone(), history()])
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
        std::thread::spawn(move || {
            let _wake_when_done = WakeOnDrop(waker);
            let (news, failure) = match latest_news() {
                Ok(news) => (Some(news), None),
                Err(reason) => (None, Some(reason)),
            };
            let home = paths::home();
            let now = seconds_now();
            let decision = decide(
                InstallKind::current(),
                CURRENT,
                news,
                &read_memory(&home.join("atualizacao.json")),
                is_locked(&home.join("atualizacao.trava"), now),
                now,
            );
            let history = match decision {
                Decision::Nothing => Vec::new(),
                Decision::Announce { .. } => HISTORY_URLS
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
                        if InstallKind::current() == InstallKind::Development {
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
        std::thread::spawn(move || {
            // Dropping this wakes the UI however the thread ends, even in a panic,
            // so a thread that died cannot leave it waiting.
            let _wake_when_done = WakeOnDrop(waker.clone());
            let stages = send.clone();
            let result = run_installer(&news.versao, &move |stage| {
                let _ = stages.send(Message::Stage(stage));
                waker.wake();
            });
            let _ = send.send(Message::Finished(news, result));
        });
    }

    /// Opens the installed version and closes this one.
    pub fn reopen(&self) {
        match paths::installed_binary().map(|bin| std::process::Command::new(bin).spawn()) {
            Some(Ok(_)) => std::process::exit(0),
            Some(Err(e)) => tracing::warn!(error = %e, "could not open the new version"),
            None => tracing::warn!("no installed binary on this system"),
        }
    }
}

fn waker_for_clock(waker: &Rc<RefCell<Option<UiWaker>>>) -> UiWaker {
    waker.borrow().clone().unwrap_or_default()
}

struct WakeOnDrop(UiWaker);

impl Drop for WakeOnDrop {
    fn drop(&mut self) {
        self.0.wake();
    }
}

fn download(url: &str) -> Result<String, String> {
    let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    response.body_mut().read_to_string().map_err(|e| e.to_string())
}

/// The latest version the repository announces, or why it could not be known.
fn latest_news() -> Result<News, String> {
    let mut reason = "a resposta não veio completa.".to_owned();
    for url in NEWS_URLS {
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

fn run_installer(version: &str, on_stage: &dyn Fn(String)) -> Result<(), String> {
    let home = paths::home();
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    let (lock, memory) = (home.join("atualizacao.trava"), home.join("atualizacao.json"));
    let now = seconds_now();
    if is_locked(&lock, now) {
        return Err("outra atualização já está em andamento neste computador.".to_owned());
    }
    std::fs::write(&lock, now.to_string()).map_err(|e| e.to_string())?;
    let result = compile_and_check(&home, version, on_stage);
    // Without this the next start would take it for a running installer until the lock expires.
    let _ = std::fs::remove_file(&lock);
    let remembered = match &result {
        Ok(()) => Memory::default(),
        Err(_) => Memory { failed: Some(version.to_owned()), when: Some(seconds_now()) },
    };
    write_memory(&memory, &remembered);
    result
}

/// Runs the installer, telling each step it announces in its log as it gets there.
fn follow_installer(
    mut command: std::process::Command,
    log: &Path,
    on_stage: &dyn Fn(String),
) -> Result<std::process::ExitStatus, String> {
    let mut child = command.spawn().map_err(|e| format!("não consegui rodar o instalador: {e}"))?;
    let mut told = None;
    loop {
        // Not `read_to_string`: a line that is not UTF-8 must not hide the steps.
        let stage = std::fs::read(log).ok().and_then(|text| last_stage(&String::from_utf8_lossy(&text)));
        if stage.is_some() && stage != told {
            told.clone_from(&stage);
            on_stage(stage.unwrap_or_default());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status);
        }
        std::thread::sleep(STAGE_EVERY);
    }
}

fn compile_and_check(home: &Path, version: &str, on_stage: &dyn Fn(String)) -> Result<(), String> {
    let text = download(INSTALLER_URL).map_err(|e| format!("não consegui baixar o instalador: {e}"))?;
    if !installer_is_whole(&text) {
        return Err("o instalador baixado veio incompleto; tente de novo mais tarde.".to_owned());
    }
    let file: PathBuf = home.join("install.cmd");
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    let (program, arguments) = installer_command(&file);
    let log_path = home.join("atualizacao.log");
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let errors = log.try_clone().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(program);
    command.args(arguments).env("FF_SEM_PAUSA", "1").stdout(log).stderr(errors);
    let status = follow_installer(command, &log_path, on_stage)?;
    if !status.success() {
        return Err(format!("a compilação falhou; veja {}.", home.join("atualizacao.log").display()));
    }
    let binary = paths::installed_binary().ok_or("não sei onde o app fica neste sistema.")?;
    let output = std::process::Command::new(binary)
        .arg("--versao")
        .output()
        .map_err(|e| format!("o app novo não abriu: {e}"))?;
    let answered = String::from_utf8_lossy(&output.stdout);
    if answered.trim() == version {
        Ok(())
    } else {
        Err(format!("o instalado responde {}, e não {version}.", answered.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn news(version: &str) -> News {
        News {
            versao: version.to_owned(),
            data: None,
            titulo: "Algo novo".to_owned(),
            importante: false,
            novidades: vec![],
            por_que_atualizar: "Porque sim".to_owned(),
        }
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("nova", "0.1.0"));
    }

    #[test]
    fn the_three_release_files_agree() {
        let news = parse_news(include_str!("../novidades.json")).unwrap_or_else(|| panic!("novidades.json"));
        assert_eq!(news.versao, CURRENT, "raise novidades.json together with Cargo.toml");
        let history = history();
        let top = history.first().unwrap_or_else(|| panic!("empty history"));
        assert!(top.data.is_some(), "the history records the day");
        let same = News { data: None, ..top.clone() };
        assert_eq!(same, news, "the history starts with this version's entry");
    }

    #[test]
    fn broken_news_is_no_news() {
        assert_eq!(parse_news("<html>404</html>"), None);
        assert_eq!(parse_news(r#"{"versao":"x","titulo":"a","novidades":[],"por_que_atualizar":"b"}"#), None);
    }

    #[test]
    fn an_installed_copy_starts_compiling_by_itself() {
        let decision =
            decide(InstallKind::Installed, "0.1.0", Some(news("0.2.0")), &Memory::default(), false, 1000);
        assert_eq!(decision, Decision::Announce { news: news("0.2.0"), start: true });
    }

    #[test]
    fn nothing_starts_by_itself_in_development_or_while_busy() {
        for (kind, busy) in [(InstallKind::Development, false), (InstallKind::Installed, true)] {
            let decision = decide(kind, "0.1.0", Some(news("0.2.0")), &Memory::default(), busy, 1000);
            assert_eq!(decision, Decision::Announce { news: news("0.2.0"), start: false });
        }
    }

    #[test]
    fn a_failed_version_waits_a_day() {
        let memory = Memory { failed: Some("0.2.0".to_owned()), when: Some(1000) };
        let soon = 1000 + RETRY_AFTER.as_secs() - 1;
        let later = 1000 + RETRY_AFTER.as_secs();
        let start_at =
            |now| match decide(InstallKind::Installed, "0.1.0", Some(news("0.2.0")), &memory, false, now) {
                Decision::Announce { start, .. } => start,
                Decision::Nothing => panic!("should announce"),
            };
        assert!(!start_at(soon));
        assert!(start_at(later));
        // Another version is not held back by this one's failure.
        let other = decide(InstallKind::Installed, "0.1.0", Some(news("0.3.0")), &memory, false, soon);
        assert!(matches!(other, Decision::Announce { start: true, .. }));
    }

    #[test]
    fn no_announcement_when_up_to_date_or_offline() {
        let quiet = |n| decide(InstallKind::Installed, "0.2.0", n, &Memory::default(), false, 0);
        assert_eq!(quiet(None), Decision::Nothing);
        assert_eq!(quiet(Some(news("0.2.0"))), Decision::Nothing);
        assert_eq!(quiet(Some(news("0.1.0"))), Decision::Nothing);
    }

    #[test]
    fn the_installer_in_the_repository_is_whole_and_a_cut_one_is_not() {
        let text = include_str!("../install.cmd");
        assert!(installer_is_whole(text));
        assert!(!installer_is_whole(&text[..text.len() / 2]));
        assert!(!installer_is_whole("<html>error</html>"));
        assert!(!installer_is_whole(""));
    }

    #[test]
    fn a_thread_that_died_stops_the_waiting_and_says_so() {
        let updater = Updater::idle();
        let (send, receive) = channel::<Message>();
        *updater.inbox.borrow_mut() = Some(receive);
        updater.set_status(UpdateStatus::Compiling(news("0.2.0")));
        updater.poll();
        assert!(updater.is_running(), "still alive: nothing to read yet");
        drop(send);
        updater.poll();
        assert!(!updater.is_running());
        assert!(matches!(updater.status(), UpdateStatus::Failed(..)));
    }

    #[test]
    fn the_status_bar_says_what_the_other_app_says() {
        let n = news("0.2.0");
        let say = |status: &UpdateStatus| notice(status, "0.1.0", Some("compilando"), None);
        use NoticeAction::*;

        assert_eq!(say(&UpdateStatus::Quiet), None);

        let available = say(&UpdateStatus::Available(n.clone())).unwrap();
        assert!(available.text.starts_with("Versão 0.2.0 disponível — "));
        assert_eq!(available.actions, [News, Update, Later]);
        assert!(!available.important && !available.busy);

        let compiling = say(&UpdateStatus::Compiling(n.clone())).unwrap();
        assert!(compiling.text.contains("em segundo plano — compilando. O app continua funcionando."));
        assert!(compiling.busy);
        assert_eq!(compiling.actions, [News]);
        let starting = notice(&UpdateStatus::Compiling(n.clone()), "0.1.0", None, None).unwrap();
        assert!(starting.text.contains("— começando."));

        let ready = say(&UpdateStatus::Ready(n.clone())).unwrap();
        assert!(ready.text.contains("entra na próxima vez que o app abrir"));
        assert_eq!(ready.actions, [Reopen]);

        let failed = say(&UpdateStatus::Failed(n.clone(), "sem disco.".to_owned())).unwrap();
        assert!(failed.text.contains("a versão 0.1.0 continua funcionando. sem disco."));
        assert_eq!(failed.actions, [Retry, HowToUpdate, Close]);

        let checking = say(&UpdateStatus::Checking).unwrap();
        assert!(checking.busy && checking.actions.is_empty());
        assert!(say(&UpdateStatus::UpToDate).unwrap().text.contains("mais recente (0.1.0)"));
        assert_eq!(say(&UpdateStatus::NoAnswer("sem rede".to_owned())).unwrap().actions, [CheckAgain, Close]);
    }

    #[test]
    fn only_an_important_version_stands_out() {
        let important = News { importante: true, ..news("0.2.0") };
        let say = |status| notice(&status, "0.1.0", None, None).unwrap();
        let offered = say(UpdateStatus::Available(important.clone()));
        assert!(offered.important && offered.text.starts_with("Atualização importante: versão 0.2.0"));
        assert!(say(UpdateStatus::Ready(important.clone())).important);
        assert!(!say(UpdateStatus::Compiling(important.clone())).important);
        assert!(!say(UpdateStatus::Failed(important, String::new())).important);
    }

    #[test]
    fn later_hides_that_version_for_the_session_and_nothing_under_way() {
        let n = news("0.2.0");
        let hide = Some("0.2.0");
        assert_eq!(notice(&UpdateStatus::Available(n.clone()), "0.1.0", None, hide), None);
        assert_eq!(notice(&UpdateStatus::Failed(n.clone(), String::new()), "0.1.0", None, hide), None);
        assert!(notice(&UpdateStatus::Available(news("0.3.0")), "0.1.0", None, hide).is_some());
        assert!(notice(&UpdateStatus::Compiling(n.clone()), "0.1.0", None, hide).is_some());
        assert!(notice(&UpdateStatus::Ready(n), "0.1.0", None, hide).is_some());
    }

    #[test]
    fn dismissing_hides_a_version_and_clears_an_answer_and_asking_brings_it_back() {
        let updater = Updater::idle();
        updater.set_status(UpdateStatus::Available(news("0.2.0")));
        updater.dismiss();
        assert_eq!(updater.notice(), None);
        assert!(matches!(updater.status(), UpdateStatus::Available(_)), "the version is still known");
        updater.ask();
        assert_eq!(updater.status(), UpdateStatus::Checking);
        assert!(updater.dismissed.borrow().is_none());

        updater.set_status(UpdateStatus::UpToDate);
        updater.dismiss();
        assert_eq!(updater.status(), UpdateStatus::Quiet);
    }

    #[test]
    fn asking_does_not_get_in_the_way_of_an_update_under_way() {
        let updater = Updater::idle();
        for status in [UpdateStatus::Compiling(news("0.2.0")), UpdateStatus::Ready(news("0.2.0"))] {
            updater.set_status(status.clone());
            updater.ask();
            assert_eq!(updater.status(), status);
        }
    }

    fn answer(updater: &Updater, decision: Decision, failure: Option<&str>) {
        let (send, receive) = channel();
        *updater.inbox.borrow_mut() = Some(receive);
        send.send(Message::Checked { decision, failure: failure.map(str::to_owned), history: Vec::new() })
            .unwrap();
        updater.poll();
    }

    #[test]
    fn only_a_check_someone_asked_for_answers_when_there_is_nothing_new() {
        let updater = Updater::idle();
        answer(&updater, Decision::Nothing, None);
        assert_eq!(updater.status(), UpdateStatus::Quiet, "the hourly look says nothing");
        answer(&updater, Decision::Nothing, Some("sem rede"));
        assert_eq!(updater.status(), UpdateStatus::Quiet, "nor does it complain about the network");

        updater.set_status(UpdateStatus::Checking);
        answer(&updater, Decision::Nothing, None);
        assert_eq!(updater.status(), UpdateStatus::UpToDate);

        updater.set_status(UpdateStatus::Checking);
        answer(&updater, Decision::Nothing, Some("sem rede"));
        assert_eq!(updater.status(), UpdateStatus::NoAnswer("sem rede".to_owned()));
    }

    #[test]
    fn looking_again_does_not_erase_a_failure_on_the_same_version() {
        let updater = Updater::idle();
        let failed = UpdateStatus::Failed(news("0.2.0"), "sem disco.".to_owned());
        updater.set_status(failed.clone());
        if InstallKind::current() == InstallKind::Installed {
            answer(&updater, Decision::Announce { news: news("0.2.0"), start: false }, None);
            assert_eq!(updater.status(), failed);
        }
    }

    #[test]
    fn it_looks_again_on_the_hour_unless_something_is_under_way() {
        let hour = CHECK_EVERY;
        let minute = Duration::from_secs(60);
        assert!(check_is_due(None, &UpdateStatus::Quiet));
        assert!(!check_is_due(Some(minute), &UpdateStatus::Quiet));
        assert!(check_is_due(Some(hour), &UpdateStatus::Quiet));
        assert!(check_is_due(Some(hour), &UpdateStatus::Available(news("0.2.0"))));
        assert!(!check_is_due(Some(hour), &UpdateStatus::Compiling(news("0.2.0"))));
        assert!(!check_is_due(Some(hour), &UpdateStatus::Ready(news("0.2.0"))));
        assert!(!check_is_due(Some(hour), &UpdateStatus::Checking));
    }

    #[test]
    fn a_test_updater_never_looks_by_itself() {
        let updater = Updater::idle();
        updater.connect(UiWaker::default());
        updater.poll();
        assert!(!updater.is_running());
    }

    #[test]
    fn the_steps_come_from_the_lines_the_installer_announces() {
        assert_eq!(installer_stage("> baixando o codigo"), Some("baixando o codigo".to_owned()));
        assert_eq!(installer_stage("   Compiling ravenrs v27.1.1"), None);
        assert_eq!(installer_stage("> "), None);
        let log = "\n> baixando o codigo\nFrom github\n\n> compilando\n   Compiling x\nwarning: y\n";
        assert_eq!(last_stage(log).as_deref(), Some("compilando"));
        assert_eq!(last_stage("nada anunciado\n"), None);
    }

    #[test]
    fn the_installer_announces_steps_on_every_system() {
        let installer = include_str!("../install.cmd");
        assert!(installer.contains("printf '\\n> %s\\n'"), "sh");
        assert!(installer.contains("Write-Host \"> compilando"), "PowerShell");
    }

    #[test]
    fn the_timeline_joins_what_is_known_with_the_newest_on_top() {
        let told_by_repository = News { titulo: "do repositório".to_owned(), ..news("0.2.0") };
        let joined = join_history([
            vec![news("0.3.0")],
            vec![told_by_repository.clone(), news("0.1.10")],
            vec![news("0.1.9"), news("0.1.10")],
        ]);
        let versions: Vec<&str> = joined.iter().map(|n| n.versao.as_str()).collect();
        assert_eq!(versions, ["0.3.0", "0.2.0", "0.1.10", "0.1.9"]);
        assert_eq!(joined[1].titulo, "do repositório");

        let updater = Updater::idle();
        updater.set_status(UpdateStatus::Available(news("99.0.0")));
        let timeline = updater.timeline();
        assert_eq!(timeline[0].versao, "99.0.0");
        assert!(timeline.iter().any(|n| n.versao == CURRENT));
    }

    #[test]
    fn a_broken_history_entry_is_skipped() {
        let text = r#"[{"versao":"0.1.0","titulo":"t","novidades":[],"por_que_atualizar":"p"},{"nada":1}]"#;
        assert_eq!(parse_history(text).len(), 1);
        assert!(parse_history("<html>erro</html>").is_empty());
    }

    #[test]
    fn the_command_by_hand_is_the_one_the_readme_shows() {
        let readme = include_str!("../README.md");
        assert!(readme.contains(INSTALL_COMMAND), "{INSTALL_COMMAND}");
    }

    #[test]
    fn a_check_asked_for_before_the_window_exists_runs_when_it_does() {
        let updater = Updater::idle();
        updater.check();
        assert!(!updater.is_running(), "nothing could wake the UI yet");
        assert!(updater.check_when_connected.get());
        // The thread starts here and asks the network; only whether it started matters.
        updater.connect(UiWaker::default());
        assert!(updater.is_running());
        assert!(!updater.check_when_connected.get());
    }

    #[test]
    fn macos_lists_one_app_with_its_new_icon() {
        let text = include_str!("../install.cmd");
        // In the Applications folder, the previous version would show as a second app.
        assert!(text.contains(r#"ANTERIOR="$CASA/anterior/$NOME.app""#));
        // macOS keeps icons by path, so it has to be told the app changed.
        assert!(text.contains("lsregister"));
        // A version that changes on every install is what makes it read the icon again.
        assert!(text.contains("<key>CFBundleVersion</key><string>$(date"));
    }

    #[test]
    fn an_update_recompiles_what_changed() {
        // Without -m the files keep their commit's date, older than the last build, so
        // Cargo skipped the crates that changed and the update failed to compile.
        let text = include_str!("../install.cmd");
        assert_eq!(text.matches("tar -xzmf").count(), 2, "on both sides of the installer");
        assert!(!text.contains("tar -xzf"));
    }

    #[test]
    fn a_stale_lock_does_not_hold_updates_back() {
        let dir = std::env::temp_dir().join(format!("ff-lock-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let lock = dir.join("trava");
        assert!(!is_locked(&lock, 100));
        std::fs::write(&lock, "100").unwrap_or_else(|e| panic!("{e}"));
        assert!(is_locked(&lock, 200));
        assert!(!is_locked(&lock, 100 + LOCK_EXPIRES.as_secs()));
        let _ = std::fs::remove_file(&lock);
    }
}
