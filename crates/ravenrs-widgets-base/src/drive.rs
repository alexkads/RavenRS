//! Driving a window's interface from outside: what a test tool does when it runs a scenario on
//! the app's real window (`--headed` in Raven Test Studio and `cargo ravenrs e2e run`).
//!
//! A platform that shows a window owns its [`Ui`], on the main thread. A [`UiDriver`] gets that
//! same `Ui` lent to it on that thread whenever it asks for a turn, so the steps of a test act on
//! the interface the person sees, and nothing is copied or run twice. Commands usually come from
//! another thread (the test channel reads its input there): the driver keeps them, calls the wake
//! function it was given, and runs them on its next turn.

use crate::Ui;

/// Who is in control of the window after a driver's turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driving {
    /// The driver: the window still draws, resizes and animates, but the person's pointer and
    /// keyboard do not reach the interface, so they cannot change the test halfway.
    Driven,
    /// The person: the driver let go (a test that ended and left its window to be looked at), and
    /// the window is an ordinary one until the person closes it.
    Released,
    /// Nobody: the driver is done, and the window closes and the app ends.
    Done,
}

/// Drives a window's [`Ui`] from outside, on the platform's thread. See the [module](self).
pub trait UiDriver {
    /// Called once, when the window is on screen: `wake` asks the platform for a turn, from any
    /// thread, and a turn follows at once, so the driver may answer that it is ready here.
    fn start(&mut self, wake: Box<dyn Fn() + Send + Sync>);

    /// A turn: runs what the driver has waiting on `ui`, whose clock reads `now` seconds, and says
    /// who is in control next. The platform draws the frame after it.
    fn drive(&mut self, ui: &mut Ui, now: f64) -> Driving;

    /// The interface's clock while the driver is in control, given the real one (`now`, seconds
    /// since the app started): the real one by default. A test tool holds it between its steps,
    /// so a pause to watch the test does not let a notice expire that the next step checks.
    fn time(&mut self, now: f64) -> f64 {
        now
    }

    /// The person closed the window while the driver was in control; the app ends after this
    /// returns. A test tool says so to whoever is waiting for a step.
    fn window_closed(&mut self) {}
}
