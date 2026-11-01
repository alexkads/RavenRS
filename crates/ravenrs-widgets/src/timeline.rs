//! Things along time, on tracks: the timeline of a video or audio editor, and of
//! anything else planned in milliseconds.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, Drag, DragPhase, NodeId, Paint, QuadPaint, Semantics};
use crate::{AnyView, ScrollView, Text, Tone, View, ViewExt};

const RULER_HEIGHT: f32 = 26.0;
const HANDLE: f32 = 8.0;
const MARKER: f32 = 10.0;
/// Labels on the ruler stay at least this far apart, in logical pixels.
const TICK_ROOM: f32 = 56.0;
const TICK_STEPS: [u32; 9] = [1, 2, 5, 10, 15, 30, 60, 120, 300];
const LONGEST_TICK: u32 = 600;

/// One block on a [`TimelineTrack`].
pub struct TimelineItem {
    id: u64,
    start_ms: u64,
    duration_ms: u64,
    label: String,
    tone: Tone,
    selected: bool,
    movable: bool,
    trimmable: bool,
    leading: Option<AnyView>,
}

impl std::fmt::Debug for TimelineItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimelineItem")
            .field("id", &self.id)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl TimelineItem {
    /// A block known to the app as `id`, from `start_ms` for `duration_ms`.
    pub fn new(id: u64, start_ms: u64, duration_ms: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            start_ms,
            duration_ms,
            label: label.into(),
            tone: Tone::Neutral,
            selected: false,
            movable: false,
            trimmable: false,
            leading: None,
        }
    }

    /// Its color.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Marks it as the selected one.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// It can be dragged along its track.
    pub fn movable(mut self) -> Self {
        self.movable = true;
        self
    }

    /// Its ends can be dragged.
    pub fn trimmable(mut self) -> Self {
        self.trimmable = true;
        self
    }

    /// Something before its label, such as a thumbnail.
    pub fn leading(mut self, view: impl View + 'static) -> Self {
        self.leading = Some(Box::new(view));
        self
    }
}

/// A row of a [`Timeline`].
#[derive(Debug)]
pub struct TimelineTrack {
    label: String,
    height: f32,
    items: Vec<TimelineItem>,
}

impl TimelineTrack {
    /// A track named `label`, `height` logical pixels tall.
    pub fn new(label: impl Into<String>, height: f32, items: Vec<TimelineItem>) -> Self {
        Self { label: label.into(), height, items }
    }
}

/// A named moment, on the ruler of a [`Timeline`].
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineMarker {
    /// When.
    pub at_ms: u64,
    /// Its name.
    pub title: String,
}

/// What someone did to a block, for the app to apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TimelineEdit {
    /// A drag began on this block: the moves that follow are one gesture.
    Began {
        /// The block.
        id: u64,
    },
    /// A block should now start here.
    Moved {
        /// The block.
        id: u64,
        /// Its new start.
        start_ms: u64,
    },
    /// A block's start moved by this much.
    TrimmedStart {
        /// The block.
        id: u64,
        /// How much, negative for earlier.
        by_ms: i64,
    },
    /// A block's end moved by this much.
    TrimmedEnd {
        /// The block.
        id: u64,
        /// How much, negative for earlier.
        by_ms: i64,
    },
}

/// `2:05`.
fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Things along time, on named tracks, under a ruler with markers and a playhead.
///
/// It holds nothing: the app gives it what is there and where the playhead is,
/// and gets back what people do. Blocks are buttons, so they work with the
/// keyboard. Dragging the ruler moves the playhead, dragging a
/// [movable](TimelineItem::movable) block moves it, and the ends of a
/// [trimmable](TimelineItem::trimmable) one change its length.
///
/// ```
/// use ravenrs_widgets::{Timeline, TimelineItem, TimelineTrack, Tone};
///
/// let timeline = Timeline::new(vec![
///     TimelineTrack::new("Notes", 34.0, vec![TimelineItem::new(1, 2_000, 3_000, "Callout").movable()]),
///     TimelineTrack::new("Film", 56.0, vec![TimelineItem::new(2, 0, 10_000, "Take 1").tone(Tone::Accent)]),
/// ])
/// .position_ms(4_000)
/// .on_seek(|ms| println!("{ms}"));
/// ```
pub struct Timeline {
    tracks: Vec<TimelineTrack>,
    markers: Vec<TimelineMarker>,
    position_ms: u64,
    pixels_per_second: f32,
    heads_width: f32,
    on_seek: Option<Rc<dyn Fn(u64)>>,
    on_select: Option<Rc<dyn Fn(u64)>>,
    on_edit: Option<Rc<dyn Fn(TimelineEdit)>>,
}

impl std::fmt::Debug for Timeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timeline").field("tracks", &self.tracks).finish_non_exhaustive()
    }
}

impl Timeline {
    /// A timeline of these tracks, top to bottom.
    pub fn new(tracks: Vec<TimelineTrack>) -> Self {
        Self {
            tracks,
            markers: Vec::new(),
            position_ms: 0,
            pixels_per_second: 28.0,
            heads_width: 112.0,
            on_seek: None,
            on_select: None,
            on_edit: None,
        }
    }

    /// The named moments on its ruler.
    pub fn markers(mut self, markers: Vec<TimelineMarker>) -> Self {
        self.markers = markers;
        self
    }

    /// Where the playhead is.
    pub fn position_ms(mut self, ms: u64) -> Self {
        self.position_ms = ms;
        self
    }

    /// How wide a second is, in logical pixels (28 by default).
    pub fn pixels_per_second(mut self, pixels: f32) -> Self {
        self.pixels_per_second = pixels.max(1.0);
        self
    }

    /// The width of the column of track names (112 by default).
    pub fn heads_width(mut self, width: f32) -> Self {
        self.heads_width = width;
        self
    }

    /// Runs when someone moves the playhead.
    pub fn on_seek(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_seek = Some(Rc::new(handler));
        self
    }

    /// Runs when someone selects a block.
    pub fn on_select(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Runs when someone moves or trims a block.
    pub fn on_edit(mut self, handler: impl Fn(TimelineEdit) + 'static) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    /// The whole height: the ruler and every track.
    pub fn total_height(&self) -> f32 {
        RULER_HEIGHT + self.tracks.iter().map(|track| track.height).sum::<f32>()
    }

    fn end_ms(&self) -> u64 {
        self.tracks
            .iter()
            .flat_map(|track| &track.items)
            .map(|item| item.start_ms + item.duration_ms)
            .max()
            .unwrap_or(0)
    }
}

/// The column of track names.
struct Heads<'a>(&'a Timeline);

impl View for Heads<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut children = vec![cx.container(
            LayoutStyle { height: Some(RULER_HEIGHT), ..LayoutStyle::stack(Axis::Vertical) },
            &[],
            Paint::None,
        )];
        for track in &self.0.tracks {
            let label = cx.child(&Text::new(track.label.clone()).style(TextStyle::Callout));
            children.push(cx.container(
                LayoutStyle {
                    height: Some(track.height),
                    padding: Insets::symmetric(0.0, theme.spacing.s3),
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[label],
                Paint::None,
            ));
        }
        cx.container(
            LayoutStyle { width: Some(self.0.heads_width), ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

/// One end of a block.
struct Handle {
    x: f32,
    height: f32,
    id: u64,
    at_start: bool,
    per_ms: f32,
    on_edit: Rc<dyn Fn(TimelineEdit)>,
}

impl View for Handle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.x, 0.0)),
                width: Some(HANDLE),
                height: Some(self.height),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[],
            Paint::None,
        );
        let (id, at_start, per_ms, on_edit) = (self.id, self.at_start, self.per_ms, self.on_edit.clone());
        let last = cx.use_state(|| Rc::new(Cell::new(0.0_f32))).get();
        cx.on_drag(
            node,
            Rc::new(move |drag: Drag| {
                if drag.phase == DragPhase::Start {
                    last.set(drag.position.x);
                    on_edit(TimelineEdit::Began { id });
                    return;
                }
                let by_ms = ((drag.position.x - last.get()) / per_ms) as i64;
                if by_ms != 0 {
                    // Only what was used: the fractions stay for the next move.
                    last.set(last.get() + by_ms as f32 * per_ms);
                    on_edit(if at_start {
                        TimelineEdit::TrimmedStart { id, by_ms }
                    } else {
                        TimelineEdit::TrimmedEnd { id, by_ms }
                    });
                }
            }),
        );
        cx.cursor(node, CursorIcon::ColResize);
        cx.semantics(node, Semantics::hidden());
        node
    }
}

/// A block.
struct Block<'a> {
    item: &'a TimelineItem,
    frame: Rect,
    per_ms: f32,
    on_select: Option<Rc<dyn Fn(u64)>>,
    on_edit: Option<Rc<dyn Fn(TimelineEdit)>>,
}

impl View for Block<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let item = self.item;
        let tint = item.tone.color(&theme);
        let (width, height) = (self.frame.width().max(4.0), self.frame.height());
        let mut row = Vec::new();
        if let Some(leading) = &item.leading {
            row.push(cx.child(leading));
        }
        row.push(cx.child(&Text::new(item.label.clone()).style(TextStyle::Caption).truncate().grow(1.0)));
        let content = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                padding: Insets::symmetric(0.0, HANDLE + 2.0),
                align: Align::Center,
                height: Some(height),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &row,
            Paint::None,
        );
        let mut children = vec![content];
        if let (true, Some(on_edit)) = (item.trimmable, &self.on_edit) {
            let handle = |x: f32, at_start: bool| Handle {
                x,
                height,
                id: item.id,
                at_start,
                per_ms: self.per_ms,
                on_edit: on_edit.clone(),
            };
            children.push(cx.child(&handle(0.0, true)));
            children.push(cx.child(&handle((width - HANDLE).max(0.0), false)));
        }
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.frame.x(), self.frame.y())),
                width: Some(width),
                height: Some(height),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::Quad(QuadPaint {
                fill: tint.faded(if item.selected { 0.34 } else { 0.18 }).into(),
                corner_radius: theme.radii.sm,
                border: Some(Border {
                    width: if item.selected { 2.0 } else { 1.0 },
                    color: if item.selected { theme.color(Semantic::FocusRing) } else { tint },
                }),
                shadow: None,
            }),
        );
        cx.clip(node);
        let mut semantics = Semantics::new(Role::Button).name(item.label.clone());
        if item.selected {
            semantics = semantics.selected(true);
        }
        cx.semantics(node, semantics);
        if let Some(select) = &self.on_select {
            let (select, id) = (select.clone(), item.id);
            cx.on_click(node, Rc::new(move || select(id)));
            cx.focusable(node, theme.radii.sm);
            cx.cursor(node, CursorIcon::Pointer);
        }
        if let (true, Some(on_edit)) = (item.movable, self.on_edit.clone()) {
            let (id, per_ms) = (item.id, self.per_ms);
            let grab = cx.use_state(|| Rc::new(Cell::new(0.0_f32))).get();
            cx.on_drag(
                node,
                Rc::new(move |drag: Drag| {
                    if drag.phase == DragPhase::Start {
                        grab.set(drag.position.x - drag.frame.x());
                        on_edit(TimelineEdit::Began { id });
                        return;
                    }
                    let x = (drag.position.x - grab.get() - drag.container.x()).max(0.0);
                    on_edit(TimelineEdit::Moved { id, start_ms: (x / per_ms) as u64 });
                }),
            );
        }
        node
    }
}

/// The ruler, the rows and the playhead, wider than the view when they have to be.
struct Rows<'a>(&'a Timeline);

impl View for Rows<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let timeline = self.0;
        let per_ms = timeline.pixels_per_second / 1000.0;
        let width = (timeline.end_ms() as f32 * per_ms + 200.0).max(cx.viewport().width);
        let height = timeline.total_height();
        let mut children = Vec::new();

        // The ruler, under everything so that blocks and marks take their clicks.
        let every = TICK_STEPS
            .into_iter()
            .find(|s| *s as f32 * timeline.pixels_per_second >= TICK_ROOM)
            .unwrap_or(LONGEST_TICK);
        let mut ticks = Vec::new();
        let mut second = 0_u32;
        while second as f32 * timeline.pixels_per_second < width {
            let label = Text::new(clock(second)).style(TextStyle::Callout).foreground(Semantic::InkMuted);
            let label = cx.child(&label);
            cx.semantics(label, Semantics::hidden());
            ticks.push(cx.container(
                LayoutStyle {
                    absolute: Some((second as f32 * timeline.pixels_per_second + 4.0, 4.0)),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[label],
                Paint::None,
            ));
            second += every;
        }
        let ruler = cx.container(
            LayoutStyle {
                absolute: Some((0.0, 0.0)),
                width: Some(width),
                height: Some(RULER_HEIGHT),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &ticks,
            Paint::None,
        );
        if let Some(seek) = timeline.on_seek.clone() {
            cx.on_drag(
                ruler,
                Rc::new(move |drag: Drag| {
                    let x = (drag.position.x - drag.frame.x()).max(0.0);
                    seek((x / per_ms) as u64);
                }),
            );
            cx.cursor(ruler, CursorIcon::Pointer);
        }
        children.push(ruler);

        let mut y = RULER_HEIGHT;
        for track in &timeline.tracks {
            let line = cx.container(
                LayoutStyle {
                    absolute: Some((0.0, y)),
                    width: Some(width),
                    height: Some(1.0),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[],
                Paint::Quad(QuadPaint::new(theme.color(Semantic::Border))),
            );
            cx.semantics(line, Semantics::hidden());
            children.push(line);
            for item in &track.items {
                let frame = Rect::new(
                    item.start_ms as f32 * per_ms,
                    y + 4.0,
                    item.duration_ms as f32 * per_ms - 2.0,
                    track.height - 8.0,
                );
                children.push(cx.child(&Block {
                    item,
                    frame,
                    per_ms,
                    on_select: timeline.on_select.clone(),
                    on_edit: timeline.on_edit.clone(),
                }));
            }
            y += track.height;
        }

        for marker in &timeline.markers {
            let node = cx.container(
                LayoutStyle {
                    absolute: Some((
                        marker.at_ms as f32 * per_ms - MARKER / 2.0,
                        RULER_HEIGHT - MARKER / 2.0,
                    )),
                    width: Some(MARKER),
                    height: Some(MARKER),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[],
                Paint::Quad(QuadPaint {
                    corner_radius: MARKER / 2.0,
                    ..QuadPaint::new(theme.color(Semantic::Warning))
                }),
            );
            cx.semantics(node, Semantics::new(Role::Button).name(marker.title.clone()));
            if let Some(seek) = timeline.on_seek.clone() {
                let at = marker.at_ms;
                cx.on_click(node, Rc::new(move || seek(at)));
                cx.focusable(node, MARKER / 2.0);
                cx.cursor(node, CursorIcon::Pointer);
            }
            children.push(node);
        }

        // The playhead, with the time where it can be read.
        let x = timeline.position_ms as f32 * per_ms;
        let needle = cx.container(
            LayoutStyle {
                absolute: Some((x - 1.0, 0.0)),
                width: Some(2.0),
                height: Some(height),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Danger))),
        );
        cx.semantics(needle, Semantics::hidden());
        children.push(needle);

        cx.container(
            LayoutStyle { width: Some(width), height: Some(height), ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

impl View for Timeline {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let heads = cx.child(&Heads(self));
        let rows = cx.child(&ScrollView::new((Rows(self),)).horizontal().grow(1.0));
        let node = cx.container(
            LayoutStyle {
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                height: Some(self.total_height()),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[heads, rows],
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Group));
        node
    }
}
