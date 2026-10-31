//! Data display: lists, tables and charts.

use std::fmt;
use std::rc::Rc;

use raven_core::{Border, Color, CursorIcon, Insets, LinearGradient, Point, Quad, Rect};
use raven_layout::{Align, Axis, Justify, LayoutStyle};
use raven_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::{Avatar, IconName, View, ViewSeq, util};

/// A bordered list; rows are separated by hairlines.
pub struct List<C> {
    rows: C,
}

impl<C> fmt::Debug for List<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("List")
    }
}

impl<C: ViewSeq> List<C> {
    pub fn new(rows: C) -> Self {
        Self { rows }
    }
}

impl<C: ViewSeq> View for List<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let rows = cx.children(&self.rows);
        let mut children = Vec::with_capacity(rows.len() * 2);
        for (i, row) in rows.into_iter().enumerate() {
            if i > 0 {
                children.push(cx.child(&crate::Divider::horizontal()));
            }
            children.push(row);
        }
        let style = LayoutStyle {
            padding: Insets::uniform(4.0),
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let paint = QuadPaint {
            corner_radius: theme.radii.lg,
            border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
            ..QuadPaint::new(theme.color(Semantic::Surface))
        };
        cx.container(style, &children, Paint::Quad(paint))
    }
}

enum Leading {
    None,
    Icon(IconName),
    Avatar(String),
}

/// A list row: leading icon or avatar, title, subtitle and trailing text.
pub struct ListItem {
    title: String,
    subtitle: Option<String>,
    leading: Leading,
    trailing: Option<String>,
    selected: bool,
    on_click: Option<Rc<dyn Fn()>>,
}

impl fmt::Debug for ListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListItem").field("title", &self.title).finish_non_exhaustive()
    }
}

impl ListItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            leading: Leading::None,
            trailing: None,
            selected: false,
            on_click: None,
        }
    }

    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.leading = Leading::Icon(icon);
        self
    }

    pub fn avatar(mut self, name: impl Into<String>) -> Self {
        self.leading = Leading::Avatar(name.into());
        self
    }

    pub fn trailing(mut self, text: impl Into<String>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl View for ListItem {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered() && self.on_click.is_some();
        let mut children = Vec::new();
        match &self.leading {
            Leading::None => {}
            Leading::Icon(icon) => {
                let i = util::icon(cx, *icon, 18.0, theme.color(Semantic::InkMuted));
                children.push(cx.container(
                    LayoutStyle {
                        width: Some(36.0),
                        height: Some(36.0),
                        align: Align::Center,
                        justify: Justify::Center,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[i],
                    Paint::Quad(QuadPaint {
                        corner_radius: 10.0,
                        ..QuadPaint::new(theme.color(Semantic::Control))
                    }),
                ));
            }
            Leading::Avatar(name) => children.push(cx.child(&Avatar::new(name.clone()))),
        }
        let mut text = vec![util::label(
            cx,
            &self.title,
            TextStyle::Body,
            Some(Weight::SEMIBOLD),
            theme.color(Semantic::Ink),
        )];
        if let Some(s) = &self.subtitle {
            text.push(util::label(cx, s, TextStyle::Callout, None, theme.color(Semantic::InkMuted)));
        }
        children.push(cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                spacing: 2.0,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &text,
            Paint::None,
        ));
        if let Some(t) = &self.trailing {
            children.push(util::label(cx, t, TextStyle::Callout, None, theme.color(Semantic::InkMuted)));
        }
        if self.on_click.is_some() {
            children.push(util::icon(cx, IconName::ChevronRight, 16.0, theme.color(Semantic::InkMuted)));
        }
        let fill = if self.selected {
            theme.color(Semantic::Control)
        } else if hovered {
            theme.color(Semantic::Ink).with_alpha(0.04)
        } else {
            Color::TRANSPARENT
        };
        let style = LayoutStyle {
            padding: Insets::symmetric(10.0, 12.0),
            spacing: 12.0,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let node = cx.container(
            style,
            &children,
            Paint::Quad(QuadPaint { corner_radius: 10.0, ..QuadPaint::new(fill) }),
        );
        if let Some(h) = &self.on_click {
            cx.on_click(node, h.clone());
            cx.focusable(node, 10.0);
            cx.cursor(node, CursorIcon::Pointer);
        }
        node
    }
}

/// A table column: title, relative width and alignment.
#[derive(Clone, Debug)]
pub struct Column {
    title: String,
    weight: f32,
    align_end: bool,
}

impl Column {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), weight: 1.0, align_end: false }
    }

    /// Relative width (default 1.0).
    pub fn weight(mut self, weight: f32) -> Self {
        self.weight = weight.max(0.1);
        self
    }

    /// Right-aligns the column, for numbers.
    pub fn numeric(mut self) -> Self {
        self.align_end = true;
        self
    }
}

/// A simple data table with a header, hover-highlighted rows and numeric columns.
#[derive(Clone, Debug)]
pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self { columns: columns.into_iter().collect(), rows: Vec::new() }
    }

    pub fn row(mut self, cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.rows.push(cells.into_iter().map(Into::into).collect());
        self
    }
}

struct TableRow<'a> {
    columns: &'a [Column],
    cells: &'a [String],
    header: bool,
}

impl View for TableRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let cells: Vec<NodeId> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, col)| {
                let text = if self.header {
                    col.title.to_uppercase()
                } else {
                    self.cells.get(i).cloned().unwrap_or_default()
                };
                let (style, color, weight) = if self.header {
                    (TextStyle::Caption, theme.color(Semantic::InkMuted), None)
                } else if i == 0 {
                    (TextStyle::Body, theme.color(Semantic::Ink), Some(Weight::SEMIBOLD))
                } else {
                    (TextStyle::Body, theme.color(Semantic::InkMuted), None)
                };
                let label = util::label(cx, &text, style, weight, color);
                cx.container(
                    LayoutStyle {
                        grow: col.weight,
                        width: Some(0.0),
                        min_width: Some(0.0),
                        justify: if col.align_end { Justify::End } else { Justify::Start },
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[label],
                    Paint::None,
                )
            })
            .collect();
        let hovered = !self.header && cx.is_hovered();
        let fill = if hovered { theme.color(Semantic::Ink).with_alpha(0.04) } else { Color::TRANSPARENT };
        let style = LayoutStyle {
            padding: Insets::symmetric(if self.header { 10.0 } else { 12.0 }, 16.0),
            spacing: 16.0,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let node = cx.container(style, &cells, Paint::Quad(QuadPaint::new(fill)));
        if !self.header {
            cx.cursor(node, CursorIcon::Default);
        }
        node
    }
}

impl View for Table {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut children = vec![cx.child(&TableRow { columns: &self.columns, cells: &[], header: true })];
        for (i, row) in self.rows.iter().enumerate() {
            children.push(cx.child(&crate::Divider::horizontal()));
            children.push(
                cx.keyed_child(i as u64, &TableRow { columns: &self.columns, cells: row, header: false }),
            );
        }
        let style = LayoutStyle {
            align: Align::Stretch,
            align_self: Some(Align::Stretch),
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let node = cx.container(
            style,
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        cx.clip(node);
        node
    }
}

/// A bar chart with labels under each bar and faint gridlines.
#[derive(Clone, Debug)]
pub struct BarChart {
    data: Vec<(String, f32)>,
    height: f32,
}

impl BarChart {
    pub fn new(data: impl IntoIterator<Item = (impl Into<String>, f32)>) -> Self {
        Self { data: data.into_iter().map(|(l, v)| (l.into(), v.max(0.0))).collect(), height: 180.0 }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl View for BarChart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let values: Vec<f32> = self.data.iter().map(|d| d.1).collect();
        let max = values.iter().copied().fold(0.0_f32, f32::max).max(f32::EPSILON);
        let (accent, grid) = (theme.color(Semantic::Accent), theme.color(Semantic::Border));
        let top = theme.color(Semantic::FocusRing).lerp(accent, 0.4);
        let bars = cx.container(
            LayoutStyle {
                height: Some(self.height),
                align_self: Some(Align::Stretch),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                for i in 0..=3 {
                    let y = f.y() + f.height() * i as f32 / 3.0;
                    scene.push_quad(Quad::new(
                        Rect::new(f.x(), y.min(f.max_y() - 1.0), f.width(), 1.0),
                        grid.with_alpha(0.6),
                    ));
                }
                let n = values.len().max(1) as f32;
                let slot = f.width() / n;
                let w = (slot * 0.56).min(44.0);
                for (i, v) in values.iter().enumerate() {
                    let h = (f.height() - 6.0) * v / max;
                    let x = f.x() + slot * i as f32 + (slot - w) / 2.0;
                    let rect = Rect::new(x, f.max_y() - h, w, h);
                    let fill = LinearGradient::vertical(top, accent);
                    scene.push_quad(Quad::new(rect, fill).radius(w.min(12.0) / 2.0));
                }
            })),
        );
        let labels: Vec<NodeId> = self
            .data
            .iter()
            .map(|(label, _)| {
                let l = util::label(cx, label, TextStyle::Caption, None, theme.color(Semantic::InkMuted));
                cx.container(
                    LayoutStyle {
                        grow: 1.0,
                        width: Some(0.0),
                        justify: Justify::Center,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[l],
                    Paint::None,
                )
            })
            .collect();
        let labels = cx.container(
            LayoutStyle { align_self: Some(Align::Stretch), ..LayoutStyle::stack(Axis::Horizontal) },
            &labels,
            Paint::None,
        );
        cx.container(
            LayoutStyle {
                spacing: 8.0,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[bars, labels],
            Paint::None,
        )
    }
}

/// A smooth area chart (sparkline style) of evenly spaced values.
#[derive(Clone, Debug)]
pub struct AreaChart {
    values: Vec<f32>,
    height: f32,
    tone: Semantic,
}

impl AreaChart {
    pub fn new(values: impl IntoIterator<Item = f32>) -> Self {
        Self { values: values.into_iter().collect(), height: 120.0, tone: Semantic::Accent }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The line color role (Accent by default; Success, Danger...).
    pub fn tone(mut self, tone: Semantic) -> Self {
        self.tone = tone;
        self
    }
}

impl View for AreaChart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let values = self.values.clone();
        let line = theme.color(self.tone);
        let canvas_color = theme.color(Semantic::SurfaceElevated);
        cx.container(
            LayoutStyle {
                height: Some(self.height),
                align_self: Some(Align::Stretch),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                if values.len() < 2 || f.width() < 2.0 {
                    return;
                }
                let (lo, hi) =
                    values.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
                let span = (hi - lo).max(f32::EPSILON);
                let pad = 6.0;
                let y_of = |v: f32| f.max_y() - pad - (f.height() - pad * 2.0) * (v - lo) / span;
                // Catmull-Rom style smoothing via cubic Hermite interpolation between samples.
                let sample = |x: f32| {
                    let t = x * (values.len() - 1) as f32;
                    let i = (t.floor() as usize).min(values.len() - 2);
                    let u = t - i as f32;
                    let p0 = values[i.saturating_sub(1)];
                    let (p1, p2) = (values[i], values[i + 1]);
                    let p3 = values[(i + 2).min(values.len() - 1)];
                    let (m1, m2) = ((p2 - p0) * 0.5, (p3 - p1) * 0.5);
                    let (u2, u3) = (u * u, u * u * u);
                    (2.0 * u3 - 3.0 * u2 + 1.0) * p1
                        + (u3 - 2.0 * u2 + u) * m1
                        + (-2.0 * u3 + 3.0 * u2) * p2
                        + (u3 - u2) * m2
                };
                let step = 2.0;
                let columns = (f.width() / step).ceil() as usize;
                let mut last = None;
                for c in 0..columns {
                    let x = f.x() + c as f32 * step;
                    let y = y_of(sample((c as f32 * step / f.width()).min(1.0)));
                    let fill = LinearGradient {
                        start: Point::new(0.0, 0.0),
                        end: Point::new(0.0, 1.0),
                        from: line.with_alpha(0.28),
                        to: line.with_alpha(0.0),
                    };
                    // Columns tile exactly (no overlap), so translucent fills do not stack into stripes.
                    let w = step.min(f.max_x() - x);
                    scene.push_quad(Quad::new(Rect::new(x, y, w, f.max_y() - y), fill));
                    last = Some((x, y));
                }
                for c in 0..columns {
                    let x = f.x() + c as f32 * step;
                    let y = y_of(sample((c as f32 * step / f.width()).min(1.0)));
                    let y2 = y_of(sample(((c + 1) as f32 * step / f.width()).min(1.0)));
                    let top = y.min(y2) - 1.0;
                    let h = (y - y2).abs() + 2.0;
                    scene.push_quad(Quad::new(Rect::new(x, top, step + 0.6, h), line).radius(1.0));
                }
                if let Some((x, y)) = last {
                    scene.push_quad(
                        Quad::new(Rect::new(x - 4.0, y - 4.0, 8.0, 8.0), line)
                            .radius(4.0)
                            .border(2.0, canvas_color),
                    );
                }
            })),
        )
    }
}
