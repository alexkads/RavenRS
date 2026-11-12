//! The installer: downloading it whole, running it, and following what it does.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::*;
use crate::paths;

/// The installer the app downloads and runs, the same one the README shows.
pub const INSTALLER_URL: &str =
    "https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd";

/// Running the installer by hand, for when updating from inside the app fails.
pub const INSTALL_COMMAND: &str = if cfg!(target_os = "windows") {
    "irm https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd -OutFile \"$env:TEMP\\install.cmd\"; & \"$env:TEMP\\install.cmd\""
} else {
    "curl -fsSL https://raw.githubusercontent.com/alexkads/RavenRS/master/examples/family-finance/install.cmd -o /tmp/install.cmd && sh /tmp/install.cmd"
};

/// A lock older than this belongs to an installer that died without removing it.
pub const LOCK_EXPIRES: Duration = Duration::from_secs(3 * 3600);

/// A running installer writes to its log at least this often; a lock whose log has
/// been quiet for longer belongs to a window that closed in the middle.
pub const LOG_QUIET_FOR: Duration = Duration::from_secs(20 * 60);

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

/// The step the installer announced on a line of its log: `> text`.
/// What it adds in parentheses is for a terminal: the status bar's sentence has
/// no room for it, and its end is what must not get cut.
pub fn installer_stage(line: &str) -> Option<String> {
    let text = line.trim().strip_prefix("> ")?.trim();
    let text = text.split(" (").next().unwrap_or(text).trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The last step announced in the log so far.
pub(super) fn last_stage(log: &str) -> Option<String> {
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

/// An installer is running. The lock holds the time it began, and the installer's
/// log, beside it, must still be getting written: the window that took the lock may
/// have closed before it could let go.
pub fn is_locked(lock: &Path, now: u64) -> bool {
    let Some(began) = std::fs::read_to_string(lock).ok().and_then(|text| text.trim().parse::<u64>().ok())
    else {
        return false;
    };
    let written = std::fs::metadata(lock.with_file_name("atualizacao.log"))
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

/// The end of the installer's log, for the details of a failure.
pub fn installer_log_tail(lines: usize) -> Option<String> {
    let text =
        std::fs::read(installer_log()).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned())?;
    let kept: Vec<&str> = text.lines().rev().take(lines).collect();
    let tail = kept.into_iter().rev().collect::<Vec<_>>().join("\n");
    (!tail.trim().is_empty()).then_some(tail)
}

/// Where the installer writes what it does.
pub fn installer_log() -> PathBuf {
    paths::home().join("atualizacao.log")
}

pub(super) fn seconds_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub(super) fn read_memory(file: &Path) -> Memory {
    std::fs::read(file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub(super) fn write_memory(file: &Path, memory: &Memory) {
    if let Ok(text) = serde_json::to_vec_pretty(memory)
        && let Err(e) = std::fs::write(file, text)
    {
        tracing::warn!(error = %e, "could not remember the failed update");
    }
}

pub(super) fn download(url: &str) -> Result<String, String> {
    let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    response.body_mut().read_to_string().map_err(|e| e.to_string())
}

pub(super) fn run_installer(version: &str, on_stage: &dyn Fn(String)) -> Result<(), String> {
    let home = paths::home();
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    let (lock, memory) = (home.join("atualizacao.trava"), home.join("atualizacao.json"));
    let now = seconds_now();
    if is_locked(&lock, now) {
        return follow_other_installer(&lock, version, on_stage);
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

/// Another window is installing: follows its steps until it lets go of the lock,
/// then asks the installed app which version it is.
pub(super) fn follow_other_installer(
    lock: &Path,
    version: &str,
    on_stage: &dyn Fn(String),
) -> Result<(), String> {
    let log = lock.with_file_name("atualizacao.log");
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
    installed_answers(version)
}

/// The installed app opens and says it is `version`.
pub(super) fn installed_answers(version: &str) -> Result<(), String> {
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

/// Runs the installer, telling each step it announces in its log as it gets there.
pub(super) fn follow_installer(
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

pub(super) fn compile_and_check(home: &Path, version: &str, on_stage: &dyn Fn(String)) -> Result<(), String> {
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
    installed_answers(version)
}
