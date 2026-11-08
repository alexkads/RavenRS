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
//! the same entry on top of `historico-de-novidades.json`, with `contribuidores`
//! naming who made it. Tests hold the three together.
//!
//! The installer is saved to a file and never piped into a shell: a failed
//! `curl | sh` hands `sh` an empty script, which exits successfully having done nothing.
//!
//! The app looks when it opens and every hour after that, silently: [`notice`]
//! only speaks when there is a version, or when someone asked and is waiting for an answer.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use ravenrs::prelude::UiWaker;

use crate::paths;

mod installer;
mod news;

pub use installer::{
    INSTALL_COMMAND, INSTALLER_URL, InstallKind, LOCK_EXPIRES, LOG_QUIET_FOR, Memory, installer_command,
    installer_is_whole, installer_log, installer_log_tail, installer_stage, is_locked, lock_is_live,
};
use installer::{download, read_memory, run_installer, seconds_now};
pub use news::{
    News, Notice, NoticeAction, UpdateStatus, history, is_newer, join_history, notice, parse_history,
    parse_news,
};

/// The running version.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Where an issue of this repository is read, by its number.
pub fn issue_url(number: u32) -> String {
    format!("https://github.com/alexkads/RavenRS/issues/{number}")
}

/// The first version whose news must name the issues it closes.
#[cfg(test)]
const ISSUES_FROM: &str = "0.1.32";

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

/// How often an open app looks for a new version.
pub const CHECK_EVERY: Duration = Duration::from_secs(3600);

/// How often the installer's log is read for the step it is on.
const STAGE_EVERY: Duration = Duration::from_secs(1);

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

    /// The step the installer was on, as it last said.
    pub fn stage(&self) -> Option<String> {
        self.stage.borrow().clone()
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::{installer::*, news::*};

    fn news(version: &str) -> News {
        News {
            versao: version.to_owned(),
            data: None,
            titulo: "Algo novo".to_owned(),
            importante: false,
            novidades: vec![],
            por_que_atualizar: "Porque sim".to_owned(),
            contribuidores: Vec::new(),
            issues: Vec::new(),
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
        let news =
            parse_news(include_str!("../../novidades.json")).unwrap_or_else(|| panic!("novidades.json"));
        assert_eq!(news.versao, CURRENT, "raise novidades.json together with Cargo.toml");
        assert!(
            !news.contribuidores.is_empty() && news.contribuidores.iter().all(|name| !name.trim().is_empty()),
            "novidades.json names who made the version in contribuidores"
        );
        if !is_newer(ISSUES_FROM, &news.versao) {
            assert!(!news.issues.is_empty(), "novidades.json lists the issues this version closes in issues");
        }
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
        let text = include_str!("../../install.cmd");
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
        assert!(failed.text.contains("a 0.1.0 continua funcionando."));
        assert!(!failed.text.contains("sem disco"), "the reason is in the details, where it is not cut");
        assert_eq!(failed.actions, [Retry, Details, Close]);

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
        assert_eq!(installer_stage("> compilando (a primeira vez demora)"), Some("compilando".to_owned()));
        let log = "\n> baixando o codigo\nFrom github\n\n> compilando\n   Compiling x\nwarning: y\n";
        assert_eq!(last_stage(log).as_deref(), Some("compilando"));
        assert_eq!(last_stage("nada anunciado\n"), None);
    }

    #[test]
    fn the_installer_announces_steps_on_every_system() {
        let installer = include_str!("../../install.cmd");
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

        let dated =
            News { data: Some("2026-11-01".to_owned()), titulo: "do histórico".to_owned(), ..news("0.2.0") };
        let joined = join_history([vec![told_by_repository], vec![dated]]);
        assert_eq!(joined[0].titulo, "do repositório");
        assert_eq!(joined[0].data.as_deref(), Some("2026-11-01"), "the announcement has no date of its own");

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
        let readme = include_str!("../../README.md");
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
        let text = include_str!("../../install.cmd");
        // In the Applications folder, the previous version would show as a second app.
        assert!(text.contains(r#"ANTERIOR="$CASA/anterior/$NOME.app""#));
        // macOS keeps icons by path, so it has to be told the app changed.
        assert!(text.contains("lsregister"));
        // A version that changes on every install is what makes it read the icon again.
        assert!(text.contains("<key>CFBundleVersion</key><string>$(date"));
    }

    #[test]
    fn gnome_finds_the_icon_through_the_window_id() {
        let installer = include_str!("../../install.cmd");
        let id = paths::APP_ID;
        assert!(installer.contains(&format!("share/applications/{id}.desktop")), "the file's name is the id");
        assert!(installer.contains(&format!("StartupWMClass={id}")));
        assert!(installer.contains(&format!("Icon={id}")));
        assert!(installer.contains(&format!("hicolor/256x256/apps/{id}.png")));
    }

    #[test]
    fn an_update_recompiles_what_changed() {
        // Without -m the files keep their commit's date, older than the last build, so
        // Cargo skipped the crates that changed and the update failed to compile.
        let text = include_str!("../../install.cmd");
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

    #[test]
    fn a_lock_whose_log_went_quiet_was_left_by_a_closed_window() {
        let (quiet, began) = (LOG_QUIET_FOR.as_secs(), 1_000);
        assert!(lock_is_live(began, None, began + 60), "just begun, no log yet");
        assert!(!lock_is_live(began, None, began + quiet + 1), "never wrote a line");
        assert!(lock_is_live(began, Some(began + 3_000), began + 3_000 + 60), "compiling, still writing");
        assert!(!lock_is_live(began, Some(began + 600), began + 600 + quiet + 1), "stopped writing");
        assert!(!lock_is_live(began, Some(began - 50), began + quiet + 1), "that log is the last run's");
        assert!(
            !lock_is_live(began, Some(began + 9_000), began + LOCK_EXPIRES.as_secs()),
            "never past the limit"
        );
    }
}
