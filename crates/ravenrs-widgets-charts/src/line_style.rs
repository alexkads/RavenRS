//! `LineStylePicker`: how wide a line is, the pattern it is drawn in and the caps at its ends,
//! with a sample of the line as it will look.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{PathFill, Point, Rect, Role, Scene};
use ravenrs_graphics::flow::{self as geometry, EdgeCap, LineDash};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::Semantic;

use crate::context::{BuildCx, NodeId, Paint, Semantics};
use crate::{FormField, Select, View, util};

/// The style of one or more lines, as a [`LineStylePicker`] edits it: each part is `None`
/// when the lines it stands for do not share it (it shows as mixed, and stays as each line
/// has it until one is picked).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineStyle {
    /// How wide, in units of the drawing.
    pub width: Option<f32>,
    /// The pattern.
    pub dash: Option<LineDash>,
    /// The cap where the line starts.
    pub start: Option<EdgeCap>,
    /// The cap where the line ends.
    pub end: Option<EdgeCap>,
}

impl LineStyle {
    /// A solid line `width` wide, with these caps.
    pub fn new(width: f32, dash: LineDash, start: EdgeCap, end: EdgeCap) -> Self {
        Self { width: Some(width), dash: Some(dash), start: Some(start), end: Some(end) }
    }

    /// The style these lines share: a part they do not all have alike is `None`.
    pub fn shared(styles: impl IntoIterator<Item = Self>) -> Self {
        let mut styles = styles.into_iter();
        let Some(first) = styles.next() else { return Self::default() };
        styles.fold(first, |all, s| Self {
            width: all.width.filter(|w| s.width.is_some_and(|v| (v - w).abs() < 0.01)),
            dash: all.dash.filter(|d| s.dash == Some(*d)),
            start: all.start.filter(|c| s.start == Some(*c)),
            end: all.end.filter(|c| s.end == Some(*c)),
        })
    }
}

/// The widths offered by default, in units of the drawing.
pub const LINE_WIDTHS: [f32; 7] = [0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

/// Picks how wide a line is, its pattern and, unless [`Self::caps`] turns them off, the caps at
/// its ends, over a sample of the line drawn as picked. A part the lines being edited do not
/// share shows as mixed until it is picked.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{EdgeCap, LineDash, LineStyle, LineStylePicker};
///
/// let style = State::new(LineStyle::new(1.5, LineDash::Solid, EdgeCap::None, EdgeCap::Arrow));
/// let picker = LineStylePicker::new(style.binding());
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LineStylePicker {
    value: Binding<LineStyle>,
    caps: bool,
    widths: Vec<f32>,
    disabled: bool,
    label: Option<String>,
}

impl fmt::Debug for LineStylePicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineStylePicker").field("caps", &self.caps).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LineStylePicker {
    /// Edits `value`, with the caps and the widths of [`LINE_WIDTHS`].
    pub fn new(value: Binding<LineStyle>) -> Self {
        Self { value, caps: true, widths: LINE_WIDTHS.to_vec(), disabled: false, label: None }
    }

    /// Shows the caps at the ends (on by default), or only the width and the pattern, for a
    /// line with no ends such as an outline.
    pub fn caps(mut self, caps: bool) -> Self {
        self.caps = caps;
        self
    }

    /// Offers these widths instead.
    pub fn widths(mut self, widths: impl IntoIterator<Item = f32>) -> Self {
        self.widths = widths.into_iter().filter(|w| *w > 0.0).collect();
        self
    }

    /// Turns it off: it fades and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The name screen readers announce for the group ("Line style" by default).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A part of the style as a [`Select::of`] binding: `None` while mixed, set on all lines once
/// picked.
fn part<T: Clone + PartialEq + 'static>(
    value: &Binding<LineStyle>,
    read: fn(&LineStyle) -> Option<T>,
    write: fn(&mut LineStyle, T),
) -> Binding<Option<T>> {
    let (reader, writer) = (value.clone(), value.clone());
    Binding::new(
        move || read(&reader.get()),
        move |picked: Option<T>| {
            if let Some(picked) = picked {
                writer.update(|s| write(s, picked));
            }
        },
    )
}

/// The choices of a part: its options, after a "mixed" one while the lines do not share it.
fn choices<T: Clone>(mixed: Option<&str>, options: Vec<(T, String)>) -> Vec<(Option<T>, String)> {
    let first = mixed.map(|words| (None, words.to_owned()));
    first.into_iter().chain(options.into_iter().map(|(v, words)| (Some(v), words))).collect()
}

impl View for LineStylePicker {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let style = self.value.get();
        let mixed = labels.line_style_mixed.as_str();
        let mut rows = Vec::new();

        // The sample: the line as picked, from left to right.
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let sample = style;
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            let y = frame.center().y;
            let (a, b) = (Point::new(frame.x() + 12.0, y), Point::new(frame.max_x() - 12.0, y));
            let width = sample.width.unwrap_or(1.5).clamp(0.5, 8.0);
            let color = if sample.width.is_some() && sample.dash.is_some() { ink } else { muted };
            let cap = 8.0_f32.max(width * 3.0);
            let (start, end) = (sample.start.unwrap_or(EdgeCap::None), sample.end.unwrap_or(EdgeCap::None));
            let line = [Point::new(a.x + start.inset(cap), y), Point::new(b.x - end.inset(cap), y)];
            let pattern = sample.dash.unwrap_or(LineDash::Solid).pattern(width);
            for piece in geometry::pattern_dashes(&line, &pattern, 0.0) {
                scene.push_path(PathFill::solid(util::stroke_outline(&piece, width), color));
            }
            for (cap_kind, tip, direction) in
                [(end, b, Point::new(1.0, 0.0)), (start, a, Point::new(-1.0, 0.0))]
            {
                let outline = geometry::cap_outline(cap_kind, tip, direction, cap);
                if cap_kind.filled() {
                    crate::flow::fill(scene, outline, color);
                } else if !outline.is_empty() {
                    let points = geometry::flatten(&outline);
                    scene.push_path(PathFill::solid(util::stroke_outline(&points, width), color));
                }
            }
        }));
        let shown = cx.container(
            LayoutStyle { height: Some(32.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() },
            &[],
            paint,
        );
        cx.semantics(shown, Semantics::hidden());
        rows.push(shown);

        let widths: Vec<(f32, String)> = self.widths.iter().map(|w| (*w, format!("{w} px"))).collect();
        let width = part(&self.value, |s| s.width, |s, w| s.width = Some(w));
        let width_mixed = style.width.is_none().then_some(mixed);
        let width = Select::of(width, choices(width_mixed, widths)).fill().disabled(self.disabled);
        rows.push(cx.child(&FormField::new(labels.line_style_width.clone(), width)));

        let dashes: Vec<(LineDash, String)> =
            LineDash::ALL.into_iter().zip(labels.line_dash_names.iter().cloned()).collect();
        let dash = part(&self.value, |s| s.dash, |s, d| s.dash = Some(d));
        let dash_mixed = style.dash.is_none().then_some(mixed);
        let dash = Select::of(dash, choices(dash_mixed, dashes)).fill().disabled(self.disabled);
        rows.push(cx.child(&FormField::new(labels.line_style_dash.clone(), dash)));

        if self.caps {
            let caps: Vec<(EdgeCap, String)> =
                EdgeCap::ALL.into_iter().zip(labels.flow_cap_names.iter().cloned()).collect();
            let start = part(&self.value, |s| s.start, |s, c| s.start = Some(c));
            let start_mixed = style.start.is_none().then_some(mixed);
            let start =
                Select::of(start, choices(start_mixed, caps.clone())).fill().searchable().disabled(self.disabled);
            let end = part(&self.value, |s| s.end, |s, c| s.end = Some(c));
            let end_mixed = style.end.is_none().then_some(mixed);
            let end = Select::of(end, choices(end_mixed, caps)).fill().searchable().disabled(self.disabled);
            let start = cx.child(&FormField::new(labels.line_style_start.clone(), start));
            let end = cx.child(&FormField::new(labels.line_style_end.clone(), end));
            let pair = [start, end].map(|c| {
                cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Vertical) }, &[c], Paint::None)
            });
            rows.push(cx.container(
                LayoutStyle { spacing: theme.spacing.s3, ..LayoutStyle::stack(Axis::Horizontal) },
                &pair,
                Paint::None,
            ));
        }
        let root = cx.container(
            LayoutStyle { spacing: theme.spacing.s2, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.line_style.clone());
        cx.semantics(root, Semantics::new(Role::Group).name(name));
        root
    }
}
