//! A list that builds only what shows, with rows of any height.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Key, Point, Role, Size};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::Semantic;

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::view::{AnyView, View};

/// Rows built past each end of the view, so that a fast wheel does not show holes.
const OVERSCAN: usize = 3;
/// What an arrow moves a list with nothing to select.
const LINE_STEP: f32 = 40.0;
/// Differences smaller than this are rounding, not a scroll.
const SAME_OFFSET: f32 = 0.25;

/// How a list knows the size of its rows.
#[derive(Clone)]
pub(crate) enum Sizing {
    /// Laid out and then read back; until then, this estimate.
    Measured(f32),
    /// All the same, so nothing has to be measured.
    Fixed(f32),
    /// Given by whoever uses the list.
    Known(Rc<dyn Fn(usize) -> f32>),
}

/// An order given to a list from outside its own build.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ScrollCommand {
    /// Puts this row at the start of the view.
    ToIndex(usize),
    /// The least scroll that shows this row whole.
    Reveal(usize),
    /// Moves by this many logical pixels.
    By(f32),
}

/// What a list found out in its last build, for the view around it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ListProbe {
    /// How far it is scrolled.
    pub offset: f32,
    /// How long the view is, along the list.
    pub view: f32,
    /// How long all the rows are.
    pub total: f32,
    /// The row in the middle of the view.
    pub current: usize,
}

/// Where each row is. It costs a look at every key per build and an addition per row when
/// something changed, and never a lookup by key unless the keys themselves changed.
#[derive(Default)]
struct Track {
    keys: Vec<u64>,
    sizes: Vec<f32>,
    /// `sizes` added up, one entry longer: `tops[n]` is the whole length.
    tops: Vec<f32>,
    measured: HashMap<u64, f32>,
    /// The rows the last build made, which are the ones with something new to read.
    built: Vec<(u64, usize)>,
    /// Room before the first row and after the last.
    padding: f32,
    /// The row at the start of the view and how far past its start the view is. What stays
    /// still when rows above it come in or turn out to have another size.
    anchor: Option<(u64, usize, f32)>,
    /// The offset the last build left; another one here means somebody scrolled.
    left_at: Option<f32>,
    at_end: bool,
    /// A row to show whole, kept until the rows around it have settled on their size.
    revealing: Option<usize>,
    visible: Option<Range<usize>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Track {
    fn total(&self) -> f32 {
        self.tops.last().map_or(0.0, |end| end + self.padding)
    }

    fn max_offset(&self, view: f32) -> f32 {
        (self.total() - view).max(0.0)
    }

    fn sum(&mut self) {
        self.tops.clear();
        self.tops.reserve(self.sizes.len() + 1);
        let mut at = self.padding;
        self.tops.push(at);
        for size in &self.sizes {
            at += size;
            self.tops.push(at);
        }
    }

    /// Brings keys and sizes up to date with the data.
    fn sync(&mut self, list: &VirtualList) {
        let count = list.count;
        let anchor_size = self.anchor.and_then(|(_, index, _)| self.sizes.get(index).copied());
        let mut stale = self.keys.len() != count;
        if !stale && let Some(key) = &list.key {
            stale = (0..count).any(|i| self.keys[i] != key(i));
        }
        let mut changed = stale || self.padding != list.padding;
        self.padding = list.padding;
        if stale {
            self.keys.clear();
            self.keys.extend((0..count).map(|i| list.key.as_ref().map_or(i as u64, |key| key(i))));
            let default = list.default_size();
            self.sizes.clear();
            for key in &self.keys {
                self.sizes.push(self.measured.get(key).copied().unwrap_or(default));
            }
        }
        match &list.sizing {
            Sizing::Measured(_) => {}
            Sizing::Fixed(size) => {
                let size = size + list.spacing;
                if self.sizes.first().is_some_and(|s| (s - size).abs() > f32::EPSILON) {
                    self.sizes.fill(size);
                    changed = true;
                }
            }
            Sizing::Known(size_of) => {
                for i in 0..count {
                    let size = size_of(i) + list.spacing;
                    if (self.sizes[i] - size).abs() > f32::EPSILON {
                        self.sizes[i] = size;
                        changed = true;
                    }
                }
                // A zoom changes every row: the view stays at the same place inside its row.
                if let (Some(before), Some((key, index, delta))) = (anchor_size, self.anchor)
                    && let Some(&now) = self.sizes.get(index)
                    && before > 0.0
                {
                    self.anchor = Some((key, index, delta * now / before));
                }
            }
        }
        if changed || self.tops.len() != count + 1 {
            self.sum();
        }
    }

    fn take_measurements(&mut self, cx: &BuildCx<'_>, list: &VirtualList) {
        if !matches!(list.sizing, Sizing::Measured(_)) {
            return;
        }
        let mut changed = false;
        for &(key, index) in &self.built {
            let Some(frame) = cx.keyed_child_frame(key) else { continue };
            let size = list.along(frame);
            if self.measured.get(&key).is_none_or(|known| (known - size).abs() > 0.1) {
                self.measured.insert(key, size);
                // Rows moved since then found their size when the keys were taken.
                if self.keys.get(index) == Some(&key) {
                    self.sizes[index] = size;
                }
                changed = true;
            }
        }
        if changed {
            self.sum();
        }
    }

    /// The row holding this offset.
    fn index_at(&self, offset: f32) -> usize {
        let last = self.keys.len().saturating_sub(1);
        self.tops.partition_point(|top| *top <= offset).saturating_sub(1).min(last)
    }

    fn set_anchor(&mut self, offset: f32) {
        if self.keys.is_empty() {
            self.anchor = None;
            return;
        }
        let index = self.index_at(offset);
        self.anchor = Some((self.keys[index], index, offset - self.tops[index]));
    }

    /// Where the anchor has gone. A row that left the list leaves the offset alone.
    fn anchored(&mut self) -> Option<f32> {
        let (key, index, delta) = self.anchor?;
        let index = if self.keys.get(index) == Some(&key) {
            index
        } else {
            self.keys.iter().position(|k| *k == key)?
        };
        self.anchor = Some((key, index, delta));
        Some(self.tops[index] + delta)
    }
}

/// A list of any length that builds only the rows that show, with rows of any height. They
/// are laid out and measured as they appear, and what was measured is kept by key, so going
/// back finds them at the right size. It can jump to a row, stay at the end as a chat does,
/// and keep what is on screen still when rows come in above it. With nothing to select, the
/// arrows, Page Up, Page Down, Home and End scroll; with a [`selection`](Self::selection) or
/// [`on_activate`](Self::on_activate), they move between rows, and Enter or Space activates.
///
/// Rows of the same height, told with [`row_height`](Self::row_height), need no measuring.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Text, Ui, VirtualList};
///
/// let chosen = State::new(None::<usize>);
/// let list = VirtualList::new(10_000, |index| Text::new(format!("Row {index}")))
///     .key(|index| index as u64)
///     .selection(chosen.binding())
///     .height(320.0);
/// let mut ui = Ui::new(list, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct VirtualList {
    pub(crate) count: usize,
    pub(crate) item: Rc<dyn Fn(usize) -> AnyView>,
    pub(crate) key: Option<Rc<dyn Fn(usize) -> u64>>,
    pub(crate) sizing: Sizing,
    pub(crate) spacing: f32,
    follow_end: bool,
    scroll_to: Option<Binding<Option<usize>>>,
    selection: Option<Binding<Option<usize>>>,
    on_activate: Option<Rc<dyn Fn(usize)>>,
    on_visible: Option<Rc<dyn Fn(Range<usize>)>>,
    row_name: Option<Rc<dyn Fn(usize) -> String>>,
    label: Option<String>,
    length: Option<f32>,
    horizontal: bool,
    /// Rows, focus and semantics are of the view around it.
    bare: bool,
    command: Option<State<Option<ScrollCommand>>>,
    probe: Option<Rc<Cell<ListProbe>>>,
    /// Scrolls across as well, up to this width of content.
    cross_content: Option<f32>,
    row_align: Align,
    padding: f32,
}

impl fmt::Debug for VirtualList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualList").field("count", &self.count).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl VirtualList {
    /// A list of `count` rows, with `item` making the view of each one when it shows.
    pub fn new<V: View + 'static>(count: usize, item: impl Fn(usize) -> V + 'static) -> Self {
        Self {
            count,
            item: Rc::new(move |index| Box::new(item(index))),
            key: None,
            sizing: Sizing::Measured(40.0),
            spacing: 0.0,
            follow_end: false,
            scroll_to: None,
            selection: None,
            on_activate: None,
            on_visible: None,
            row_name: None,
            label: None,
            length: None,
            horizontal: false,
            bare: false,
            command: None,
            probe: None,
            cross_content: None,
            row_align: Align::Stretch,
            padding: 0.0,
        }
    }

    /// What identifies a row, which is its position by default. With it, a row keeps its
    /// measured height and what is on screen stays still when rows come in above. It is
    /// asked for every row on each build, so it should only read a value.
    pub fn key(mut self, key: impl Fn(usize) -> u64 + 'static) -> Self {
        self.key = Some(Rc::new(key));
        self
    }

    /// How tall a row probably is until it has been measured (40 by default).
    pub fn estimated_height(mut self, height: f32) -> Self {
        self.sizing = Sizing::Measured(height.max(1.0));
        self
    }

    /// Every row has this height, and nothing has to be measured.
    pub fn row_height(mut self, height: f32) -> Self {
        self.sizing = Sizing::Fixed(height.max(1.0));
        self
    }

    /// The height of each row is known, as with the pages of a document. It is asked for every
    /// row on each build.
    pub fn row_heights(mut self, height: impl Fn(usize) -> f32 + 'static) -> Self {
        self.sizing = Sizing::Known(Rc::new(height));
        self
    }

    /// Gap between rows, in logical pixels.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    /// Starts at the end and stays there as rows arrive, as a chat does, until the user
    /// scrolls away; coming back to the end takes it up again.
    pub fn follow_end(mut self, follow: bool) -> Self {
        self.follow_end = follow;
        self
    }

    /// Set it to a row to bring that row to the start of the view. It goes back to `None`.
    pub fn scroll_to(mut self, index: Binding<Option<usize>>) -> Self {
        self.scroll_to = Some(index);
        self
    }

    /// Makes the rows selectable, one at a time, with the pointer and the arrows.
    pub fn selection(mut self, selected: Binding<Option<usize>>) -> Self {
        self.selection = Some(selected);
        self
    }

    /// Called with a row on a double click, or on Enter or Space.
    pub fn on_activate(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Called with the rows that show whenever they change, as for loading what comes next.
    pub fn on_visible(mut self, handler: impl Fn(Range<usize>) + 'static) -> Self {
        self.on_visible = Some(Rc::new(handler));
        self
    }

    /// What screen readers call each row, for rows whose content does not say it.
    pub fn row_name(mut self, name: impl Fn(usize) -> String + 'static) -> Self {
        self.row_name = Some(Rc::new(name));
        self
    }

    /// What screen readers call the list, instead of the name in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A fixed height. It takes the height its parent gives it by default.
    pub fn height(mut self, height: f32) -> Self {
        self.length = Some(height);
        self
    }

    /// Runs from left to right; the sizes above are then widths, and the height is across.
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    pub(crate) fn bare(mut self) -> Self {
        self.bare = true;
        self
    }

    pub(crate) fn commands(mut self, command: State<Option<ScrollCommand>>) -> Self {
        self.command = Some(command);
        self
    }

    pub(crate) fn probe(mut self, probe: Rc<Cell<ListProbe>>) -> Self {
        self.probe = Some(probe);
        self
    }

    pub(crate) fn scroll_across(mut self, content: f32) -> Self {
        self.cross_content = Some(content);
        self
    }

    pub(crate) fn row_align(mut self, align: Align) -> Self {
        self.row_align = align;
        self
    }

    /// Room before the first row and after the last, in logical pixels.
    pub(crate) fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    fn default_size(&self) -> f32 {
        match &self.sizing {
            Sizing::Measured(estimate) => estimate + self.spacing,
            Sizing::Fixed(size) => size + self.spacing,
            Sizing::Known(_) => self.spacing,
        }
    }

    fn along(&self, size: Size) -> f32 {
        if self.horizontal { size.width } else { size.height }
    }

    fn axis_point(&self, along: f32, across: f32) -> Point {
        if self.horizontal { Point::new(along, across) } else { Point::new(across, along) }
    }

    fn interactive(&self) -> bool {
        self.selection.is_some() || self.on_activate.is_some()
    }

    fn spacer(&self, cx: &mut BuildCx<'_>, length: f32) -> NodeId {
        let style = if self.horizontal {
            LayoutStyle { width: Some(length), ..LayoutStyle::leaf() }
        } else {
            LayoutStyle { height: Some(length), ..LayoutStyle::leaf() }
        };
        cx.container(style, &[], Paint::None)
    }

    fn apply(&self, track: &mut Track, command: ScrollCommand, offset: f32, view: f32) -> f32 {
        let index = |i: usize| i.min(self.count.saturating_sub(1));
        let target = match command {
            ScrollCommand::ToIndex(i) => {
                let i = index(i);
                let (key, top) = (track.keys.get(i).copied(), track.tops.get(i).copied().unwrap_or(0.0));
                track.at_end = false;
                let target = (top - self.padding).clamp(0.0, track.max_offset(view));
                track.anchor = key.map(|key| (key, i, target - top));
                return target;
            }
            ScrollCommand::Reveal(i) => {
                let i = index(i);
                let (Some(top), Some(bottom)) = (track.tops.get(i), track.tops.get(i + 1)) else {
                    return offset;
                };
                let bottom = bottom - self.spacing + self.padding;
                if *top < offset {
                    top - self.padding
                } else if bottom > offset + view {
                    (bottom - view).min(*top)
                } else {
                    offset
                }
            }
            ScrollCommand::By(by) => offset + by,
        };
        let target = target.clamp(0.0, track.max_offset(view));
        track.set_anchor(target);
        track.at_end = self.follow_end && target >= track.max_offset(view) - 1.0;
        target
    }
}

/// A row of a [`VirtualList`]: what measures it and what makes it a row of a list.
struct Row<'a> {
    list: &'a VirtualList,
    index: usize,
    size: f32,
    selected: bool,
    active: bool,
    pick: Option<Rc<dyn Fn(usize)>>,
}

impl View for Row<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let list = self.list;
        let theme = *cx.theme();
        let item = (list.item)(self.index);
        let child = cx.child(&*item);
        let fixed = !matches!(list.sizing, Sizing::Measured(_));
        let gap = ravenrs_core::Insets {
            bottom: if list.horizontal { 0.0 } else { list.spacing },
            right: if list.horizontal { list.spacing } else { 0.0 },
            ..ravenrs_core::Insets::default()
        };
        let accent = theme.color(Semantic::Accent);
        let paint = if self.selected || self.active {
            let fill = if self.selected { accent.with_alpha(0.14) } else { Color::TRANSPARENT };
            let border = self.active.then_some(Border { width: 1.5, color: accent });
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, border, ..QuadPaint::new(fill) })
        } else {
            Paint::None
        };
        let node = cx.container(
            LayoutStyle {
                width: (fixed && list.horizontal).then_some(self.size),
                height: (fixed && !list.horizontal).then_some(self.size),
                min_width: Some(0.0),
                padding: gap,
                overflow_hidden: fixed,
                align: list.row_align,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[child],
            paint,
        );
        if !fixed {
            cx.report_frame(node);
        }
        if list.bare {
            return node;
        }
        let mut semantics = Semantics::new(Role::ListItem).selected(self.selected).active(self.active);
        if let Some(name) = &list.row_name {
            semantics = semantics.name(name(self.index));
        }
        cx.semantics(node, semantics);
        if let Some(pick) = &self.pick {
            let (pick, index) = (pick.clone(), self.index);
            cx.on_click(node, Rc::new(move || pick(index)));
            cx.cursor(node, CursorIcon::Pointer);
        }
        if let Some(open) = &list.on_activate {
            let (open, index) = (open.clone(), self.index);
            cx.on_double_click(node, Rc::new(move || open(index)));
        }
        node
    }
}

impl View for VirtualList {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let track = cx.use_state(|| Rc::new(RefCell::new(Track::default()))).peek().clone();
        let local: State<Option<ScrollCommand>> = cx.use_state(|| None);
        let commands = self.command.clone().unwrap_or_else(|| local.clone());
        let cursor: State<usize> = cx.use_state(|| 0);
        let selected = self.selection.as_ref().and_then(Binding::get);

        let raw = cx.scroll_offset();
        let frame = cx.scroll_viewport();
        let window = cx.viewport();
        let view = frame.map_or(self.length.unwrap_or(self.along(window)), |size| self.along(size));
        let cross = frame.map_or(if self.horizontal { window.height } else { window.width }, |size| {
            if self.horizontal { size.height } else { size.width }
        });
        let mut offset = if self.horizontal { raw.x } else { raw.y };
        let across = if self.horizontal { raw.y } else { raw.x };

        let mut t = track.borrow_mut();
        let first_build = t.left_at.is_none();
        if first_build {
            t.at_end = self.follow_end;
        } else if t.left_at.is_some_and(|left| (left - offset).abs() > SAME_OFFSET) {
            // Somebody scrolled: this is the new place to hold, over the sizes it was seen with.
            t.set_anchor(offset);
            t.at_end = self.follow_end && offset >= t.max_offset(view) - 1.0;
            t.revealing = None;
        }
        t.sync(self);
        t.take_measurements(cx, self);

        let mut desired = t.anchored().unwrap_or(offset);
        if t.at_end {
            desired = t.max_offset(view);
        }
        desired = desired.clamp(0.0, t.max_offset(view));

        // Each order is read once. Setting them back during a build only asks for another,
        // which a jump needs anyway.
        if let Some(order) = commands.get() {
            commands.set(None);
            t.revealing = match order {
                ScrollCommand::Reveal(index) => Some(index),
                _ => None,
            };
            desired = self.apply(&mut t, order, desired, view);
        }
        if let Some(asked) = &self.scroll_to
            && let Some(index) = asked.get()
        {
            asked.set(None);
            t.revealing = None;
            desired = self.apply(&mut t, ScrollCommand::ToIndex(index), desired, view);
        }
        if let Some(index) = t.revealing {
            desired = self.apply(&mut t, ScrollCommand::Reveal(index), desired, view);
        }
        if (desired - offset).abs() > SAME_OFFSET {
            cx.set_scroll_offset(self.axis_point(desired, across));
            offset = desired;
        }
        t.left_at = Some(offset);
        if t.anchor.is_none() {
            t.set_anchor(offset);
        }

        let count = self.count;
        let first_seen = t.index_at(offset);
        let end_seen = t.tops.partition_point(|top| *top < offset + view).max(first_seen + 1).min(count);
        let current = if offset >= t.max_offset(view) - 1.0 && t.max_offset(view) > 0.0 {
            count.saturating_sub(1)
        } else if offset <= 0.0 {
            0
        } else {
            t.index_at(offset + view / 2.0)
        };
        if let Some(probe) = &self.probe {
            probe.set(ListProbe { offset, view, total: t.total(), current });
        }
        let seen = first_seen.min(end_seen)..end_seen;
        if t.visible.as_ref() != Some(&seen) {
            t.visible = Some(seen.clone());
            if let Some(handler) = &self.on_visible {
                handler(seen);
            }
        }
        let first = first_seen.saturating_sub(OVERSCAN);
        let last = (end_seen + OVERSCAN).min(count);
        let before = t.tops.get(first).map_or(0.0, |top| top - self.padding);
        let after = t.tops.last().copied().unwrap_or(0.0) - t.tops.get(last).copied().unwrap_or(0.0);
        let keys: Vec<u64> = t.keys.get(first..last).unwrap_or(&[]).to_vec();
        let sizes: Vec<f32> = t.sizes.get(first..last).unwrap_or(&[]).to_vec();
        t.built.clear();
        t.built.extend(keys.iter().enumerate().map(|(at, key)| (*key, first + at)));
        // Nothing below reads the track, and rows may build lists of their own.
        let unmeasured = matches!(self.sizing, Sizing::Measured(_))
            && keys.iter().any(|key| !t.measured.contains_key(key));
        if !unmeasured {
            t.revealing = None;
        }
        drop(t);
        if unmeasured {
            // Rows that appear in the last build of a frame are only known in the next one.
            cx.wake_at(cx.now());
        }

        let active = cursor.get().min(count.saturating_sub(1));
        let pick: Option<Rc<dyn Fn(usize)>> = self.selection.clone().map(|selection| {
            let cursor = cursor.clone();
            Rc::new(move |index: usize| {
                cursor.set(index);
                selection.set(Some(index));
            }) as Rc<dyn Fn(usize)>
        });
        let pick = pick.or_else(|| {
            self.on_activate.as_ref().map(|_| {
                let cursor = cursor.clone();
                Rc::new(move |index: usize| cursor.set(index)) as Rc<dyn Fn(usize)>
            })
        });

        let mut nodes = Vec::with_capacity(keys.len() + 2);
        nodes.push(self.spacer(cx, before));
        let focused = cx.is_focus_visible();
        for (at, (key, size)) in keys.iter().zip(&sizes).enumerate() {
            let index = first + at;
            let row = Row {
                list: self,
                index,
                size: *size,
                selected: !self.bare && selected == Some(index),
                active: !self.bare && self.interactive() && focused && index == active,
                pick: pick.clone(),
            };
            nodes.push(cx.keyed_child(*key, &row));
        }
        nodes.push(self.spacer(cx, after));
        let padding = if self.horizontal {
            ravenrs_core::Insets::symmetric(0.0, self.padding)
        } else {
            ravenrs_core::Insets::symmetric(self.padding, 0.0)
        };
        let content = cx.container(
            LayoutStyle {
                align: Align::Stretch,
                padding,
                // Across a strip, the rows take the whole height it has.
                grow: if self.horizontal { 1.0 } else { 0.0 },
                min_width: self.cross_content.map(|content| content.max(cross)),
                ..LayoutStyle::stack(if self.horizontal { Axis::Horizontal } else { Axis::Vertical })
            },
            &nodes,
            Paint::None,
        );
        let body = cx.container(
            LayoutStyle {
                height: self.length,
                grow: if self.length.is_some() { 0.0 } else { 1.0 },
                min_height: Some(0.0),
                min_width: Some(0.0),
                overflow_hidden: true,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        cx.scrollable_virtual(body, self.horizontal || self.cross_content.is_some());
        if self.horizontal {
            cx.wheel_scrolls_sideways(body);
        }
        if self.bare {
            return body;
        }

        let name = self.label.clone().unwrap_or_else(|| cx.labels().list_name.clone());
        cx.semantics(body, Semantics::new(Role::List).name(name).description(count.to_string()));
        cx.focusable(body, cx.theme().radii.sm);
        let (interactive, selection, open) =
            (self.interactive(), self.selection.clone(), self.on_activate.clone());
        let horizontal = self.horizontal;
        let page = (end_seen.saturating_sub(first_seen)).max(2) - 1;
        cx.on_key(
            body,
            Rc::new(move |key, _| {
                if count == 0 {
                    return false;
                }
                let (back, forward) = if horizontal {
                    (Key::ArrowLeft, Key::ArrowRight)
                } else {
                    (Key::ArrowUp, Key::ArrowDown)
                };
                if !interactive {
                    let order = match key {
                        k if *k == back => ScrollCommand::By(-LINE_STEP),
                        k if *k == forward => ScrollCommand::By(LINE_STEP),
                        Key::PageUp => ScrollCommand::By(-view * 0.9),
                        Key::PageDown => ScrollCommand::By(view * 0.9),
                        Key::Home => ScrollCommand::ToIndex(0),
                        Key::End => ScrollCommand::Reveal(count - 1),
                        _ => return false,
                    };
                    commands.set(Some(order));
                    return true;
                }
                let to = match key {
                    k if *k == back => active.saturating_sub(1),
                    k if *k == forward => (active + 1).min(count - 1),
                    Key::PageUp => active.saturating_sub(page),
                    Key::PageDown => (active + page).min(count - 1),
                    Key::Home => 0,
                    Key::End => count - 1,
                    Key::Enter | Key::Space => {
                        if let Some(open) = &open {
                            open(active);
                        }
                        if let Some(selection) = &selection {
                            selection.set(Some(active));
                        }
                        return true;
                    }
                    _ => return false,
                };
                cursor.set(to);
                if let Some(selection) = &selection {
                    selection.set(Some(to));
                }
                commands.set(Some(ScrollCommand::Reveal(to)));
                true
            }),
        );
        body
    }
}
