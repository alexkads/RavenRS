//! The installer: downloading it whole, running it, and following what it does.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::InstalledApp;

/// What the installer writes, in the app's home.
pub(crate) const LOG: &str = "atualizacao.log";
/// The time an installer began, while it runs.
pub(crate) const LOCK: &str = "atualizacao.trava";
/// The [`Memory`] of the last update.
pub(crate) const MEMORY: &str = "atualizacao.json";
/// The installer as downloaded, which is what runs.
const DOWNLOADED: &str = "install.cmd";

/// A lock older than this belongs to an installer that died without removing it.
pub const LOCK_EXPIRES: Duration = Duration::from_secs(3 * 3600);

/// A running installer writes to its log at least this often; a lock whose log has
/// been quiet for longer belongs to a window that closed in the middle.
pub const LOG_QUIET_FOR: Duration = Duration::from_secs(20 * 60);

/// How often the installer's log is read for the step it is on.
const STAGE_EVERY: Duration = Duration::from_secs(1);

/// Remembers an update that failed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// The version that failed last.
    pub failed: Option<String>,
    /// When, in seconds since 1970.
    pub when: Option<u64>,
}

/// The step the installer announced on a line of its log: `> text`.
/// What it adds in parentheses is for a terminal: the status bar's sentence has
/// no room for it, and its end is what must not get cut.
pub fn installer_stage(line: &str) -> Option<String> {
    let text = line.trim().strip_prefix("> ")?.trim();
    let text = text.split(" (").next().unwrap_or(text).trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The last step announced in the log so far.
pub(crate) fn last_stage(log: &str) -> Option<String> {
    log.lines().rev().find_map(installer_stage)
}

/// The downloaded file really is the whole installer. An error page, an empty
/// file or one cut at any point does not pass, because the end of the file has to be there.
///
/// Copies already installed check with these same marks, so the template keeps them.
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

/// An installer is running. The lock holds the time it began, and the installer's
/// log, beside it, must still be getting written: the window that took the lock may
/// have closed before it could let go.
pub fn is_locked(lock: &Path, now: u64) -> bool {
    let Some(began) = std::fs::read_to_string(lock).ok().and_then(|text| text.trim().parse::<u64>().ok())
    else {
        return false;
    };
    let written = std::fs::metadata(lock.with_file_name(LOG))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|when| when.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_secs());
    lock_is_live(began, written, now)
}

/// The rule behind [`is_locked`]: begun less than [`LOCK_EXPIRES`] ago, and its log
/// written in the last [`LOG_QUIET_FOR`]. A log older than the lock is the last run's.
pub fn lock_is_live(began: u64, log_written: Option<u64>, now: u64) -> bool {
    let last_sign = log_written.filter(|&written| written >= began).unwrap_or(began);
    now.saturating_sub(began) < LOCK_EXPIRES.as_secs()
        && now.saturating_sub(last_sign) < LOG_QUIET_FOR.as_secs()
}

/// Seconds since 1970, by the machine's clock.
pub(crate) fn seconds_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// What is remembered about the last update; nothing, when it is missing or unreadable.
pub fn read_memory(file: &Path) -> Memory {
    std::fs::read(file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn write_memory(file: &Path, memory: &Memory) {
    if let Ok(text) = serde_json::to_vec_pretty(memory)
        && let Err(e) = std::fs::write(file, text)
    {
        tracing::warn!(error = %e, "could not remember the failed update");
    }
}

pub(crate) fn download(url: &str) -> Result<String, String> {
    let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    response.body_mut().read_to_string().map_err(|e| e.to_string())
}

/// Where one run of the installer happens. The app's comes from its [`InstalledApp`];
/// tests give a folder of their own and an installer that does not compile.
pub(crate) struct Setup<'a> {
    /// Where the lock, the memory, the log and the downloaded installer go.
    pub home: PathBuf,
    /// The installed app, which has to answer `--versao` with the new version.
    pub binary: Option<PathBuf>,
    /// The variable that keeps the installer from waiting for a key.
    pub no_pause_env: String,
    /// Gets the installer's text.
    pub fetch: Box<dyn Fn() -> Result<String, String> + 'a>,
}

impl Setup<'static> {
    /// The app's own: its home, its installed binary, and the installer from its address.
    pub(crate) fn of(app: &InstalledApp) -> Self {
        let url = app.installer_url;
        Setup {
            home: app.home(),
            binary: app.installed_binary(),
            no_pause_env: app.no_pause_env(),
            fetch: Box::new(move || download(url)),
        }
    }
}

/// Runs the installer once, unless another window already is: then follows that one.
/// A failure is remembered, so that version is not tried again by itself for a day.
pub(crate) fn run_installer(
    setup: &Setup<'_>,
    version: &str,
    on_stage: &dyn Fn(String),
) -> Result<(), String> {
    let home = &setup.home;
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let (lock, memory) = (home.join(LOCK), home.join(MEMORY));
    let now = seconds_now();
    if is_locked(&lock, now) {
        return follow_other_installer(&lock, setup.binary.as_deref(), version, on_stage);
    }
    std::fs::write(&lock, now.to_string()).map_err(|e| e.to_string())?;
    let result = compile_and_check(setup, version, on_stage);
    // Without this the next start would take it for a running installer until the lock expires.
    let _ = std::fs::remove_file(&lock);
    let remembered = match &result {
        Ok(()) => Memory::default(),
        Err(_) => Memory { failed: Some(version.to_owned()), when: Some(seconds_now()) },
    };
    write_memory(&memory, &remembered);
    result
}

/// Another window is installing: follows its steps until it lets go of the lock,
/// then asks the installed app which version it is.
fn follow_other_installer(
    lock: &Path,
    binary: Option<&Path>,
    version: &str,
    on_stage: &dyn Fn(String),
) -> Result<(), String> {
    let log = lock.with_file_name(LOG);
    let mut told = None;
    while is_locked(lock, seconds_now()) {
        let stage = std::fs::read(&log).ok().and_then(|text| last_stage(&String::from_utf8_lossy(&text)));
        let stage = format!("outra janela está atualizando — {}", stage.as_deref().unwrap_or("começando"));
        if told.as_ref() != Some(&stage) {
            told = Some(stage.clone());
            on_stage(stage);
        }
        std::thread::sleep(STAGE_EVERY);
    }
    installed_answers(binary, version)
}

/// The installed app opens and says it is `version`.
pub(crate) fn installed_answers(binary: Option<&Path>, version: &str) -> Result<(), String> {
    let binary = binary.ok_or("não sei onde o app fica neste sistema.")?;
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

fn compile_and_check(setup: &Setup<'_>, version: &str, on_stage: &dyn Fn(String)) -> Result<(), String> {
    let text = (setup.fetch)().map_err(|e| format!("não consegui baixar o instalador: {e}"))?;
    if !installer_is_whole(&text) {
        return Err("o instalador baixado veio incompleto; tente de novo mais tarde.".to_owned());
    }
    let file: PathBuf = setup.home.join(DOWNLOADED);
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    let (program, arguments) = installer_command(&file);
    let log_path = setup.home.join(LOG);
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let errors = log.try_clone().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(program);
    command.args(arguments).env(&setup.no_pause_env, "1").stdout(log).stderr(errors);
    let status = follow_installer(command, &log_path, on_stage)?;
    if !status.success() {
        return Err(format!("a compilação falhou; veja {}.", log_path.display()));
    }
    installed_answers(setup.binary.as_deref(), version)
}
