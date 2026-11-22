//! Running with the app's real window on screen (`--headed`, #729): the mode, the pace a person
//! can follow and the window kept open to look at a failure.
//!
//! A run with no window is the default, for build servers and agents. With the window, the
//! scenario, its blocks, their waits and their codes are the same; the app is opened with
//! `--e2e-serve --e2e-window`, shows its window on its test data, and each step lands on the
//! interface the window shows (`ravenrs_e2e_wire::Headed`). On top of that:
//!
//! - **slow-mo** waits a moment after each step, in the tool, while the app is idle and its
//!   window keeps drawing. It is not a step and has no timeout, so it never makes a step fail;
//!   the app's clock keeps running, as it does for a person.
//! - **[`Control`]** pauses before the next block, runs exactly one more, or stops the run; the
//!   window keeps drawing all along, since the app only waits for its next command.
//! - **[`KeepOpen`]** leaves the window to the person at the end, to look at how the app was
//!   left; the run waits until they close it, and no process is left behind.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Whether the app shows its window during a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowMode {
    /// No window: the default, for build servers and agents.
    #[default]
    Headless,
    /// The app's real window, on screen.
    Headed,
}

impl WindowMode {
    /// The words `e2e.toml` takes for `window`.
    pub const WORDS: [&'static str; 2] = ["headless", "headed"];

    /// The mode a word says, in English or in Portuguese.
    pub fn read(word: &str) -> Option<Self> {
        Some(match word {
            "headless" | "sem-janela" => Self::Headless,
            "headed" | "com-janela" => Self::Headed,
            _ => return None,
        })
    }

    /// The word for the mode, as reports write it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Headless => "headless",
            Self::Headed => "headed",
        }
    }
}

/// When the window is left open at the end of a run, for the person to look at it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeepOpen {
    /// The window closes with the run: the default, so nothing waits on a person.
    #[default]
    Never,
    /// The window stays when a block failed, showing the screen as the failure left it.
    OnFailure,
    /// The window always stays.
    Always,
}

impl KeepOpen {
    /// The words `--keep-open` and `e2e.toml` take.
    pub const WORDS: [&'static str; 3] = ["never", "on-failure", "always"];

    /// The choice a word says, in English or in Portuguese.
    pub fn read(word: &str) -> Option<Self> {
        Some(match word {
            "never" | "nunca" => Self::Never,
            "on-failure" | "na-falha" => Self::OnFailure,
            "always" | "sempre" => Self::Always,
            _ => return None,
        })
    }

    /// Whether a run that `failed` leaves its window open.
    pub fn keeps(self, failed: bool) -> bool {
        match self {
            Self::Never => false,
            Self::OnFailure => failed,
            Self::Always => true,
        }
    }
}

/// How a run shows the app: with or without its window, at what pace, and whether the window
/// stays at the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window {
    /// With or without the window.
    pub mode: WindowMode,
    /// How long to wait after each step, for a person to follow it; only with the window.
    pub slow_mo: Duration,
    /// Whether the window stays at the end; only with the window.
    pub keep_open: KeepOpen,
}

impl Window {
    /// Whether the app shows its window.
    pub fn headed(&self) -> bool {
        self.mode == WindowMode::Headed
    }

    /// Why these choices make no sense together, said to whoever wrote them: a pace or a window
    /// kept open are only for a run with the window.
    pub fn check(&self) -> Result<(), String> {
        if self.headed() {
            return Ok(());
        }
        if !self.slow_mo.is_zero() {
            return Err("--slow-mo só vale com a janela: acrescente --headed".to_owned());
        }
        if self.keep_open != KeepOpen::Never {
            return Err("--keep-open só vale com a janela: acrescente --headed".to_owned());
        }
        Ok(())
    }
}

/// Whether this computer can show a window, or why not. Linux needs a graphical session
/// (`DISPLAY` or `WAYLAND_DISPLAY`), which a server or an SSH session usually lacks; macOS and
/// Windows are taken as able, and an app that still cannot open its window says why when it
/// ends.
pub fn graphic_session() -> Result<(), String> {
    graphic_session_in(|name| std::env::var_os(name).is_some_and(|v| !v.is_empty()))
}

/// [`graphic_session`] with the environment read through `set`, for tests.
pub fn graphic_session_in(set: impl Fn(&str) -> bool) -> Result<(), String> {
    if cfg!(any(target_os = "macos", target_os = "windows")) || set("DISPLAY") || set("WAYLAND_DISPLAY") {
        return Ok(());
    }
    Err("este computador não tem sessão gráfica (nem DISPLAY nem WAYLAND_DISPLAY): rode sem --headed, ou numa sessão com tela".to_owned())
}

/// The systems this build runs on and the ones it does not, for what a report could not check.
pub fn other_systems() -> &'static str {
    if cfg!(target_os = "macos") {
        "Linux e Windows não rodaram aqui"
    } else if cfg!(target_os = "windows") {
        "macOS e Linux não rodaram aqui"
    } else {
        "macOS e Windows não rodaram aqui"
    }
}

/// The system this run ran on, as reports write it.
pub fn this_system() -> &'static str {
    std::env::consts::OS
}

/// The window options as a command line writes them (`--headed`, `--headless`, `--slow-mo`,
/// `--keep-open`, `--step`), the same for Raven Test Studio and `cargo ravenrs e2e run`; read
/// them with [`crate::config::Config::window_from`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ask {
    /// `--headed`: show the app's real window.
    pub headed: bool,
    /// `--headless`: no window, even when `e2e.toml` asks for it.
    pub headless: bool,
    /// `--slow-mo <ms>`: milliseconds to wait after each step.
    pub slow_mo: Option<String>,
    /// `--keep-open <never|on-failure|always>`.
    pub keep_open: Option<String>,
    /// `--step`: start paused, one block per Enter.
    pub step: bool,
}

/// What a person watching a run asked for, shared between the run and whoever reads them (a
/// terminal, a window): pause, one more block, carry on, stop, and the pace.
#[derive(Clone, Default)]
pub struct Control(Arc<(Mutex<Pace>, Condvar)>);

#[derive(Debug, Default)]
struct Pace {
    paused: bool,
    /// Blocks allowed to run while paused.
    steps: u32,
    stopped: bool,
    slow_mo: Duration,
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Control").field(&*self.lock()).finish()
    }
}

impl PartialEq for Control {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Control {}

impl Control {
    /// Control of a run that waits `slow_mo` after each step and starts running.
    pub fn new(slow_mo: Duration) -> Self {
        let control = Self::default();
        control.set_slow_mo(slow_mo);
        control
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Pace> {
        self.0.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn changed(&self, change: impl FnOnce(&mut Pace)) {
        change(&mut self.lock());
        self.0.1.notify_all();
    }

    /// Pauses before the next block; the one running finishes.
    pub fn pause(&self) {
        self.changed(|p| p.paused = true);
    }

    /// Carries on with every block left.
    pub fn resume(&self) {
        self.changed(|p| {
            p.paused = false;
            p.steps = 0;
        });
    }

    /// Runs exactly one more block, then stays paused.
    pub fn next(&self) {
        self.changed(|p| {
            p.paused = true;
            p.steps += 1;
        });
    }

    /// Stops the run before the next block; the blocks left are reported as not run.
    pub fn stop(&self) {
        self.changed(|p| p.stopped = true);
    }

    /// Changes how long to wait after each step, even halfway through a run.
    pub fn set_slow_mo(&self, slow_mo: Duration) {
        self.changed(|p| p.slow_mo = slow_mo);
    }

    /// Whether the run is paused.
    pub fn is_paused(&self) -> bool {
        self.lock().paused
    }

    /// Whether the run was stopped.
    pub fn is_stopped(&self) -> bool {
        self.lock().stopped
    }

    /// Waits for the turn of the next block: `true` when it may run, `false` when the run was
    /// stopped. While paused it waits for [`Self::next`], [`Self::resume`] or [`Self::stop`].
    pub fn turn(&self) -> bool {
        let (lock, changed) = &*self.0;
        let mut pace = lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if pace.stopped {
                return false;
            }
            if !pace.paused {
                return true;
            }
            if pace.steps > 0 {
                pace.steps -= 1;
                return true;
            }
            pace = changed.wait(pace).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Waits the slow-mo after a step, cut short when the run is stopped or paused (a pause
    /// already waits for the person).
    pub fn present(&self) {
        let (lock, changed) = &*self.0;
        let pace = lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let slow_mo = pace.slow_mo;
        if slow_mo.is_zero() {
            return;
        }
        let _ = changed.wait_timeout_while(pace, slow_mo, |p| !p.stopped && !p.paused);
    }
}

/// What a person types in the terminal to steer a run with the window, one word per line, as
/// `--step` reads them: Enter runs one more block, `c` carries on, `p` pauses, `q` stops.
pub fn steer(control: &Control, line: &str) -> Option<&'static str> {
    Some(match line.trim() {
        "" | "n" | "proximo" | "próximo" => {
            control.next();
            "próximo passo"
        }
        "c" | "continuar" => {
            control.resume();
            "continuando"
        }
        "p" | "pausar" => {
            control.pause();
            "pausado"
        }
        "q" | "parar" => {
            control.stop();
            "parado"
        }
        _ => return None,
    })
}

/// What the terminal says a person can type while a run with the window goes.
pub const KEYS: &str = "Enter = próximo passo, c = continuar, p = pausar, q = parar";

/// Lets the person at the terminal steer a run with the window, when standard input is a
/// terminal (a pipe is never read): [`steer_from_terminal`], after saying how.
pub fn steer_when_terminal(control: &Control) {
    use std::io::IsTerminal as _;
    if !std::io::stdin().is_terminal() {
        return;
    }
    if control.is_paused() {
        eprintln!("[e2e] pausado antes do primeiro passo: {KEYS}");
    } else {
        eprintln!("[e2e] com a janela do app: {KEYS}");
    }
    steer_from_terminal(control.clone());
}

/// Reads the terminal on a thread of its own and steers `control` with each line ([`steer`]),
/// telling on standard error what each did, until the input ends.
pub fn steer_from_terminal(control: Control) {
    std::thread::spawn(move || {
        use std::io::BufRead as _;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            match steer(&control, &line) {
                Some(done) => eprintln!("[e2e] {done}"),
                None => eprintln!("[e2e] {KEYS}"),
            }
        }
    });
}
