//! The app's side of the channel with its real window on screen (`--headed` in the tools).
//!
//! The app builds its view as usual, on its test data, and runs it with
//! `ravenrs_platform_native::run_driven(view, options, Headed::new(hello))`. The window's
//! platform lends its interface to the [`Headed`] driver on the main thread whenever a command
//! arrives, and the command runs through the same code as on the channel with no window, on the
//! interface the window shows. Standard input is read on a thread of its own, so the window keeps
//! drawing while the tool is quiet: between steps, during a pause or a slowed-down run.
//!
//! The interface's clock is the driver's ([`UiDriver::time`]): between two steps the app gets at
//! most [`SETTLE`] seconds, enough for what a step started (a menu opening, a page sliding) to
//! finish on screen, and then its time stands still until the next step. A slowed-down run or a
//! pause therefore never lets a notice expire, or a timer fire, that a run at full speed would
//! not: the same scenario passes or fails the same way at any pace, with or without the window.

use std::io::{self, BufRead, BufReader, Write};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

use ravenrs_e2e::E2e;
use ravenrs_widgets_base::{Driving, Ui, UiDriver};

use crate::serve::{End, Filming, answer, write_line};
use crate::{Failure, Hello, Reply, codes};

/// How long the app's clock runs after a step before it stands still until the next one, in
/// seconds: what the step started finishes on screen, and a pause adds nothing after it.
pub const SETTLE: f64 = 0.6;

/// A line from the tool, or the end of its input.
enum Line {
    Text(String),
    Closed,
}

/// Answers a tool on the interface of the app's real window. See the [module](self).
pub struct Headed {
    hello: Hello,
    session: E2e,
    filming: Option<Box<dyn Filming>>,
    input: Option<Box<dyn BufRead + Send>>,
    output: Box<dyn Write>,
    lines: Option<Receiver<Line>>,
    released: bool,
    /// How far the interface's clock is behind the real one: the time the app stood still.
    behind: f64,
    /// The interface's clock when the last step ended.
    last_step: Option<f64>,
}

impl std::fmt::Debug for Headed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Headed")
            .field("app", &self.hello.app)
            .field("released", &self.released)
            .finish_non_exhaustive()
    }
}

impl Headed {
    /// Answers over this process's standard input and output, starting with `hello` (which is
    /// sent with `window: true`) once the window is on screen. Nothing else may write to standard
    /// output meanwhile.
    pub fn new(hello: Hello) -> Self {
        Self::over(hello, BufReader::new(io::stdin()), io::stdout())
    }

    /// [`Self::new`] over any input and output, for tests and other transports.
    pub fn over(hello: Hello, input: impl BufRead + Send + 'static, output: impl Write + 'static) -> Self {
        Self {
            hello: Hello { window: true, ..hello },
            session: E2e::for_window(),
            filming: None,
            input: Some(Box::new(input)),
            output: Box::new(output),
            lines: None,
            released: false,
            behind: 0.0,
            last_step: None,
        }
    }

    /// Also films the window's interface when the tool asks, as `serve_filmed` does.
    pub fn filmed(mut self, filming: impl Filming + 'static) -> Self {
        self.filming = Some(Box::new(filming));
        self
    }
}

impl UiDriver for Headed {
    fn start(&mut self, wake: Box<dyn Fn() + Send + Sync>) {
        let (send, lines) = channel();
        if let Some(input) = self.input.take() {
            std::thread::spawn(move || {
                for line in input.lines() {
                    let Ok(line) = line else { break };
                    if send.send(Line::Text(line)).is_err() {
                        return;
                    }
                    wake();
                }
                let _ = send.send(Line::Closed);
                wake();
            });
        }
        self.lines = Some(lines);
        if write_line(&mut self.output, &self.hello).is_err() {
            // Nobody to answer: the tool is gone, and the first turn ends the app.
            self.lines = None;
        }
    }

    fn time(&mut self, now: f64) -> f64 {
        let clock = now - self.behind;
        match self.last_step {
            Some(ended) => clock.min(ended + SETTLE),
            None => clock,
        }
    }

    fn drive(&mut self, ui: &mut Ui, now: f64) -> Driving {
        // `now` is already the interface's clock; the real one goes on while steps run.
        let real = now + self.behind;
        let started = Instant::now();
        let Some(lines) = &self.lines else {
            return if self.released { Driving::Released } else { Driving::Done };
        };
        loop {
            let line = match lines.try_recv() {
                Ok(Line::Text(line)) => line,
                Err(TryRecvError::Empty) => break,
                // The tool let go of the window and closed its end: the window stays.
                Ok(Line::Closed) | Err(TryRecvError::Disconnected) if self.released => break,
                // The tool is gone without saying bye: nobody will drive the window again.
                Ok(Line::Closed) | Err(TryRecvError::Disconnected) => return Driving::Done,
            };
            if self.released {
                continue;
            }
            let real_now = real + started.elapsed().as_secs_f64();
            // The time the app stood still since the last step stays behind it.
            if let Some(ended) = self.last_step {
                self.behind = self.behind.max(real_now - (ended + SETTLE));
            }
            let Self { session, filming, .. } = self;
            let answered = session.in_window(ui, real_now - self.behind, |session| {
                answer(&line, session, filming.as_deref_mut())
            });
            // A step that waited moved the session's clock with real time; the interface's goes on
            // from there.
            let after = real + started.elapsed().as_secs_f64();
            self.behind = after - self.session.clock();
            self.last_step = Some(self.session.clock());
            let Some((reply, end)) = answered else { continue };
            if write_line(&mut self.output, &reply).is_err() {
                return Driving::Done;
            }
            match end {
                Some(End::Bye) => return Driving::Done,
                Some(End::Release) => self.released = true,
                None => {}
            }
        }
        if self.released { Driving::Released } else { Driving::Driven }
    }

    fn window_closed(&mut self) {
        let failure = Failure::of(codes::WINDOW_CLOSED, "the window was closed while the test drove it");
        let _ = write_line(&mut self.output, &Reply::of(Err(failure)));
    }
}
