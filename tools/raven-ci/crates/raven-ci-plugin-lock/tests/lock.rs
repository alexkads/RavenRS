//! The plugin lock (#871): builds that ask for one lock run one after another, a script outside
//! holding it makes them wait, a build that fails or is cancelled lets it go, and a program a step
//! leaves running does not keep it. Scripts are said for `sh` and for `cmd`.
#![allow(clippy::expect_used, clippy::panic, reason = "tests fail loudly")]

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use raven_ci_core::{Ci, Executors, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus};
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_lock::LockPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use serde_json::json;

fn os(unix: &str, windows: &str) -> String {
    if cfg!(windows) { windows.to_owned() } else { unix.to_owned() }
}

/// A folder of locks of its own, removed when dropped.
struct Locks(PathBuf);

impl Drop for Locks {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn locks() -> Locks {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("raven-ci-travas-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder");
    Locks(dir)
}

fn ci(dir: &Path) -> Ci {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(Arc::new(LockPlugin::new()));
    ci.apply_plugins().expect("on");
    ci.configure_plugin("lock", json!({ "dir": dir.display().to_string() })).expect("configured");
    ci
}

fn pipeline(script: &str) -> String {
    format!("[[stages]]\nname = \"Pesado\"\nlock = \"maquina\"\nsteps = ['{script}']\n")
}

fn texts(ci: &Ci, job: &str, number: u64) -> Vec<(i64, String)> {
    ci.build_log(job, number, 0)
        .expect("log")
        .into_iter()
        .map(|l| (l.at_ms, l.text.trim().to_owned()))
        .collect()
}

fn at(log: &[(i64, String)], text: &str) -> i64 {
    log.iter().find(|(_, t)| t == text).unwrap_or_else(|| panic!("{text} in {log:?}")).0
}

/// Waits until `job #number` ended, up to `limit`.
fn ended(ci: &Ci, job: &str, number: u64, limit: Duration) -> BuildStatus {
    let began = Instant::now();
    loop {
        let status = ci.build(job, number).expect("build").status;
        if status.ended() || began.elapsed() > limit {
            return status;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The lock file `maquina`, held by this test as a script outside would hold it.
fn hold(dir: &Path) -> File {
    let file =
        OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join("maquina.lock"));
    let file = file.expect("the file");
    file.lock().expect("held");
    file
}

fn free(dir: &Path) -> bool {
    let file =
        OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join("maquina.lock"));
    file.expect("the file").try_lock().is_ok()
}

#[test]
fn two_builds_that_ask_for_one_lock_run_one_after_the_other() {
    let locks = locks();
    let ci = Arc::new(ci(&locks.0));
    let slow = pipeline(&os("sleep 0.4", "ping -n 2 127.0.0.1 >nul"));
    ci.save_job(JobSpec::new("a", "toml", &slow)).expect("saved");
    ci.save_job(JobSpec::new("b", "toml", &slow)).expect("saved");
    let a = ci.request_build(BuildRequest::new("a", "teste")).expect("asked").number;
    let b = ci.request_build(BuildRequest::new("b", "teste")).expect("asked").number;
    let executors = Executors::start(ci.clone(), 2, Duration::from_millis(10));
    let limit = Duration::from_secs(20);
    assert_eq!(ended(&ci, "a", a, limit), BuildStatus::Succeeded);
    assert_eq!(ended(&ci, "b", b, limit), BuildStatus::Succeeded);
    executors.stop();
    let (log_a, log_b) = (texts(&ci, "a", a), texts(&ci, "b", b));
    let spans = [
        (at(&log_a, "peguei a trava maquina"), at(&log_a, "soltei a trava maquina")),
        (at(&log_b, "peguei a trava maquina"), at(&log_b, "soltei a trava maquina")),
    ];
    let overlap = spans[0].0 < spans[1].1 && spans[1].0 < spans[0].1;
    assert!(!overlap, "the two held it together: {spans:?}\n{log_a:?}\n{log_b:?}");
}

#[test]
fn a_script_outside_holding_the_lock_makes_the_build_wait_until_it_lets_go() {
    let locks = locks();
    let ci = Arc::new(ci(&locks.0));
    ci.save_job(JobSpec::new("site", "toml", pipeline("echo dentro"))).expect("saved");
    let outside = hold(&locks.0);
    let n = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    let executors = Executors::start(ci.clone(), 1, Duration::from_millis(10));
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(ci.build("site", n).expect("build").status, BuildStatus::Running);
    let waiting = texts(&ci, "site", n);
    assert!(waiting.iter().any(|(_, t)| t.starts_with("esperando a trava maquina")), "{waiting:?}");
    assert!(!waiting.iter().any(|(_, t)| t == "dentro"), "{waiting:?}");
    drop(outside);
    assert_eq!(ended(&ci, "site", n, Duration::from_secs(10)), BuildStatus::Succeeded);
    executors.stop();
    assert!(texts(&ci, "site", n).iter().any(|(_, t)| t == "dentro"));
}

#[test]
fn a_build_that_fails_inside_the_stage_lets_the_lock_go() {
    let locks = locks();
    let ci = ci(&locks.0);
    ci.save_job(JobSpec::new("quebra", "toml", pipeline(&os("exit 1", "exit /b 1")))).expect("saved");
    let n = ci.request_build(BuildRequest::new("quebra", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.build("quebra", n).expect("build").status, BuildStatus::Failed);
    assert!(free(&locks.0), "the lock was kept after the build failed");
    assert!(texts(&ci, "quebra", n).iter().any(|(_, t)| t == "no fim do build: soltar a trava maquina"));
}

#[test]
fn a_program_left_running_by_a_step_does_not_keep_the_lock() {
    let locks = locks();
    let ci = ci(&locks.0);
    let background = os(
        "sleep 3 >/dev/null 2>&1 &",
        "start \"\" /b cmd /c \"ping -n 4 127.0.0.1 >nul 2>&1\" <nul >nul 2>&1",
    );
    ci.save_job(JobSpec::new("fundo", "toml", pipeline(&background))).expect("saved");
    let n = ci.request_build(BuildRequest::new("fundo", "teste")).expect("asked").number;
    let began = Instant::now();
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.build("fundo", n).expect("build").status, BuildStatus::Succeeded);
    assert!(began.elapsed() < Duration::from_secs(3), "the build waited for the program it left");
    assert!(free(&locks.0), "the program left running kept the lock");
}

#[test]
fn cancelling_a_build_that_waits_for_the_lock_ends_it_as_cancelled() {
    let locks = locks();
    let ci = Arc::new(ci(&locks.0));
    ci.save_job(JobSpec::new("espera", "toml", pipeline("echo nunca"))).expect("saved");
    let _outside = hold(&locks.0);
    let n = ci.request_build(BuildRequest::new("espera", "teste")).expect("asked").number;
    let executors = Executors::start(ci.clone(), 1, Duration::from_millis(10));
    std::thread::sleep(Duration::from_millis(300));
    ci.cancel("espera", n, "não precisa mais").expect("cancelled");
    assert_eq!(ended(&ci, "espera", n, Duration::from_secs(10)), BuildStatus::Cancelled);
    executors.stop();
}

#[test]
fn a_lock_with_a_name_that_is_not_a_plain_word_is_refused() {
    let locks = locks();
    let ci = ci(&locks.0);
    let text = "[[stages]]\nname = \"T\"\nlock = \"../fora\"\nsteps = [\"echo x\"]\n";
    ci.save_job(JobSpec::new("nome", "toml", text)).expect("saved");
    let n = ci.request_build(BuildRequest::new("nome", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let build = ci.build("nome", n).expect("build");
    assert_eq!(build.status, BuildStatus::Failed);
    assert!(build.reason.unwrap_or_default().contains("não serve de nome de trava"));
    assert!(!locks.0.parent().expect("above").join("fora.lock").exists());
}

/// A lock in a file said by the stage (#930): the one a machine's scripts already `flock`, as the
/// trava of the oficina, outside the plugin's folder.
#[test]
fn a_lock_in_a_file_said_by_the_stage_waits_for_a_script_holding_that_file() {
    let locks = locks();
    let outside_dir = locks.0.join("de-fora");
    std::fs::create_dir_all(&outside_dir).expect("a folder");
    let trava = outside_dir.join(".trava-compilacao");
    let ci = Arc::new(ci(&locks.0));
    let text = format!(
        "[[stages]]\nname = \"Pesado\"\nlock = {{ name = \"oficina\", path = '{}' }}\nsteps = ['echo dentro']\n",
        trava.display()
    );
    ci.save_job(JobSpec::new("site", "toml", &text)).expect("saved");
    let held = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&trava)
        .expect("the file");
    held.lock().expect("held");
    let n = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    let executors = Executors::start(ci.clone(), 1, Duration::from_millis(10));
    std::thread::sleep(Duration::from_millis(500));
    let waiting = texts(&ci, "site", n);
    assert!(waiting.iter().any(|(_, t)| t.starts_with("esperando a trava oficina")), "{waiting:?}");
    assert!(!locks.0.join("oficina.lock").exists(), "not the plugin's own folder");
    drop(held);
    assert_eq!(ended(&ci, "site", n, Duration::from_secs(10)), BuildStatus::Succeeded);
    executors.stop();
    assert!(texts(&ci, "site", n).iter().any(|(_, t)| t == "dentro"));
}
