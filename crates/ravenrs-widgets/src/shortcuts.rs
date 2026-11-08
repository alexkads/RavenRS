//! What a screen's keys do: a sheet with all of them and a line of hints.

use ravenrs_core::{Insets, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, Semantics};
use crate::{Dialog, Kbd, View, util};

/// Width of a group of a sheet, so that as many as fit stand side by side.
const SHEET_GROUP_WIDTH: f32 = 280.0;
/// How much of the window the list of a sheet may take before it scrolls.
const SHEET_HEIGHT_SHARE: f32 = 0.7;
/// The height of a line of a sheet and of a group's title, to tell when it may not fit.
const SHEET_LINE: f32 = 30.0;

/// One group of a sheet, with what it holds as (keys, what they do).
#[derive(Clone, Debug)]
struct ShortcutGroup {
    title: String,
    rows: Vec<(String, String)>,
}

/// A dialog listing the keys of a screen, in groups. Escape, the backdrop and its X close it.
///
/// It shows what it is told, so write the keys as they read on this system:
/// [`Hotkey::label`](crate::Hotkey::label) does it for the keys an app has registered.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::ShortcutSheet;
///
/// let open = State::new(true);
/// let sheet = ShortcutSheet::new(open.binding())
///     .group("Navigate", [("←", "Previous photo"), ("→", "Next photo")])
///     .group("Rate", [("1–5", "Give a score"), ("0", "Take it away")]);
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ShortcutSheet {
    open: Binding<bool>,
    title: Option<String>,
    groups: Vec<ShortcutGroup>,
    width: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ShortcutSheet {
    /// A sheet shown while `open` is true.
    pub fn new(open: Binding<bool>) -> Self {
        Self { open, title: None, groups: Vec::new(), width: 2.0 * SHEET_GROUP_WIDTH + 100.0 }
    }

    /// Sets the title, instead of the one in [`Labels`](crate::Labels).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Adds a group of `(keys, what they do)`.
    pub fn group<K: Into<String>, D: Into<String>>(
        mut self,
        title: impl Into<String>,
        rows: impl IntoIterator<Item = (K, D)>,
    ) -> Self {
        let rows = rows.into_iter().map(|(keys, does)| (keys.into(), does.into())).collect();
        self.groups.push(ShortcutGroup { title: title.into(), rows });
        self
    }

    /// Sets the width of the dialog in logical pixels; groups go side by side as they fit.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// The groups inside the dialog.
struct SheetBody {
    groups: Vec<ShortcutGroup>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SheetBody {
    fn row(cx: &mut BuildCx<'_>, keys: &str, does: &str) -> NodeId {
        let theme = *cx.theme();
        let cap = cx.child(&Kbd::new(keys));
        // A column for the keys, so that what they do starts in the same place.
        let cap = cx.container(
            LayoutStyle { min_width: Some(64.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[cap],
            Paint::None,
        );
        let does = util::label(cx, does, TextStyle::Callout, None, theme.color(Semantic::InkMuted));
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                align: Align::Center,
                min_height: Some(SHEET_LINE - theme.spacing.s1),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[cap, does],
            Paint::None,
        )
    }
}

impl View for SheetBody {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut groups = Vec::with_capacity(self.groups.len());
        for group in &self.groups {
            let title = util::label(
                cx,
                &group.title,
                TextStyle::Caption,
                Some(Weight::SEMIBOLD),
                theme.color(Semantic::InkMuted),
            );
            cx.semantics(title, Semantics::new(Role::Heading).level(3));
            let mut column = vec![title];
            for (keys, does) in &group.rows {
                column.push(Self::row(cx, keys, does));
            }
            groups.push(cx.container(
                LayoutStyle {
                    width: Some(SHEET_GROUP_WIDTH),
                    spacing: theme.spacing.s1,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &column,
                Paint::None,
            ));
        }
        let lines: usize = self.groups.iter().map(|g| g.rows.len() + 1).sum();
        let room = cx.viewport().height * SHEET_HEIGHT_SHARE;
        // Scrolling only when it could not fit even in one column, since a box that scrolls
        // has a fixed height and would leave a short list in empty space.
        let scrolls = lines as f32 * SHEET_LINE > room;
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s5,
                wrap: true,
                height: scrolls.then_some(room),
                overflow_hidden: scrolls,
                padding: Insets::ZERO,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &groups,
            Paint::None,
        );
        if scrolls {
            cx.scrollable(node);
            cx.semantics(node, Semantics::new(Role::ScrollView));
        }
        node
    }
}

impl View for ShortcutSheet {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let title = self.title.clone().unwrap_or_else(|| cx.labels().shortcut_sheet.clone());
        // The groups are copied into the dialog only while it shows.
        let groups = if self.open.get() { self.groups.clone() } else { Vec::new() };
        let dialog = Dialog::new(self.open.clone(), title).width(self.width).content(SheetBody { groups });
        cx.child(&dialog)
    }
}

/// A line of key hints under what they act on: `← → move · 1–5 rate`.
///
/// ```
/// use ravenrs_widgets::ShortcutHintBar;
///
/// let hints = ShortcutHintBar::new()
///     .leading("3 of 40")
///     .hint(["←", "→"], "move")
///     .hint(["1", "5"], "rate");
/// ```
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ShortcutHintBar {
    leading: Option<String>,
    hints: Vec<(Vec<String>, String)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ShortcutHintBar {
    /// A line with no hints yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Text before the hints, such as where the selection is.
    pub fn leading(mut self, text: impl Into<String>) -> Self {
        self.leading = Some(text.into());
        self
    }

    /// Adds the caps of `keys`, followed by what they do.
    pub fn hint<K: Into<String>>(
        mut self,
        keys: impl IntoIterator<Item = K>,
        does: impl Into<String>,
    ) -> Self {
        self.hints.push((keys.into_iter().map(Into::into).collect(), does.into()));
        self
    }
}

impl View for ShortcutHintBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let muted = theme.color(Semantic::InkMuted);
        let mut children = Vec::with_capacity(self.hints.len() + 1);
        if let Some(text) = &self.leading {
            children.push(util::label(cx, text, TextStyle::Caption, None, muted));
        }
        for (keys, does) in &self.hints {
            let mut hint: Vec<NodeId> = keys.iter().map(|key| cx.child(&Kbd::new(key.clone()))).collect();
            hint.push(util::label(cx, does, TextStyle::Caption, None, muted));
            children.push(cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s1,
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &hint,
                Paint::None,
            ));
        }
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s3,
                align: Align::Center,
                wrap: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        )
    }
}
