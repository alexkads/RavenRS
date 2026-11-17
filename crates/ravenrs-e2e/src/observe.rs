//! Watching a session as it runs: what a film of a test needs (`ravenrs-movie`'s `TestFilm`),
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
        observer.pointer(self.pointer);
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
