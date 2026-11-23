//! Data display: lists and tables.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{AnyView, Avatar, IconName, Tone, View, ViewSeq, util};

/// How tall a row of a [`List::compact`] is, in logical pixels.
const COMPACT_ROW: f32 = 48.0;

/// What a [`List::compact`] tells the rows built inside it.
struct CompactRows;

/// A bordered list; rows are separated by hairlines.
pub struct List<C> {
    rows: C,
    compact: bool,
}

impl<C> fmt::Debug for List<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("List")
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: ViewSeq> List<C> {
    /// A list of `rows`, usually [`ListItem`]s.
    pub fn new(rows: C) -> Self {
        Self { rows, compact: false }
    }

    /// Rows of 48 from edge to edge, for a list of stages or a queue: the icon without its tile
    /// and in the color of its [`ListItem::tone`], the subtitle beside the title, and no arrow.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl<C: ViewSeq> View for List<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let outer = self.compact.then(|| cx.replace_scoped(Some(Rc::new(CompactRows))));
        let rows = cx.children(&self.rows);
        if let Some(outer) = outer {
            cx.replace_scoped::<CompactRows>(outer);
        }
        let mut children = Vec::with_capacity(rows.len() * 2);
        for (i, row) in rows.into_iter().enumerate() {
            if i > 0 {
                let line = cx.child(&crate::Divider::horizontal());
                cx.semantics(line, Semantics::hidden());
                children.push(line);
            }
            cx.semantics_if_none(row, Semantics::new(Role::ListItem));
            children.push(row);
        }
        // Compact rows reach the hairline around them.
        let style = LayoutStyle {
            padding: Insets::uniform(if self.compact { 1.0 } else { 4.0 }),
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let paint = QuadPaint {
            corner_radius: theme.radii.lg,
            border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
            ..QuadPaint::new(theme.color(Semantic::Surface))
        };
        let node = cx.container(style, &children, Paint::Quad(paint));
        cx.semantics(node, Semantics::new(Role::List));
        node
    }
}

enum Leading {
    None,
    Icon(IconName),
    Avatar(String),
}

/// A list row: leading icon or avatar, title, subtitle, trailing text and an action.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ListItem {
    title: String,
    subtitle: Option<String>,
    leading: Leading,
    tone: Option<Tone>,
    trailing: Option<String>,
    action: Option<AnyView>,
    selected: bool,
    on_click: Option<Rc<dyn Fn()>>,
}

impl fmt::Debug for ListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListItem").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ListItem {
    /// A row with this title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            leading: Leading::None,
            tone: None,
            trailing: None,
            action: None,
            selected: false,
            on_click: None,
        }
    }

    /// A muted second line under the title.
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    /// A leading icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.leading = Leading::Icon(icon);
        self
    }

    /// A leading avatar showing the initials of `name`.
    pub fn avatar(mut self, name: impl Into<String>) -> Self {
        self.leading = Leading::Avatar(name.into());
        self
    }

    /// Muted text at the end of the row.
    pub fn trailing(mut self, text: impl Into<String>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    /// The color of the leading icon, for what the row's state means: a stage that passed in
    /// [`Tone::Success`], a build that waits in [`Tone::Warning`].
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// A view at the very end of the row, usually a small button: "Turn the plugin on".
    pub fn action(mut self, view: impl View + 'static) -> Self {
        self.action = Some(Box::new(view));
        self
    }

    /// Highlights the row as selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Runs `handler` when the row is clicked; also makes it hoverable.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl View for ListItem {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let compact = cx.scoped::<CompactRows>().is_some();
        let hovered = cx.is_hovered() && self.on_click.is_some();
        let icon_color = self.tone.map_or(theme.color(Semantic::InkMuted), |tone| tone.color(&theme));
        let mut children = Vec::new();
        match &self.leading {
            Leading::None => {}
            // In a compact list the icon stands alone, in the color of the row's state.
            Leading::Icon(icon) if compact => children.push(util::icon(cx, *icon, 16.0, icon_color)),
            Leading::Icon(icon) => {
                let i = util::icon(cx, *icon, 18.0, icon_color);
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
            let attrs = util::attrs(&theme, TextStyle::Callout, None);
            let muted = theme.color(Semantic::InkMuted);
            // Beside the title, a long subtitle ends in an ellipsis instead of a second line.
            text.push(if compact {
                cx.text_truncated(
                    s,
                    &attrs,
                    muted,
                    LayoutStyle { min_width: Some(0.0), ..LayoutStyle::leaf() },
                )
            } else {
                util::label(cx, s, TextStyle::Callout, None, muted)
            });
        }
        children.push(cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                spacing: if compact { 12.0 } else { 2.0 },
                align: if compact { Align::Center } else { Align::Start },
                ..LayoutStyle::stack(if compact { Axis::Horizontal } else { Axis::Vertical })
            },
            &text,
            Paint::None,
        ));
        if let Some(t) = &self.trailing {
            // It keeps its width: the text beside it is what wraps in a narrow list.
            let attrs = util::attrs(&theme, TextStyle::Callout, None);
            let width = cx.measure_text(t, &attrs).width.ceil();
            let style = LayoutStyle { min_width: Some(width), ..LayoutStyle::leaf() };
            children.push(cx.text(t, &attrs, theme.color(Semantic::InkMuted), style));
        }
        if let Some(action) = &self.action {
            children.push(cx.child(action.as_ref()));
        }
        if self.on_click.is_some() && !compact {
            children.push(util::icon(cx, IconName::ChevronRight, 16.0, theme.color(Semantic::InkMuted)));
        }
        let fill = if self.selected {
            theme.color(Semantic::Control)
        } else if hovered {
            theme.color(Semantic::Ink).with_alpha(0.04)
        } else {
            Color::TRANSPARENT
        };
        let style = if compact {
            LayoutStyle {
                min_height: Some(COMPACT_ROW),
                padding: Insets::symmetric(0.0, 16.0),
                spacing: 12.0,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            }
        } else {
            LayoutStyle {
                padding: Insets::symmetric(10.0, 12.0),
                spacing: 12.0,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            }
        };
        let radius = if compact { 0.0 } else { 10.0 };
        let node = cx.container(
            style,
            &children,
            Paint::Quad(QuadPaint { corner_radius: radius, ..QuadPaint::new(fill) }),
        );
        // The title names the row; the subtitle and the words at its end describe it.
        let said: Vec<&str> = self.subtitle.iter().chain(&self.trailing).map(String::as_str).collect();
        let mut semantics =
            Semantics::new(if self.on_click.is_some() { Role::Button } else { Role::ListItem })
                .name(self.title.clone());
        if !said.is_empty() {
            semantics = semantics.description(said.join(" · "));
        }
        cx.semantics(node, semantics);
        if let Some(h) = &self.on_click {
            cx.on_click(node, h.clone());
            cx.focusable(node, radius);
            cx.cursor(node, CursorIcon::Pointer);
        }
        node
    }
}

/// A table column: title, relative width and alignment.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Column {
    title: String,
    weight: f32,
    align_end: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Column {
    /// A column with this header title, weight 1.0, left-aligned.
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Table {
    /// An empty table with these columns.
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self { columns: columns.into_iter().collect(), rows: Vec::new() }
    }

    /// Appends a row; cells map to columns in order.
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
                let header = self.header;
                let cell = cx.container(
                    LayoutStyle {
                        grow: col.weight,
                        width: Some(0.0),
                        min_width: Some(0.0),
                        justify: if col.align_end { Justify::End } else { Justify::Start },
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &[label],
                    Paint::None,
                );
                let semantics = if header {
                    Semantics::new(Role::ColumnHeader).name(col.title.clone())
                } else {
                    Semantics::new(Role::Cell)
                };
                cx.semantics(cell, semantics);
                cell
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
        cx.semantics(node, Semantics::new(Role::Row));
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
            let line = cx.child(&crate::Divider::horizontal());
            cx.semantics(line, Semantics::hidden());
            children.push(line);
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
        cx.semantics(node, Semantics::new(Role::Table));
        cx.clip(node);
        node
    }
}
