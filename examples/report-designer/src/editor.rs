//! The designer's shared state: the report being edited, the selection,
//! undo/redo and the engine.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_report::{Control, Report, ReportEngine};

use crate::bands::BandRef;

/// What is selected on the design surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selection {
    None,
    Band(BandRef),
    Control(BandRef, usize),
}

/// A field being dragged from the field list.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDrag {
    pub path: String,
    pub start: Point,
    pub at: Point,
}

/// Everything panels share. Cheap to clone.
#[derive(Clone)]
pub struct Editor {
    pub report: State<Report>,
    pub selection: State<Selection>,
    pub tab: State<usize>,
    pub zoom: State<f32>,
    pub version: State<u64>,
    pub page: State<usize>,
    pub gallery_open: State<bool>,
    pub wizard_open: State<bool>,
    pub drag: State<Option<FieldDrag>>,
    pub engine: Rc<RefCell<ReportEngine>>,
    /// Frames of the band bodies in the last paint, for drops.
    pub band_frames: Rc<RefCell<Vec<(BandRef, Rect)>>>,
    history: Rc<RefCell<(Vec<Report>, Vec<Report>)>>,
    /// The kind of the last edit, so typing in one field is one undo step.
    last_edit: Rc<Cell<Option<&'static str>>>,
}

impl Editor {
    pub fn new(report: Report) -> Self {
        Self {
            report: State::new(report),
            selection: State::new(Selection::None),
            tab: State::new(0),
            zoom: State::new(1.25),
            version: State::new(0),
            page: State::new(0),
            gallery_open: State::new(true),
            wizard_open: State::new(false),
            drag: State::new(None),
            engine: Rc::new(RefCell::new(ReportEngine::new())),
            band_frames: Rc::new(RefCell::new(Vec::new())),
            history: Rc::new(RefCell::new((Vec::new(), Vec::new()))),
            last_edit: Rc::new(Cell::new(None)),
        }
    }

    /// Changes the report as one undoable step.
    pub fn edit(&self, f: impl FnOnce(&mut Report)) {
        self.last_edit.set(None);
        self.checkpoint();
        self.apply(f);
    }

    /// Changes the report; consecutive edits of the same `kind` (typing in one
    /// property) form one undo step.
    pub fn edit_as(&self, kind: &'static str, f: impl FnOnce(&mut Report)) {
        if self.last_edit.get() != Some(kind) {
            self.checkpoint();
            self.last_edit.set(Some(kind));
        }
        self.apply(f);
    }

    /// Records the current report for undo (start of a drag).
    pub fn checkpoint(&self) {
        let mut h = self.history.borrow_mut();
        h.0.push(self.report.get());
        if h.0.len() > 200 {
            h.0.remove(0);
        }
        h.1.clear();
    }

    /// Changes the report without recording an undo step (during a drag).
    pub fn apply(&self, f: impl FnOnce(&mut Report)) {
        self.report.update(f);
        self.version.update(|v| *v += 1);
    }

    /// Replaces the whole report (a template, a loaded file).
    pub fn load(&self, report: Report) {
        self.checkpoint();
        self.last_edit.set(None);
        self.report.set(report);
        self.selection.set(Selection::None);
        self.page.set(0);
        self.version.update(|v| *v += 1);
    }

    pub fn undo(&self) {
        let previous = self.history.borrow_mut().0.pop();
        if let Some(prev) = previous {
            let current = self.report.get();
            self.history.borrow_mut().1.push(current);
            self.report.set(prev);
            self.after_history();
        }
    }

    pub fn redo(&self) {
        let next = self.history.borrow_mut().1.pop();
        if let Some(next) = next {
            let current = self.report.get();
            self.history.borrow_mut().0.push(current);
            self.report.set(next);
            self.after_history();
        }
    }

    fn after_history(&self) {
        self.last_edit.set(None);
        self.version.update(|v| *v += 1);
        // The selection may point past the end of a band now.
        let valid = self.report.with(|r| match self.selection.get() {
            Selection::Control(b, i) => b.get(r).is_some_and(|band| i < band.controls.len()),
            Selection::Band(b) => b.get(r).is_some(),
            Selection::None => true,
        });
        if !valid {
            self.selection.set(Selection::None);
        }
    }

    /// The selected control, if any.
    pub fn selected_control(&self) -> Option<(BandRef, usize, Control)> {
        match self.selection.get() {
            Selection::Control(b, i) => self
                .report
                .with(|r| b.get(r).and_then(|band| band.controls.get(i).cloned()))
                .map(|c| (b, i, c)),
            _ => None,
        }
    }

    /// Changes the selected control.
    pub fn edit_control(&self, kind: &'static str, f: impl FnOnce(&mut Control)) {
        if let Selection::Control(b, i) = self.selection.get() {
            self.edit_as(kind, |r| {
                if let Some(c) = b.get_mut(r).and_then(|band| band.controls.get_mut(i)) {
                    f(c);
                }
            });
        }
    }

    /// Adds a control to `band` and selects it.
    pub fn add_control(&self, band: BandRef, control: Control) {
        let mut index = None;
        self.edit(|r| {
            if let Some(b) = band.get_mut(r) {
                // Grow the band if the control would stick out of it.
                b.height = b.height.max(control.y + control.height);
                b.controls.push(control);
                index = Some(b.controls.len() - 1);
            }
        });
        if let Some(i) = index {
            self.selection.set(Selection::Control(band, i));
        }
    }

    /// Deletes the selected control.
    pub fn delete_selected(&self) {
        if let Selection::Control(b, i) = self.selection.get() {
            self.edit(|r| {
                if let Some(band) = b.get_mut(r)
                    && i < band.controls.len()
                {
                    band.controls.remove(i);
                }
            });
            self.selection.set(Selection::Band(b));
        }
    }

    /// The band to insert new controls into: the selected one, else the detail.
    pub fn target_band(&self) -> BandRef {
        match self.selection.get() {
            Selection::Band(b) | Selection::Control(b, _) => b,
            Selection::None => BandRef::Detail,
        }
    }

    /// A free spot for a new control in `band`: below its lowest control.
    pub fn free_spot(&self, band: BandRef, height: f32) -> (f32, f32) {
        self.report.with(|r| {
            let Some(b) = band.get(r) else { return (0.0, 0.0) };
            // Next to the right-most control on the first row, if there is room.
            let width = r.page.content_width();
            let right = b.controls.iter().filter(|c| c.y < height).map(|c| c.x + c.width).fold(0.0, f32::max);
            if right + 110.0 <= width {
                (right, 0.0)
            } else {
                (0.0, b.controls.iter().map(|c| c.y + c.height).fold(0.0, f32::max))
            }
        })
    }

    /// The band under `point` (logical pixels), and the point in band coordinates.
    pub fn band_at(&self, point: Point) -> Option<(BandRef, Point)> {
        let zoom = self.zoom.get();
        self.band_frames
            .borrow()
            .iter()
            .find(|(_, f)| f.contains(point))
            .map(|(b, f)| (*b, Point::new((point.x - f.x()) / zoom, (point.y - f.y()) / zoom)))
    }
}

/// Rounds to the 2-point design grid.
pub fn snap(v: f32) -> f32 {
    (v / 2.0).round() * 2.0
}
