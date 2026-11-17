//! The tool's side of the channel: the app as a child process.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command as Process, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{Command, Failure, Hello, PROTOCOL, Reply, VERSION, codes};

/// How long a step may take before the app is taken as stuck.
const STEP_TIME: Duration = Duration::from_secs(30);
/// Lines of the app's error output a failure keeps.
const ERROR_LINES: usize = 12;

/// An app opened in a child process with no window, driven over [the channel](crate). The
/// child ends when the client is dropped.
#[derive(Debug)]
pub struct Client {
    child: Child,
    input: Option<ChildStdin>,
    lines: Receiver<String>,
    errors: Arc<Mutex<Vec<String>>>,
    app: String,
    scenarios: Vec<String>,
    step_time: Duration,
}

impl Client {
    /// Starts `process` (the app with [`crate::SERVE_FLAG`] among its arguments) and waits up to
    /// `start_time` for its hello: a build with `cargo run` needs minutes, an installed app a
    /// moment.
    pub fn spawn(mut process: Process, start_time: Duration) -> Result<Self, Failure> {
        let mut child =
            process
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| Failure::of(codes::APP_START, format!("could not start the app: {e}")))?;
        let (send, lines) = channel();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    // Only the channel's lines: anything else the app prints is not an answer.
                    if line.trim_start().starts_with('{') && send.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        let errors = Arc::new(Mutex::new(Vec::new()));
        if let Some(err) = child.stderr.take() {
            let kept = Arc::clone(&errors);
            std::thread::spawn(move || keep_tail(err, &kept));
        }
        let mut client = Self {
            input: child.stdin.take(),
            child,
            lines,
            errors,
            app: String::new(),
            scenarios: Vec::new(),
            step_time: STEP_TIME,
        };
        let hello = client.next_line(start_time, codes::APP_START)?;
        let hello: Hello = serde_json::from_str(&hello)
            .map_err(|e| client.failure(codes::APP_START, format!("the app did not say hello: {e}")))?;
        if hello.protocol != PROTOCOL || hello.version != VERSION {
            return Err(client.failure(
                codes::WIRE,
                format!(
                    "the app speaks {} {}, this tool {PROTOCOL} {VERSION}",
                    hello.protocol, hello.version
                ),
            ));
        }
        client.app = hello.app;
        client.scenarios = hello.scenarios;
        Ok(client)
    }

    /// The app's name, from its hello.
    pub fn app(&self) -> &str {
        &self.app
    }

    /// The scenarios the app brings, as the text of its `.raventest` files; read each with
    /// [`crate::Scenario::parse`].
    pub fn scenarios(&self) -> &[String] {
        &self.scenarios
    }

    /// Gives each step up to `time` before the app is taken as stuck; 30 s by default.
    pub fn step_time(mut self, time: Duration) -> Self {
        self.step_time = time;
        self
    }

    /// Sends `command` and waits for the app's answer, as [`Command::apply`] returns it.
    pub fn send(&mut self, command: &Command) -> Result<Option<String>, Failure> {
        self.exchange(command)?.into_result()
    }

    /// The app's current vector frame, encoded as a single-frame `.ravenmovie`. The app must
    /// provide frames through its `Filming` adapter. Decode with `Movie::from_bytes`
    /// in `ravenrs-movie`; this crate has no dependency on the movie engine.
    pub fn frame(&mut self) -> Result<Vec<u8>, Failure> {
        self.exchange(&Command::Frame)?.into_frame()
    }

    fn exchange(&mut self, command: &Command) -> Result<Reply, Failure> {
        let line = serde_json::to_string(command).map_err(|e| Failure::of(codes::WIRE, e.to_string()))?;
        let written = match self.input.as_mut() {
            Some(input) => writeln!(input, "{line}").and_then(|()| input.flush()).is_ok(),
            None => false,
        };
        if !written {
            return Err(self.failure(codes::APP_START, "the app is no longer running"));
        }
        let answer = self.next_line(self.step_time, codes::TIMEOUT)?;
        serde_json::from_str(&answer)
            .map_err(|e| Failure::of(codes::WIRE, format!("not an answer of this channel: {e}")))
    }

    /// The next line of the channel, or why there is none: the app ended, or took too long.
    fn next_line(&mut self, time: Duration, late: &str) -> Result<String, Failure> {
        match self.lines.recv_timeout(time) {
            Ok(line) => Ok(line),
            Err(RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                Err(self
                    .failure(late, format!("the app did not answer in {} s and was ended", time.as_secs())))
            }
            Err(RecvTimeoutError::Disconnected) => {
                let status = self.child.wait().map(|s| s.to_string()).unwrap_or_default();
                Err(self.failure(codes::APP_START, format!("the app ended ({status})")))
            }
        }
    }

    /// A failure with the last lines the app wrote to its error output, which say why.
    fn failure(&self, code: &str, message: impl Into<String>) -> Failure {
        let tail = self.errors.lock().map(|lines| lines.join("\n")).unwrap_or_default();
        Failure { screen: tail, ..Failure::of(code, message) }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(mut input) = self.input.take()
            && let Ok(bye) = serde_json::to_string(&Command::Bye)
        {
            let _ = writeln!(input, "{bye}");
        }
        // A moment to leave on its own, then the child goes either way.
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn keep_tail(err: impl Read, kept: &Mutex<Vec<String>>) {
    for line in BufReader::new(err).lines().map_while(Result::ok) {
        if let Ok(mut lines) = kept.lock() {
            lines.push(line);
            let extra = lines.len().saturating_sub(ERROR_LINES);
            lines.drain(..extra);
        }
    }
}
