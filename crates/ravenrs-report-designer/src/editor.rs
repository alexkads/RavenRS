//! The designer's shared state: the report being edited, the selection,
//! undo/redo and the engine.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::prelude::*;
use ravenrs_report::{Control, ControlKind, Report, ReportEngine};

use crate::bands::BandRef;

/// What is selected on the design surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selection {
    None,
    Band(BandRef),
    Control(BandRef, usize),
}

/// How [`Editor::arrange_selected`] lines up or sizes the controls selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrange {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
    SameWidth,
    SameHeight,
    /// The same space between each two, side by side; the first and the last stay.
    SpaceAcross,
    /// The same space between each two, one under the other; the top and the bottom stay.
    SpaceDown,
}

/// Spreads the controls at `picked` so the space between each two is the same, across or down,
/// with the first and the last where they are. With fewer than three, nothing moves.
fn distribute(controls: &mut [Control], picked: &[usize], across: bool) {
    let start = |c: &Control| if across { c.x } else { c.y };
    let size = |c: &Control| if across { c.width } else { c.height };
    let mut order: Vec<usize> = picked.iter().copied().filter(|&i| i < controls.len()).collect();
    if order.len() < 3 {
        return;
    }
    order.sort_by(|&a, &b| start(&controls[a]).total_cmp(&start(&controls[b])));
    let (first, last) = (&controls[order[0]], &controls[order[order.len() - 1]]);
    let span = start(last) + size(last) - start(first);
    let filled: f32 = order.iter().map(|&i| size(&controls[i])).sum();
    let gap = (span - filled) / (order.len() - 1) as f32;
    let mut at = start(first);
    for &i in &order {
        let c = &mut controls[i];
        if across {
            c.x = at
        } else {
            c.y = at
        }
        at += size(c) + gap;
    }
}

/// A field being dragged from the field list.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDrag {
    /// The field, or for a control of the toolbox, its name.
    pub path: String,
    /// The control of the toolbox being dragged, by its place in `TOOLS`; `None` for a field.
    pub tool: Option<usize>,
    pub start: Point,
    pub at: Point,
}

/// An expression the expression editor can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpressionTarget {
    /// The selected control's expression: a field, a barcode's or a bookmark's.
    Control,
    /// The report's filter.
    Filter,
    /// What a group is grouped by.
    Group(usize),
}

/// Everything panels share. Cheap to clone.
#[derive(Clone)]
pub struct Editor {
    pub report: State<Report>,
    pub selection: State<Selection>,
    /// Other controls selected with the one of `selection`, in the same band, by place: Shift- or
    /// Cmd-click adds and removes them. Aligning and sizing take `selection` as the reference.
    pub also: State<Vec<usize>>,
    pub tab: State<usize>,
    pub zoom: State<f32>,
    pub version: State<u64>,
    pub page: State<usize>,
    pub gallery_open: State<bool>,
    pub wizard_open: State<bool>,
    pub thermal_open: State<bool>,
    pub drag: State<Option<FieldDrag>>,
    /// The controls last copied or cut, for Paste, the reference first.
    pub copied: State<Vec<Control>>,
    /// The expression open in the expression editor, and the text being edited.
    pub expression: State<Option<ExpressionTarget>>,
    pub expression_draft: State<String>,
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
            also: State::new(Vec::new()),
            tab: State::new(0),
            zoom: State::new(1.25),
            version: State::new(0),
            page: State::new(0),
            gallery_open: State::new(true),
            wizard_open: State::new(false),
            thermal_open: State::new(false),
            drag: State::new(None),
            copied: State::new(Vec::new()),
            expression: State::new(None),
            expression_draft: State::new(String::new()),
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
        self.also.set(Vec::new());
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
        // Places in a band may mean other controls now.
        self.also.set(Vec::new());
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
            self.also.set(Vec::new());
            self.selection.set(Selection::Control(band, i));
        }
    }

    /// Deletes the selected control.
    pub fn delete_selected(&self) {
        if let Some((b, mut indices)) = self.selected_indices() {
            // From the last, so the places of the others stay.
            indices.sort_unstable_by(|a, b| b.cmp(a));
            self.edit(|r| {
                if let Some(band) = b.get_mut(r) {
                    for i in indices {
                        if i < band.controls.len() {
                            band.controls.remove(i);
                        }
                    }
                }
            });
            self.also.set(Vec::new());
            self.selection.set(Selection::Band(b));
        }
    }

    /// Selects this control alone, unless it is already among those selected (a drag of one of
    /// them keeps the others).
    pub fn select_control(&self, band: BandRef, index: usize) {
        match self.selected_indices() {
            // One of those picked: the others stay, the one before it among them.
            Some((b, all)) if b == band && all.contains(&index) => {
                self.also.set(all.into_iter().filter(|&i| i != index).collect());
            }
            _ => self.also.set(Vec::new()),
        }
        self.selection.set(Selection::Control(band, index));
    }

    /// Shift- or Cmd-click: adds the control to those selected, or takes it out. A control of
    /// another band starts a selection of its own.
    pub fn toggle_selected(&self, band: BandRef, index: usize) {
        match self.selection.get() {
            Selection::Control(b, first) if b == band => {
                if index == first {
                    return;
                }
                self.also.update(|also| {
                    if let Some(at) = also.iter().position(|&i| i == index) {
                        also.remove(at);
                    } else {
                        also.push(index);
                    }
                });
            }
            _ => {
                self.also.set(Vec::new());
                self.selection.set(Selection::Control(band, index));
            }
        }
    }

    /// The band and the places of every control selected, the reference first.
    pub fn selected_indices(&self) -> Option<(BandRef, Vec<usize>)> {
        let Selection::Control(band, first) = self.selection.get() else { return None };
        let mut all = vec![first];
        all.extend(self.also.get().into_iter().filter(|&i| i != first));
        Some((band, all))
    }

    /// Whether more than one control is selected.
    pub fn several_selected(&self) -> bool {
        self.selected_indices().is_some_and(|(_, all)| all.len() > 1)
    }

    /// Lines up or sizes the controls selected to the reference, the first one selected, as one
    /// undoable step.
    pub fn arrange_selected(&self, how: Arrange) {
        let Some((band, all)) = self.selected_indices() else { return };
        if all.len() < 2 {
            return;
        }
        if matches!(how, Arrange::SpaceAcross | Arrange::SpaceDown) {
            self.edit(|r| {
                if let Some(b) = band.get_mut(r) {
                    distribute(&mut b.controls, &all, how == Arrange::SpaceAcross);
                }
            });
            return;
        }
        self.edit(|r| {
            let Some(b) = band.get_mut(r) else { return };
            let Some(anchor) = b.controls.get(all[0]).cloned() else { return };
            for &i in &all[1..] {
                let Some(c) = b.controls.get_mut(i) else { continue };
                match how {
                    Arrange::Left => c.x = anchor.x,
                    Arrange::Center => c.x = anchor.x + (anchor.width - c.width) / 2.0,
                    Arrange::Right => c.x = anchor.x + anchor.width - c.width,
                    Arrange::Top => c.y = anchor.y,
                    Arrange::Middle => c.y = anchor.y + (anchor.height - c.height) / 2.0,
                    Arrange::Bottom => c.y = anchor.y + anchor.height - c.height,
                    Arrange::SameWidth => c.width = anchor.width,
                    Arrange::SameHeight => c.height = anchor.height,
                    Arrange::SpaceAcross | Arrange::SpaceDown => {}
                }
            }
        });
    }

    /// The text of an expression, empty if there is none.
    pub fn expression_of(&self, target: ExpressionTarget) -> String {
        match target {
            ExpressionTarget::Control => self
                .selected_control()
                .and_then(|(_, _, c)| match c.kind {
                    ControlKind::Label(l) => l.expression,
                    ControlKind::Barcode(b) => b.expression,
                    ControlKind::Bookmark(b) => b.expression,
                    _ => None,
                })
                .unwrap_or_default(),
            ExpressionTarget::Filter => self.report.with(|r| r.filter.clone().unwrap_or_default()),
            ExpressionTarget::Group(i) => {
                self.report.with(|r| r.groups.get(i).map(|g| g.expression.clone()).unwrap_or_default())
            }
        }
    }

    /// Whether the selected control has an expression to edit.
    pub fn control_has_expression(&self) -> bool {
        self.selected_control().is_some_and(|(_, _, c)| match c.kind {
            ControlKind::Label(l) => l.expression.is_some(),
            ControlKind::Barcode(b) => b.expression.is_some(),
            ControlKind::Bookmark(b) => b.expression.is_some(),
            _ => false,
        })
    }

    /// Opens the expression editor on `target`.
    pub fn open_expression(&self, target: ExpressionTarget) {
        self.expression_draft.set(self.expression_of(target));
        self.expression.set(Some(target));
    }

    /// Writes the expression being edited and closes the editor.
    pub fn apply_expression(&self) {
        let Some(target) = self.expression.get() else { return };
        let text = self.expression_draft.get();
        match target {
            ExpressionTarget::Control => self.edit_control("expression", |c| match &mut c.kind {
                ControlKind::Label(l) => l.expression = Some(text),
                ControlKind::Barcode(b) => b.expression = Some(text),
                ControlKind::Bookmark(b) => b.expression = Some(text),
                _ => {}
            }),
            ExpressionTarget::Filter => {
                self.edit(|r| r.filter = (!text.trim().is_empty()).then_some(text));
            }
            ExpressionTarget::Group(i) => self.edit(|r| {
                if let Some(g) = r.groups.get_mut(i) {
                    g.expression = text;
                }
            }),
        }
        self.expression.set(None);
    }

    /// Keeps a copy of the selected control for Paste.
    pub fn copy_selected(&self) {
        let Some((band, all)) = self.selected_indices() else { return };
        let controls: Vec<Control> = self
            .report
            .with(|r| band.get(r).map(|b| all.iter().filter_map(|&i| b.controls.get(i).cloned()).collect()))
            .unwrap_or_default();
        if !controls.is_empty() {
            self.copied.set(controls);
        }
    }

    /// Copies the selected control and deletes it.
    pub fn cut_selected(&self) {
        if self.selected_control().is_some() {
            self.copy_selected();
            self.delete_selected();
        }
    }

    /// Adds the copied controls to the band in use, a little below and to the right of where they
    /// were, as one undoable step, and selects them.
    pub fn paste(&self) {
        let mut copies = self.copied.get();
        if copies.is_empty() {
            return;
        }
        let band = self.target_band();
        let width = self.report.with(|r| r.page.content_width());
        for c in &mut copies {
            c.x = snap((c.x + 8.0).min((width - c.width).max(0.0)));
            c.y = snap(c.y + 8.0);
        }
        let mut first = None;
        self.edit(|r| {
            if let Some(b) = band.get_mut(r) {
                first = Some(b.controls.len());
                for c in &copies {
                    b.height = b.height.max(c.y + c.height);
                    b.controls.push(c.clone());
                }
            }
        });
        if let Some(first) = first {
            self.also.set((first + 1..first + copies.len()).collect());
            self.selection.set(Selection::Control(band, first));
        }
        // The next paste lands below these, as in a stack of copies.
        self.copied.set(copies);
    }

    /// Moves the selected control over the others in its band (`front`), or under them.
    pub fn reorder_selected(&self, front: bool) {
        let Selection::Control(b, i) = self.selection.get() else { return };
        let mut to = None;
        self.edit(|r| {
            if let Some(band) = b.get_mut(r)
                && i < band.controls.len()
            {
                let c = band.controls.remove(i);
                let at = if front { band.controls.len() } else { 0 };
                band.controls.insert(at, c);
                to = Some(at);
            }
        });
        if let Some(at) = to {
            self.selection.set(Selection::Control(b, at));
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
