//! Waiting: a clock the session drives, and steps that try again until the screen is ready.

use std::time::{Duration, Instant};

use crate::{E2e, E2eResult, Locator};

/// How far one frame moves the clock in [`E2e::advance`], as at 60 frames a second.
const FRAME: f64 = 1.0 / 60.0;
/// How long a wait sleeps between looks at the screen.
const POLL: Duration = Duration::from_millis(10);

impl E2e {
    /// The session's clock, in seconds since it started. It moves only when the session moves
    /// it: with [`Self::advance`], and while a step waits.
    pub fn clock(&self) -> f64 {
        self.clock
    }

    /// Moves the clock `seconds` forward one frame at a time, as a platform would: animations
    /// run to where they would be by then, and timers that come due fire. Without it, the
    /// session never starts an animation, and what a screen shows "after a moment" never shows.
    pub fn advance(&mut self, seconds: f64) {
        let end = self.clock + seconds.max(0.0);
        while self.clock < end {
            self.clock = (self.clock + FRAME).min(end);
            self.ui.set_time(self.clock);
            self.ui.update();
        }
        self.frame();
    }

    /// Looks at the screen until `check` passes, moving the clock with real time between looks,
    /// so timers fire and work done in the background (which wakes the screen) shows. When
    /// `timeout` runs out, it fails with [`ErrorCode::Timeout`](crate::ErrorCode::Timeout) and
    /// what the last look found.
    pub fn wait_until(
        &mut self,
        timeout: Duration,
        mut check: impl FnMut(&mut Self) -> E2eResult,
    ) -> E2eResult {
        self.looking(timeout, &mut check, true)
    }

    /// Like [`Self::wait_until`], but only for a failure waiting may still fix
    /// ([`E2eError::may_pass_later`](crate::E2eError::may_pass_later)), and when time runs out
    /// it fails as the step itself failed, with the same code. It is how a tool runs a step
    /// without making the person who writes it think about time.
    pub fn patiently(
        &mut self,
        patience: Duration,
        mut step: impl FnMut(&mut Self) -> E2eResult,
    ) -> E2eResult {
        self.looking(patience, &mut step, false)
    }

    fn looking(
        &mut self,
        timeout: Duration,
        check: &mut dyn FnMut(&mut Self) -> E2eResult,
        as_timeout: bool,
    ) -> E2eResult {
        let start = Instant::now();
        let mut last = start;
        loop {
            let failure = match check(self) {
                Ok(()) => return Ok(()),
                Err(e) => e,
            };
            if !as_timeout && !failure.may_pass_later() {
                return Err(failure);
            }
            if start.elapsed() >= timeout {
                return Err(if as_timeout { failure.timed_out(start.elapsed()) } else { failure });
            }
            std::thread::sleep(POLL);
            let now = Instant::now();
            self.advance((now - last).as_secs_f64());
            last = now;
        }
    }
}

impl Locator<'_> {
    /// Waits until the query finds its node, up to `timeout`; see [`E2e::wait_until`].
    pub fn wait_for(self, timeout: Duration) -> E2eResult {
        let query = self.query;
        self.e2e.wait_until(timeout, |e| e.find(&query).map(drop))
    }

    /// Waits until the node mentions `text` in its name or description, up to `timeout`.
    pub fn wait_to_mention(self, text: &str, timeout: Duration) -> E2eResult {
        let query = self.query;
        self.e2e.wait_until(timeout, |e| e.locate(query.clone()).expect().to_mention(text))
    }
}
