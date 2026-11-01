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
//! | the app checks the version of the installed binary | the banner says so; the open app goes on |
//!
//! Releasing is one commit: the `version` in `Cargo.toml`, `novidades.json` and
//! the same entry on top of `historico-de-novidades.json`. Tests hold the three together.
//!
//! The installer is saved to a file and never piped into a shell: a failed
//! `curl | sh` hands `sh` an empty script, which exits successfully having done nothing.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

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

/// The installer the app downloads and runs, the same one the README shows.
pub const INSTALLER_URL: &str =
    "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd";

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

/// Every version up to this one, newest first, compiled into the app.
pub fn history() -> Vec<News> {
    let items: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../historico-de-novidades.json")).unwrap_or_default();
    items.into_iter().filter_map(|item| serde_json::from_value(item).ok()).collect()
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

/// Where an update stands, for the banner and the settings page.
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
}

/// Checks for updates and runs the installer on background threads. Results
/// come through [`Updater::poll`], which the UI calls on each build; while
/// something runs, [`Updater::is_running`] tells it to keep waking up.
#[derive(Clone, Debug)]
pub struct Updater {
    status: Rc<RefCell<UpdateStatus>>,
    inbox: Rc<RefCell<Option<Receiver<Message>>>>,
}

#[derive(Debug)]
enum Message {
    Checked(Decision),
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
        Self { status: Rc::new(RefCell::new(UpdateStatus::Quiet)), inbox: Rc::new(RefCell::new(None)) }
    }

    /// Asks the network in the background.
    pub fn check(&self) {
        let (send, receive) = channel();
        *self.inbox.borrow_mut() = Some(receive);
        std::thread::spawn(move || {
            let news = NEWS_URLS.iter().find_map(|url| download(url).ok().as_deref().and_then(parse_news));
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
            // The receiver going away only means the window closed.
            let _ = send.send(Message::Checked(decision));
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

    /// A background thread is working, so the UI should come back and [`Self::poll`] again.
    pub fn is_running(&self) -> bool {
        self.inbox.borrow().is_some()
    }

    /// Takes in what the background thread finished. Called while building, so it
    /// writes plain cells: the view that called it reads them right after.
    pub fn poll(&self) {
        let received = match self.inbox.borrow().as_ref() {
            Some(inbox) => inbox.try_recv(),
            None => return,
        };
        let message = match received {
            Ok(message) => message,
            Err(TryRecvError::Empty) => return,
            // The thread died without answering. Waiting on would wake the UI forever.
            Err(TryRecvError::Disconnected) => {
                *self.inbox.borrow_mut() = None;
                if let UpdateStatus::Compiling(news) = self.status() {
                    self.set_status(UpdateStatus::Failed(news, "a atualização parou no meio.".to_owned()));
                }
                return;
            }
        };
        *self.inbox.borrow_mut() = None;
        match message {
            Message::Checked(Decision::Nothing) => {}
            Message::Checked(Decision::Announce { news, start }) => {
                if InstallKind::current() == InstallKind::Development {
                    self.set_status(UpdateStatus::Development(news));
                } else if start {
                    self.install(news);
                } else {
                    self.set_status(UpdateStatus::Available(news));
                }
            }
            Message::Finished(news, Ok(())) => self.set_status(UpdateStatus::Ready(news)),
            Message::Finished(news, Err(reason)) => self.set_status(UpdateStatus::Failed(news, reason)),
        }
    }

    /// Downloads the installer and runs it in the background.
    pub fn install(&self, news: News) {
        self.set_status(UpdateStatus::Compiling(news.clone()));
        let (send, receive) = channel();
        *self.inbox.borrow_mut() = Some(receive);
        std::thread::spawn(move || {
            let result = run_installer(&news.versao);
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

fn download(url: &str) -> Result<String, String> {
    let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    response.body_mut().read_to_string().map_err(|e| e.to_string())
}

fn run_installer(version: &str) -> Result<(), String> {
    let home = paths::home();
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    let (lock, memory) = (home.join("atualizacao.trava"), home.join("atualizacao.json"));
    let now = seconds_now();
    if is_locked(&lock, now) {
        return Err("outra atualização já está em andamento neste computador.".to_owned());
    }
    std::fs::write(&lock, now.to_string()).map_err(|e| e.to_string())?;
    let result = compile_and_check(&home, version);
    // Without this the next start would take it for a running installer until the lock expires.
    let _ = std::fs::remove_file(&lock);
    let remembered = match &result {
        Ok(()) => Memory::default(),
        Err(_) => Memory { failed: Some(version.to_owned()), when: Some(seconds_now()) },
    };
    write_memory(&memory, &remembered);
    result
}

fn compile_and_check(home: &Path, version: &str) -> Result<(), String> {
    let text = download(INSTALLER_URL).map_err(|e| format!("não consegui baixar o instalador: {e}"))?;
    if !installer_is_whole(&text) {
        return Err("o instalador baixado veio incompleto; tente de novo mais tarde.".to_owned());
    }
    let file: PathBuf = home.join("install.cmd");
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    let (program, arguments) = installer_command(&file);
    let log = std::fs::File::create(home.join("atualizacao.log")).map_err(|e| e.to_string())?;
    let errors = log.try_clone().map_err(|e| e.to_string())?;
    let status = std::process::Command::new(program)
        .args(arguments)
        .env("FF_SEM_PAUSA", "1")
        .stdout(log)
        .stderr(errors)
        .status()
        .map_err(|e| format!("não consegui rodar o instalador: {e}"))?;
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
        let (send, receive) = channel();
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
