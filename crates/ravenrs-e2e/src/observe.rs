//! Watching a session as it runs: what a film of a test needs (`ravenrs-movie`'s `SessionFilm`),
//! and nothing a test that does not watch pays for.

use ravenrs_core::{Color, Point, Primitive, Size};

use crate::E2e;

/// Told about what a session draws and does, in order: each frame it draws, where the pointer
/// goes, each click and key, and each step a test marks. Every method but [`Self::frame`] does
/// nothing unless implemented.
pub trait FrameObserver {
    /// A frame the session drew, as its primitives.
    fn frame(&mut self, primitives: &[Primitive]);
    /// The pointer moved to `at`.
    fn pointer(&mut self, at: Point) {
        let _ = at;
    }
    /// The primary button went down at `at`.
    fn click(&mut self, at: Point) {
        let _ = at;
    }
    /// A key was pressed.
    fn key(&mut self) {}
    /// A test began the step called `title`, as [`E2e::mark_step`] says.
    fn step(&mut self, title: &str) {
        let _ = title;
    }
}

impl E2e {
    /// Tells `observer` about everything the session draws and does from now on, starting with
    /// the frame on the screen; it replaces any other.
    pub fn observe(&mut self, mut observer: Box<dyn FrameObserver>) {
        observer.frame(self.ui.scene().primitives());
        self.observer = Some(observer);
    }

    /// Stops telling the observer, and gives it back.
    pub fn stop_observing(&mut self) -> Option<Box<dyn FrameObserver>> {
        self.observer.take()
    }

    /// Marks the start of a step called `title`, for an observer: a film makes it a chapter.
    pub fn mark_step(&mut self, title: &str) {
        if let Some(observer) = &mut self.observer {
            observer.step(title);
        }
    }

    /// The window's size, in logical pixels.
    pub fn size(&self) -> Size {
        self.size
    }

    /// The color behind everything the session draws.
    pub fn background(&self) -> Color {
        self.ui.scene().clear_color()
    }

    pub(crate) fn observed_frame(&mut self) {
        if let Some(observer) = &mut self.observer {
            observer.frame(self.ui.scene().primitives());
        }
    }

    pub(crate) fn observed_pointer(&mut self, at: Point) {
        if let Some(observer) = &mut self.observer {
            observer.pointer(at);
        }
    }

    pub(crate) fn observed_click(&mut self, at: Point) {
        if let Some(observer) = &mut self.observer {
            observer.click(at);
        }
    }

    pub(crate) fn observed_key(&mut self) {
        if let Some(observer) = &mut self.observer {
            observer.key();
        }
    }
}

/// Input as raw as a platform's, for whoever scripts a session step by step, such as
/// `ravenrs-movie`'s `Recorder`, which paces a training as a person would: each call sends one
/// event, tells the observer and draws what follows, with no checks on what is under it.
impl E2e {
    /// Moves the pointer to `at`.
    pub fn pointer_to(&mut self, at: Point) {
        self.move_to(at);
    }

    /// Presses the primary button at `at`.
    pub fn mouse_down(&mut self, at: Point) {
        self.pointer = at;
        self.observed_click(at);
        self.send(&ravenrs_core::Event::PointerDown {
            position: at,
            button: ravenrs_core::PointerButton::Primary,
        });
    }

    /// Lets go of the primary button at `at`.
    pub fn mouse_up(&mut self, at: Point) {
        self.pointer = at;
        self.send(&ravenrs_core::Event::PointerUp {
            position: at,
            button: ravenrs_core::PointerButton::Primary,
        });
    }

    /// Presses `key` with `modifiers`, without letting it go.
    pub fn key_down(&mut self, key: ravenrs_core::Key, modifiers: ravenrs_core::Modifiers) {
        self.observed_key();
        self.send(&ravenrs_core::Event::KeyDown { key, modifiers });
    }

    /// Turns the wheel by `delta` over `at`.
    pub fn scroll_at(&mut self, at: Point, delta: Point) {
        self.send(&ravenrs_core::Event::Scroll { position: at, delta });
    }

    /// Gives the keyboard focus to the node `id` of the tree, as assistive technology does.
    pub fn focus(&mut self, id: u64) {
        self.ui.perform_action(id, ravenrs_core::AccessAction::Focus);
        self.frame();
    }

    /// Draws one frame at `seconds` on the session's clock, which only moves forward: what a
    /// script that keeps its own pace calls once per frame of its film.
    pub fn frame_at(&mut self, seconds: f64) {
        self.clock = self.clock.max(seconds);
        self.ui.set_time(self.clock);
        self.frame();
    }

    /// Every node `query` finds, in reading order.
    pub fn find_all(&self, query: &crate::Query) -> crate::E2eResult<Vec<ravenrs_core::AccessNode>> {
        Ok(self.candidates(query)?.into_iter().cloned().collect())
    }
}
