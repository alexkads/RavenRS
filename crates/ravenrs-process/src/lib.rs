//! Running a child program and watching it: the arguments apart (no shell), the folder, an
//! environment of only the variables let through, lines written to its input, each line of its
//! output and of its errors as an event as it comes, a time limit, and stopping it with every
//! process it started.
//!
//! ```no_run
//! use std::time::Duration;
//! use ravenrs_process::{Event, Spec};
//!
//! let running = Spec::new("cargo").arg("--version").timeout(Duration::from_secs(10)).spawn()?;
//! for event in running.events() {
//!     match event {
//!         Event::Out(line) => println!("{line}"),
//!         Event::Err(line) => eprintln!("{line}"),
//!         Event::Ended(end) => println!("ended: {end:?}"),
//!     }
//! }
//! # Ok::<(), ravenrs_process::ProcessError>(())
//! ```
//!
//! On Linux and macOS the child leads a process group of its own, so stopping it reaches what it
//! started: first a polite stop (`TERM`), then, after [`GRACE`], a forced one (`KILL`). On Windows
//! the child runs with no console window and `taskkill /T` stops the tree.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a child has, after the polite stop, before it is stopped by force.
pub const GRACE: Duration = Duration::from_secs(2);

/// How long a line can be before it is cut into lines of this length.
pub const LINE_LIMIT: usize = 64 * 1024;

/// The variables a child gets from this process unless [`Spec::inherit_env`] gives it all: what
/// programs need to find themselves and to write text, nothing private.
pub const BASIC_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TMPDIR",
    "TEMP",
    "TMP",
    "SYSTEMROOT",
    "COMSPEC",
    "PATHEXT",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
];

/// Why a child could not be run.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProcessError {
    /// The program is not where it was said to be, nor in `PATH`.
    NotFound(String),
    /// It could not start, with why.
    Start(String),
    /// It did not answer in time.
    TimedOut,
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(program) => write!(f, "{program} is not installed, or not in PATH"),
            Self::Start(why) => write!(f, "it could not start: {why}"),
            Self::TimedOut => write!(f, "it did not answer in time"),
        }
    }
}

impl std::error::Error for ProcessError {}

/// How a child ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// It ended on its own with this code; 0 is success.
    Code(i32),
    /// It was ended by a signal (Linux and macOS), with its number.
    Signal(i32),
    /// It passed its time limit and was stopped, with what it started.
    TimedOut,
    /// It was stopped by [`Control::stop`].
    Stopped,
}

impl End {
    /// Whether it ended on its own with success.
    pub fn success(self) -> bool {
        self == Self::Code(0)
    }
}

/// What a running child says, in the order it comes on each stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A line of its output.
    Out(String),
    /// A line of its errors.
    Err(String),
    /// It ended; nothing comes after this.
    Ended(End),
}

/// A child program to run.
#[derive(Clone, Debug)]
pub struct Spec {
    program: OsString,
    args: Vec<OsString>,
    dir: Option<PathBuf>,
    inherit: bool,
    allowed: Vec<String>,
    vars: BTreeMap<String, String>,
    stdin: bool,
    timeout: Option<Duration>,
    line_limit: usize,
}

impl Spec {
    /// The program, by its path or its name in `PATH`.
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            dir: None,
            inherit: false,
            allowed: Vec::new(),
            vars: BTreeMap::new(),
            stdin: false,
            timeout: None,
            line_limit: LINE_LIMIT,
        }
    }

    /// One argument, passed as it is: no shell reads it.
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Arguments, passed as they are.
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// The folder it runs in.
    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = Some(dir.into());
        self
    }

    /// Lets this variable of this process through, besides [`BASIC_ENV`].
    pub fn allow_env(mut self, name: impl Into<String>) -> Self {
        self.allowed.push(name.into());
        self
    }

    /// Gives it every variable of this process, as a terminal would.
    pub fn inherit_env(mut self) -> Self {
        self.inherit = true;
        self
    }

    /// Sets a variable for it, whatever this process has.
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.insert(name.into(), value.into());
        self
    }

    /// Keeps its input open, for [`Control::send`]; otherwise it reads nothing.
    pub fn stdin(mut self) -> Self {
        self.stdin = true;
        self
    }

    /// Stops it, and what it started, when it runs longer than `limit`.
    pub fn timeout(mut self, limit: Duration) -> Self {
        self.timeout = Some(limit);
        self
    }

    /// Cuts lines longer than `limit` bytes into lines of that length.
    pub fn line_limit(mut self, limit: usize) -> Self {
        self.line_limit = limit.max(1);
        self
    }

    /// The command, with the environment it gives.
    fn command(&self) -> Result<Command, ProcessError> {
        let program = resolve(&self.program)
            .ok_or_else(|| ProcessError::NotFound(self.program.to_string_lossy().into_owned()))?;
        let mut command = Command::new(program);
        command.args(&self.args);
        if let Some(dir) = &self.dir {
            command.current_dir(dir);
        }
        if !self.inherit {
            command.env_clear();
            for name in BASIC_ENV.iter().copied().chain(self.allowed.iter().map(String::as_str)) {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
        }
        command.envs(&self.vars);
        command
            .stdin(if self.stdin { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        alone(&mut command);
        Ok(command)
    }

    /// Starts it.
    pub fn spawn(self) -> Result<Running, ProcessError> {
        let mut child = self.command()?.spawn().map_err(|e| ProcessError::Start(e.to_string()))?;
        let pid = child.id();
        let (send, events) = channel();
        let control = Control {
            pid,
            stdin: Arc::new(Mutex::new(child.stdin.take())),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let readers: Vec<_> = [
            child.stdout.take().map(|s| read_lines(s, send.clone(), Event::Out, self.line_limit)),
            child.stderr.take().map(|s| read_lines(s, send.clone(), Event::Err, self.line_limit)),
        ]
        .into_iter()
        .flatten()
        .collect();
        let deadline = self.timeout.map(|limit| Instant::now() + limit);
        let watching = control.clone();
        std::thread::Builder::new()
            .name(format!("ravenrs-process-{pid}"))
            .spawn(move || watch(child, deadline, watching, readers, send))
            .map_err(|e| ProcessError::Start(e.to_string()))?;
        Ok(Running { events, control })
    }

    /// Runs it to the end and gives everything it said.
    pub fn output(self) -> Result<Output, ProcessError> {
        Ok(self.spawn()?.wait())
    }
}

/// Starts the child alone: a process group of its own, or with no console window.
#[cfg(unix)]
fn alone(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn alone(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(any(unix, windows)))]
fn alone(_: &mut Command) {}

/// Reads `stream` line by line into `send` as `wrap`, cutting long lines, until it closes.
fn read_lines(
    stream: impl Read + Send + 'static,
    send: Sender<Event>,
    wrap: fn(String) -> Event,
    limit: usize,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            while buffer.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
                buffer.pop();
            }
            for piece in buffer.chunks(limit) {
                if send.send(wrap(String::from_utf8_lossy(piece).into_owned())).is_err() {
                    return;
                }
            }
            if buffer.is_empty() && send.send(wrap(String::new())).is_err() {
                return;
            }
        }
    })
}

/// Waits for the child, stopping it at the deadline or when asked, then for its streams, and
/// says how it ended.
fn watch(
    mut child: Child,
    deadline: Option<Instant>,
    control: Control,
    readers: Vec<std::thread::JoinHandle<()>>,
    send: Sender<Event>,
) {
    let mut forced: Option<End> = None;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break None,
        }
        let late = deadline.is_some_and(|d| Instant::now() >= d);
        if forced.is_none() && (late || control.stop.load(Ordering::SeqCst)) {
            forced = Some(if late { End::TimedOut } else { End::Stopped });
            stop_tree(control.pid, &mut child);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // What it started is stopped too, even when it ended on its own leaving them behind.
    if forced.is_some() {
        stop_tree(control.pid, &mut child);
    }
    control.close();
    for reader in readers {
        let _ = reader.join();
    }
    // Asked to stop, it is stopped, even when closing its input let it end on its own first.
    let forced = forced.or_else(|| control.stop.load(Ordering::SeqCst).then_some(End::Stopped));
    let end = forced.unwrap_or_else(|| match status {
        Some(status) => match status.code() {
            Some(code) => End::Code(code),
            None => End::Signal(signal_of(&status)),
        },
        None => End::Code(-1),
    });
    let _ = send.send(Event::Ended(end));
}

#[cfg(unix)]
fn signal_of(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.signal().unwrap_or(-1)
}

#[cfg(not(unix))]
fn signal_of(_: &std::process::ExitStatus) -> i32 {
    -1
}

/// Stops the child and what it started: politely, then by force after [`GRACE`].
#[cfg(unix)]
fn stop_tree(pid: u32, child: &mut Child) {
    let group = format!("-{pid}");
    let signal = |name: &str| {
        let _ = Command::new("kill")
            .args([name, "--", &group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    signal("-TERM");
    let until = Instant::now() + GRACE;
    while Instant::now() < until {
        if matches!(child.try_wait(), Ok(Some(_))) && !group_alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    signal("-KILL");
    let _ = child.kill();
    let _ = child.wait();
}

/// Whether any process of the group `pid` still runs.
#[cfg(unix)]
fn group_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(windows)]
fn stop_tree(pid: u32, child: &mut Child) {
    let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).status();
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(any(unix, windows)))]
fn stop_tree(_: u32, child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// A child that runs: its events, and what stops it or writes to it.
#[derive(Debug)]
pub struct Running {
    events: Receiver<Event>,
    control: Control,
}

impl Running {
    /// Each event as it comes, ending with [`Event::Ended`].
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        let mut done = false;
        std::iter::from_fn(move || {
            if done {
                return None;
            }
            let event = self.events.recv().ok()?;
            done = matches!(event, Event::Ended(_));
            Some(event)
        })
    }

    /// The next event, waiting at most `within`; `None` when none came in time.
    pub fn next_within(&self, within: Duration) -> Option<Event> {
        self.events.recv_timeout(within).ok()
    }

    /// What stops it or writes to it, from any thread.
    pub fn control(&self) -> Control {
        self.control.clone()
    }

    /// Waits for it to end and gives everything it said.
    pub fn wait(self) -> Output {
        let mut output = Output { stdout: Vec::new(), stderr: Vec::new(), end: End::Code(-1) };
        for event in self.events() {
            match event {
                Event::Out(line) => output.stdout.push(line),
                Event::Err(line) => output.stderr.push(line),
                Event::Ended(end) => output.end = end,
            }
        }
        output
    }
}

/// Everything a child said, and how it ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// The lines of its output.
    pub stdout: Vec<String>,
    /// The lines of its errors.
    pub stderr: Vec<String>,
    /// How it ended.
    pub end: End,
}

/// Stops a running child or writes to it, from any thread.
#[derive(Clone, Debug)]
pub struct Control {
    pid: u32,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stop: Arc<AtomicBool>,
}

impl Control {
    /// The child's process id.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Writes a line to its input, as typed in a terminal. False when its input is closed.
    pub fn send(&self, line: &str) -> bool {
        let Ok(mut stdin) = self.stdin.lock() else { return false };
        let Some(stdin) = stdin.as_mut() else { return false };
        writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_ok()
    }

    /// Closes its input, so a program that reads to the end of it finishes.
    pub fn close(&self) {
        if let Ok(mut stdin) = self.stdin.lock() {
            stdin.take();
        }
    }

    /// Stops it and what it started: its input closes, then a polite stop, then a forced one.
    pub fn stop(&self) {
        self.close();
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Where `program` is: itself when it is a path that exists, otherwise the first match in
/// `PATH` (with the extensions of `PATHEXT` on Windows).
pub fn resolve(program: impl AsRef<std::ffi::OsStr>) -> Option<PathBuf> {
    let program = Path::new(program.as_ref());
    if program.components().count() > 1 || program.is_absolute() {
        return program.exists().then(|| program.to_path_buf());
    }
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(str::to_owned)
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        extensions.iter().find_map(|ext| {
            let mut name = program.as_os_str().to_owned();
            name.push(ext);
            let candidate = dir.join(name);
            candidate.is_file().then_some(candidate)
        })
    })
}

/// The first line `program` writes with `args` (as `--version`), waiting at most `within`.
pub fn version(
    program: impl Into<OsString>,
    args: &[&str],
    within: Duration,
) -> Result<String, ProcessError> {
    let running = Spec::new(program).args(args.iter().copied()).timeout(within).spawn()?;
    let output = running.wait();
    if output.end == End::TimedOut {
        return Err(ProcessError::TimedOut);
    }
    output
        .stdout
        .into_iter()
        .chain(output.stderr)
        .find(|l| !l.trim().is_empty())
        .ok_or(ProcessError::TimedOut)
}
