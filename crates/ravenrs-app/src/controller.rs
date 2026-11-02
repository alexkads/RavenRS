//! What gives an application its behavior: controllers bring actions and react to views.

use crate::action::TargetScope;
use crate::frame::FrameContext;
use crate::{Action, Frame};

/// Behavior tied to a view: it turns on when a view it targets is shown, and off when that goes.
/// Its actions are off while it is off. Each frame has its own.
pub trait ViewController {
    /// What it is known by.
    fn id(&self) -> &str;

    /// Which views it is for. Every one by default.
    fn scope(&self) -> TargetScope {
        TargetScope::any()
    }

    /// The actions it brings. Asked once.
    fn actions(&self) -> Vec<Action> {
        Vec::new()
    }

    /// A view it targets is in place.
    fn activated(&mut self, _frame: &Frame) {}

    /// The view is going, or something turned it off.
    fn deactivated(&mut self, _frame: &Frame) {}

    /// The view's controls exist now.
    fn view_controls_created(&mut self, _frame: &Frame) {}
}

/// Behavior tied to a window, whatever view it shows. Stays on while the frame lasts.
pub trait WindowController {
    /// What it is known by.
    fn id(&self) -> &str;

    /// The kinds of window it is for. Only the main one by default.
    fn contexts(&self) -> &[FrameContext] {
        &[FrameContext::Main]
    }

    /// The actions it brings. Asked once.
    fn actions(&self) -> Vec<Action> {
        Vec::new()
    }

    /// The frame is ready.
    fn activated(&mut self, _frame: &Frame) {}

    /// The frame is closing.
    fn deactivated(&mut self, _frame: &Frame) {}
}
