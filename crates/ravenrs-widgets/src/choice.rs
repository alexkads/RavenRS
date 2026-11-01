//! Toggle buttons, toggle groups, pagination, labels and aspect-ratio boxes.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::{ButtonSize, ButtonStyle, IconName, View, util};

/// A button that stays pressed: bold, italic, "pin", "mute".
#[derive(Debug)]
pub struct ToggleButton {
    value: Binding<bool>,
    label: String,
    icon: Option<IconName>,
}

impl ToggleButton {
    pub fn new(value: Binding<bool>, label: impl Into<String>) -> Self {
        Self { value, label: label.into(), icon: None }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// The look shared by toggle buttons and toggle group items.
fn toggle_item(
    cx: &mut BuildCx<'_>,
    on: bool,
    icon: Option<IconName>,
    label: &str,
    on_click: Rc<dyn Fn()>,
) -> NodeId {
    let theme = *cx.theme();
    let hovered = cx.is_hovered();
    let ink = theme.color(Semantic::Ink);
    let color = if on || hovered { ink } else { theme.color(Semantic::InkMuted) };
    let mut children = Vec::new();
    if let Some(i) = icon {
        children.push(util::icon(cx, i, 16.0, color));
    }
    if !label.is_empty() {
        children.push(util::label(cx, label, TextStyle::Callout, Some(Weight::SEMIBOLD), color));
    }
    let fill = if on {
        theme.color(Semantic::Control)
    } else if hovered {
        ink.with_alpha(0.06)
    } else {
        Color::TRANSPARENT
    };
    let border = on.then(|| Border { width: 1.0, color: theme.color(Semantic::Border) });
    let icon_only = label.is_empty();
    let node = cx.container(
        LayoutStyle {
            height: Some(34.0),
            width: icon_only.then_some(34.0),
            padding: Insets::symmetric(0.0, if icon_only { 0.0 } else { 12.0 }),
            spacing: 6.0,
            align: Align::Center,
            justify: Justify::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &children,
        Paint::Quad(QuadPaint { corner_radius: 8.0, border, ..QuadPaint::new(fill) }),
    );
    cx.on_click(node, on_click);
    cx.focusable(node, 8.0);
    cx.cursor(node, CursorIcon::Pointer);
    node
}

impl View for ToggleButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let v = self.value.clone();
        toggle_item(cx, self.value.get(), self.icon, &self.label, Rc::new(move || v.update(|b| *b = !*b)))
    }
}

/// A row of toggle buttons where one (or several) can be on.
#[derive(Debug)]
pub struct ToggleGroup {
    items: Vec<(Option<IconName>, String)>,
    single: Option<Binding<usize>>,
    multiple: Option<Binding<BTreeSet<usize>>>,
}

impl ToggleGroup {
    /// Exactly one item is on (like text alignment).
    pub fn single(
        selected: Binding<usize>,
        items: impl IntoIterator<Item = (Option<IconName>, &'static str)>,
    ) -> Self {
        Self { items: collect(items), single: Some(selected), multiple: None }
    }

    /// Any number of items are on (like bold / italic / underline).
    pub fn multiple(
        selected: Binding<BTreeSet<usize>>,
        items: impl IntoIterator<Item = (Option<IconName>, &'static str)>,
    ) -> Self {
        Self { items: collect(items), single: None, multiple: Some(selected) }
    }
}

fn collect(
    items: impl IntoIterator<Item = (Option<IconName>, &'static str)>,
) -> Vec<(Option<IconName>, String)> {
    items.into_iter().map(|(i, l)| (i, l.to_owned())).collect()
}

struct GroupItem<'a> {
    index: usize,
    group: &'a ToggleGroup,
}

impl View for GroupItem<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let g = self.group;
        let i = self.index;
        let (icon, label) = &g.items[i];
        let (on, action): (bool, Rc<dyn Fn()>) = match (&g.single, &g.multiple) {
            (Some(b), _) => {
                let b2 = b.clone();
                (b.get() == i, Rc::new(move || b2.set(i)))
            }
            (_, Some(b)) => {
                let b2 = b.clone();
                (
                    b.get().contains(&i),
                    Rc::new(move || {
                        b2.update(|s| {
                            if !s.remove(&i) {
                                s.insert(i);
                            }
                        })
                    }),
                )
            }
            _ => (false, Rc::new(|| {})),
        };
        toggle_item(cx, on, *icon, label, action)
    }
}

impl View for ToggleGroup {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let items: Vec<NodeId> = (0..self.items.len())
            .map(|index| cx.keyed_child(index as u64, &GroupItem { index, group: self }))
            .collect();
        cx.container(
            LayoutStyle {
                spacing: 4.0,
                align: Align::Center,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
        )
    }
}

/// Page navigation: Previous, numbered pages with ellipses, Next.
#[derive(Debug)]
pub struct Pagination {
    page: Binding<usize>,
    pages: usize,
}

impl Pagination {
    /// `page` is zero-based; `pages` is the total count.
    pub fn new(page: Binding<usize>, pages: usize) -> Self {
        Self { page, pages: pages.max(1) }
    }
}

/// The page numbers to show (zero-based), with `None` for an ellipsis.
pub(crate) fn page_window(current: usize, pages: usize) -> Vec<Option<usize>> {
    if pages <= 7 {
        return (0..pages).map(Some).collect();
    }
    let mut out = vec![Some(0)];
    let (lo, hi) = if current <= 3 {
        (1, 4)
    } else if current >= pages - 4 {
        (pages - 5, pages - 2)
    } else {
        (current - 1, current + 1)
    };
    if lo > 1 {
        out.push(None);
    }
    out.extend((lo..=hi).map(Some));
    if hi < pages - 2 {
        out.push(None);
    }
    out.push(Some(pages - 1));
    out
}

impl View for Pagination {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let current = self.page.get().min(self.pages - 1);
        let mut items = Vec::new();
        let prev = self.page.clone();
        items.push(
            cx.child(
                &crate::Button::new("Previous")
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .icon(IconName::ChevronLeft)
                    .disabled(current == 0)
                    .on_click(move || prev.update(|p| *p = p.saturating_sub(1))),
            ),
        );
        for (k, entry) in page_window(current, self.pages).into_iter().enumerate() {
            match entry {
                Some(p) => {
                    let b = self.page.clone();
                    let style = if p == current { ButtonStyle::Secondary } else { ButtonStyle::Ghost };
                    let button = crate::Button::new((p + 1).to_string())
                        .style(style)
                        .size(ButtonSize::Small)
                        .on_click(move || b.set(p));
                    items.push(cx.keyed_child(p as u64, &button));
                }
                None => {
                    let dots = util::icon(cx, IconName::Ellipsis, 16.0, theme.color(Semantic::InkMuted));
                    items.push(cx.container(
                        LayoutStyle {
                            width: Some(28.0),
                            justify: Justify::Center,
                            ..LayoutStyle::stack(Axis::Horizontal)
                        },
                        &[dots],
                        Paint::None,
                    ));
                    let _ = k;
                }
            }
        }
        let (next, last) = (self.page.clone(), self.pages - 1);
        items.push(
            cx.child(
                &crate::Button::new("Next")
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .trailing_icon(IconName::ChevronRight)
                    .disabled(current == last)
                    .on_click(move || next.update(|p| *p = (*p + 1).min(last))),
            ),
        );
        cx.container(
            LayoutStyle {
                spacing: 4.0,
                align: Align::Center,
                wrap: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
        )
    }
}

/// A form label with an optional required marker and hint.
#[derive(Clone, Debug)]
pub struct Label {
    text: String,
    required: bool,
    hint: Option<String>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), required: false, hint: None }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl View for Label {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut row = vec![util::label(
            cx,
            &self.text,
            TextStyle::Callout,
            Some(Weight::SEMIBOLD),
            theme.color(Semantic::Ink),
        )];
        if self.required {
            row.push(util::label(
                cx,
                "*",
                TextStyle::Callout,
                Some(Weight::SEMIBOLD),
                theme.color(Semantic::Danger),
            ));
        }
        let row = cx.container(
            LayoutStyle { spacing: 3.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &row,
            Paint::None,
        );
        let mut children = vec![row];
        if let Some(h) = &self.hint {
            children.push(util::label(cx, h, TextStyle::Caption, None, theme.color(Semantic::InkMuted)));
        }
        cx.container(
            LayoutStyle { spacing: 2.0, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

/// Keeps its content at a width-to-height ratio while filling the width.
#[derive(Debug)]
pub struct AspectRatio<V> {
    ratio: f32,
    content: V,
}

impl<V: View> AspectRatio<V> {
    pub fn new(ratio: f32, content: V) -> Self {
        Self { ratio: ratio.max(0.01), content }
    }
}

impl<V: View> View for AspectRatio<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        cx.container(
            LayoutStyle {
                aspect_ratio: Some(self.ratio),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[child],
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::page_window;

    #[test]
    fn page_window_keeps_ends_and_neighbors() {
        let show = |c, n| {
            page_window(c, n)
                .iter()
                .map(|p| p.map_or("…".into(), |p: usize| (p + 1).to_string()))
                .collect::<Vec<String>>()
                .join(" ")
        };
        assert_eq!(show(0, 5), "1 2 3 4 5");
        assert_eq!(show(0, 20), "1 2 3 4 5 … 20");
        assert_eq!(show(9, 20), "1 … 9 10 11 … 20");
        assert_eq!(show(19, 20), "1 … 16 17 18 19 20");
    }
}
