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
//!
//! A child that ends on its own may leave behind what it started in the background (`sleep 600 &`,
//! a daemon such as the server of `sccache`): they stay in its group. [`stop_group`] stops them,
//! as a step of a build does when it ends, so nothing of a build outlives it.

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
    /// This system cannot do what was asked, with what.
    Unsupported(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(program) => write!(f, "{program} is not installed, or not in PATH"),
            Self::Start(why) => write!(f, "it could not start: {why}"),
            Self::TimedOut => write!(f, "it did not answer in time"),
            Self::Unsupported(what) => write!(f, "this system cannot {what}"),
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
    user: Option<String>,
    leftovers: bool,
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
            leftovers: false,
            timeout: None,
            line_limit: LINE_LIMIT,
            user: None,
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

    /// Runs it as the user `name`, so it reads and writes only what that user may: a build agent
    /// that must not read the server's files.
    ///
    /// On Linux and macOS, a process of root switches the child's user and group itself (the ids
    /// come from `id`). Anyone else runs it through `sudo -n -u NAME --preserve-env=…`, which fails
    /// at once, saying so, when sudo has no rule letting this user run the program as `NAME`
    /// without a password; the rule needs `SETENV` for the variables of [`env`](Self::env), as
    /// `raven ALL=(ci) NOPASSWD:SETENV: /usr/local/bin/raven-ci-agent`. On Windows it is refused
    /// with [`ProcessError::Unsupported`]: running as another user there needs that user's password.
    pub fn user(mut self, name: impl Into<String>) -> Self {
        self.user = Some(name.into());
        self
    }

    /// Stops what it leaves running in its group the moment it ends on its own ([`stop_group`]),
    /// before waiting for its streams: a `sleep 600 &` holds the output open, and without this the
    /// child would seem to run until the sleep ends. What was stopped is in
    /// [`Control::leftovers`]. A step of a build asks for it, so nothing of a build outlives it.
    pub fn stop_leftovers(mut self) -> Self {
        self.leftovers = true;
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
        let mut command = match &self.user {
            None => {
                let mut command = Command::new(program);
                command.args(&self.args);
                command
            }
            Some(user) => as_user(user, program, &self.args, &self.vars)?,
        };
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
            leftovers: self.leftovers.then(|| Arc::new(Mutex::new(Vec::new()))),
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

/// The command that runs `program` with `args` as `user`. See [`Spec::user`].
#[cfg(unix)]
fn as_user(
    user: &str,
    program: PathBuf,
    args: &[OsString],
    vars: &BTreeMap<String, String>,
) -> Result<Command, ProcessError> {
    use std::os::unix::process::CommandExt;
    let id = |flag: &str, of: Option<&str>| -> Result<u32, ProcessError> {
        let mut asked = Command::new("id");
        asked.arg(flag);
        if let Some(of) = of {
            asked.arg(of);
        }
        let out = asked.output().map_err(|e| ProcessError::Start(format!("id {flag}: {e}")))?;
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .map_err(|_| ProcessError::Start(format!("there is no user {}", of.unwrap_or("running"))))
    };
    if id("-u", None)? == 0 {
        let (uid, gid) = (id("-u", Some(user))?, id("-g", Some(user))?);
        let mut command = Command::new(program);
        command.args(args).uid(uid).gid(gid);
        return Ok(command);
    }
    let sudo = resolve("sudo").ok_or_else(|| ProcessError::NotFound("sudo".to_owned()))?;
    let mut command = Command::new(sudo);
    command.args(["-n", "-u", user]);
    if !vars.is_empty() {
        let names: Vec<&str> = vars.keys().map(String::as_str).collect();
        command.arg(format!("--preserve-env={}", names.join(",")));
    }
    command.arg("--").arg(program).args(args);
    Ok(command)
}

#[cfg(not(unix))]
fn as_user(
    user: &str,
    _: PathBuf,
    _: &[OsString],
    _: &BTreeMap<String, String>,
) -> Result<Command, ProcessError> {
    Err(ProcessError::Unsupported(format!("run a program as the user {user} without their password")))
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
    } else if let Some(left) = &control.leftovers {
        let stopped = stop_group(control.pid);
        if let Ok(mut left) = left.lock() {
            *left = stopped;
        }
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

/// Stops what is left of the process group of `pid`, the child of a [`Running`] that ended: what it
/// started in the background and never waited for. A polite stop (`TERM`), then, after [`GRACE`],
/// a forced one (`KILL`). It gives the names of the processes it found, one per process (none when
/// the group had ended), so whoever asked can say what was left.
///
/// A process that left the group on purpose (`setsid`, as a daemon that detaches does) is not in
/// it and stays. On Windows, where the tree of a child ended on its own is not kept, it does
/// nothing and gives none.
#[cfg(unix)]
pub fn stop_group(pid: u32) -> Vec<String> {
    if !group_alive(pid) {
        return Vec::new();
    }
    let listed = Command::new("pgrep")
        .args(["-l", "-g", &pid.to_string()])
        .stderr(Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    let mut names: Vec<String> = listed
        .lines()
        .filter_map(|line| line.split_once(' ').map(|(_, name)| name.trim().to_owned()))
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        names.push("?".to_owned());
    }
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
    while Instant::now() < until && group_alive(pid) {
        std::thread::sleep(Duration::from_millis(20));
    }
    if group_alive(pid) {
        signal("-KILL");
    }
    names
}

/// On Windows the tree of a child that ended is not kept: nothing to stop.
#[cfg(not(unix))]
pub fn stop_group(_pid: u32) -> Vec<String> {
    Vec::new()
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

    /// Stops it as [`Control::stop`] does, and waits at most `within` for it and what it started
    /// to end: how it ended, or `None` when it had not ended in time.
    ///
    /// [`Control::stop`] only asks: a thread of this crate sends the polite stop and, after
    /// [`GRACE`], the forced one. A program that ends right after asking (a test, a server that
    /// stops) takes that thread with it, and the child is left running on its own. Waiting here
    /// keeps the asker alive until the child is gone. Give `within` more than [`GRACE`], so the
    /// forced stop has its time.
    pub fn stop_and_wait(&self, within: Duration) -> Option<End> {
        self.control.stop();
        let until = Instant::now() + within;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            match self.events.recv_timeout(left) {
                Ok(Event::Ended(end)) => return Some(end),
                Ok(_) => {}
                Err(_) => return None,
            }
        }
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
    leftovers: Option<Arc<Mutex<Vec<String>>>>,
}

impl Control {
    /// The child's process id.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// With [`Spec::stop_leftovers`], once it ended: the names of what it had left running in its
    /// group and was stopped, one per process. None otherwise.
    pub fn leftovers(&self) -> Vec<String> {
        self.leftovers.as_ref().and_then(|left| left.lock().ok().map(|left| left.clone())).unwrap_or_default()
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

/// What a process may hold: the most memory, and how many cores' worth of time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Limits {
    /// The most memory, in bytes.
    pub memory_bytes: Option<u64>,
    /// How many cores' worth of time it gets: `2.0` is two cores, `0.5` half of one.
    pub cores: Option<f64>,
}

/// A process put in a cgroup of its own by [`confine`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confined {
    /// The folder of its cgroup, under `/sys/fs/cgroup`.
    pub cgroup: PathBuf,
}

/// Puts the process `pid` (and what it starts from then on) in a cgroup of its own, `name`, under
/// the one it is in, with `limits`: `memory.max` and `cpu.max` of cgroup v2, which the system holds
/// whatever the process does. What a build agent does to itself.
///
/// It needs the cgroup it is in delegated to its user (a systemd service with `Delegate=yes`) or
/// root; without that it fails saying which file it could not write, and nothing is changed but
/// an empty folder. Only Linux has cgroups: elsewhere it is [`ProcessError::Unsupported`].
#[cfg(target_os = "linux")]
pub fn confine(pid: u32, name: &str, limits: &Limits) -> Result<Confined, ProcessError> {
    let failed = |what: &Path, e: std::io::Error| ProcessError::Start(format!("{}: {e}", what.display()));
    let own = std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .map_err(|e| ProcessError::Start(e.to_string()))?;
    let relative = own
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or_else(|| ProcessError::Unsupported("find a cgroup v2 (this system has only v1)".to_owned()))?;
    let base = Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/'));
    let cgroup = if base.file_name().is_some_and(|n| n == name) { base.clone() } else { base.join(name) };
    std::fs::create_dir_all(&cgroup).map_err(|e| failed(&cgroup, e))?;
    let procs = cgroup.join("cgroup.procs");
    std::fs::write(&procs, pid.to_string()).map_err(|e| failed(&procs, e))?;
    if cgroup != base {
        // A cgroup with processes cannot hand controllers down; the process left it just now.
        let control = base.join("cgroup.subtree_control");
        let _ = std::fs::write(&control, "+memory +cpu");
    }
    if let Some(bytes) = limits.memory_bytes {
        let file = cgroup.join("memory.max");
        std::fs::write(&file, bytes.to_string()).map_err(|e| failed(&file, e))?;
    }
    if let Some(cores) = limits.cores {
        let file = cgroup.join("cpu.max");
        let quota = (cores * 100_000.0).round().max(1_000.0) as u64;
        std::fs::write(&file, format!("{quota} 100000")).map_err(|e| failed(&file, e))?;
    }
    Ok(Confined { cgroup })
}

/// Only Linux has cgroups: see the Linux version.
#[cfg(not(target_os = "linux"))]
pub fn confine(_pid: u32, _name: &str, _limits: &Limits) -> Result<Confined, ProcessError> {
    Err(ProcessError::Unsupported(
        "hold a limit of memory and cores by cgroup, which only Linux has".to_owned(),
    ))
}
