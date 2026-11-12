//! Choosing several values, shown as tags.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Key, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::floating::{fuzzy_match, search_row};
use crate::overlay::panel;
use crate::{ButtonSize, IconButton, IconName, LookupItem, TextField, View, util};

/// The options shown at a time; a search reaches the others.
const MOST_SHOWN: usize = 50;

/// Picks several values from a list, showing each as a tag with a button to take it
/// off. The list searches as you type; Enter picks or takes off the highlighted
/// option, Backspace on the empty search takes off the last tag, Escape closes.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{LookupItem, TagBox, Ui};
///
/// let tags = State::new(vec!["red".to_owned()]);
/// let field = TagBox::new(
///     tags.binding(),
///     vec![LookupItem::new("red", "Red"), LookupItem::new("blue", "Blue")],
/// );
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TagBox {
    selected: Binding<Vec<String>>,
    options: Vec<LookupItem>,
    placeholder: Option<String>,
    allow_new: bool,
    most: Option<usize>,
    width: f32,
    fill: bool,
    label: Option<String>,
    state: util::EditorState,
}

impl fmt::Debug for TagBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TagBox").field("options", &self.options.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TagBox {
    /// A field bound to the keys chosen, in the order they were chosen.
    pub fn new(selected: Binding<Vec<String>>, options: Vec<LookupItem>) -> Self {
        Self {
            selected,
            options,
            placeholder: None,
            allow_new: false,
            most: None,
            width: 280.0,
            fill: false,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// Text shown while nothing is chosen.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Lets people add what they typed when it is not in the list. The text becomes the key.
    pub fn allow_new(mut self, allow: bool) -> Self {
        self.allow_new = allow;
        self
    }

    /// The most values that can be chosen.
    pub fn most(mut self, most: usize) -> Self {
        self.most = Some(most.max(1));
        self
    }

    /// Takes the width of its parent, as other fields of a form do.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Width in logical pixels. 280 by default; tags that do not fit go to a new line.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the field off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the values without letting them change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }

    fn caption(&self, key: &str) -> String {
        self.options.iter().find(|o| o.key == key).map_or_else(|| key.to_owned(), |o| o.label.clone())
    }
}

/// Picks `key` if it is not chosen and there is room; takes it off if it is.
fn flip(chosen: &mut Vec<String>, key: &str, most: Option<usize>) {
    if let Some(at) = chosen.iter().position(|k| k == key) {
        chosen.remove(at);
    } else if most.is_none_or(|most| chosen.len() < most) {
        chosen.push(key.to_owned());
    }
}

struct TagPanel<'a> {
    field: &'a TagBox,
    open: State<bool>,
}

impl View for TagPanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let query: State<String> = cx.use_state(String::new);
        let highlight: State<usize> = cx.use_state(|| 0);
        let q = query.get();
        let chosen = self.field.selected.get();
        let typed = q.trim().to_owned();
        let mut shown: Vec<LookupItem> = self
            .field
            .options
            .iter()
            .filter(|o| fuzzy_match(&o.label, &q))
            .take(MOST_SHOWN)
            .cloned()
            .collect();
        let exists = self.field.options.iter().any(|o| o.label.eq_ignore_ascii_case(&typed))
            || chosen.iter().any(|k| k.eq_ignore_ascii_case(&typed));
        if self.field.allow_new && !typed.is_empty() && !exists {
            shown.push(LookupItem::new(typed.clone(), cx.labels().tag_add(&typed)));
        }
        let hl = highlight.get().min(shown.len().saturating_sub(1));
        let shown = Rc::new(shown);
        let most = self.field.most;
        let pick = {
            let (selected, shown, query) = (self.field.selected.clone(), shown.clone(), query.clone());
            Rc::new(move |i: usize| {
                if let Some(item) = shown.get(i) {
                    selected.update(|chosen| flip(chosen, &item.key, most));
                    query.set(String::new());
                }
            })
        };
        let count = shown.len();
        let (h, p, open, selected, empty) =
            (highlight.clone(), pick.clone(), self.open.clone(), self.field.selected.clone(), q.is_empty());
        let search = cx.child(
            &TextField::new(query.binding())
                .placeholder(cx.labels().search.clone())
                .icon(IconName::Search)
                .compact()
                .autofocus()
                .intercept_keys(Rc::new(move |key, _| {
                    match key {
                        Key::ArrowDown if count > 0 => h.update(|x| *x = (*x + 1).min(count - 1)),
                        Key::ArrowUp => h.update(|x| *x = x.saturating_sub(1)),
                        Key::Enter if count > 0 => p(h.get().min(count - 1)),
                        Key::Enter => {}
                        Key::Escape => open.set(false),
                        Key::Backspace if empty => selected.update(|chosen| {
                            chosen.pop();
                        }),
                        _ => return false,
                    }
                    true
                })),
        );
        let mut rows = vec![search];
        for (i, item) in shown.iter().enumerate() {
            let p = pick.clone();
            let on = chosen.contains(&item.key);
            rows.push(search_row(cx, &item.label, on, i == hl, Rc::new(move || p(i))));
        }
        if shown.is_empty() {
            let note = cx.labels().no_results.clone();
            rows.push(util::label(cx, &note, TextStyle::Callout, None, theme.color(Semantic::InkMuted)));
        }
        let node = panel(cx, &rows, 6.0, self.field.width);
        cx.semantics(node, Semantics::new(Role::ListBox));
        node
    }
}

impl View for TagBox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<bool> = cx.use_state(|| false);
        let state = self.state;
        let is_open = open.get() && state.accepts_input();
        let hovered = cx.is_hovered() && state.accepts_input();
        let chosen = self.selected.get();
        let ink = state.ink(theme.color(Semantic::Ink));
        let mut tags = Vec::with_capacity(chosen.len());
        for key in &chosen {
            let caption = self.caption(key);
            let text = util::label(cx, &caption, TextStyle::Callout, None, ink);
            cx.semantics(text, Semantics::hidden());
            let mut parts = vec![text];
            if state.accepts_input() {
                let (selected, key) = (self.selected.clone(), key.clone());
                let name = format!("{} {caption}", cx.labels().tag_remove);
                parts.push(
                    cx.keyed_child(
                        util::hash_text(&key),
                        &IconButton::new(IconName::X)
                            .label(name)
                            .size(ButtonSize::Small)
                            .on_click(move || selected.update(|chosen| chosen.retain(|k| *k != key))),
                    ),
                );
            }
            let tag = cx.container(
                LayoutStyle {
                    padding: Insets { left: 8.0, right: 2.0, top: 2.0, bottom: 2.0 },
                    spacing: 2.0,
                    align: Align::Center,
                    min_height: Some(26.0),
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &parts,
                Paint::Quad(QuadPaint {
                    corner_radius: theme.radii.sm,
                    ..QuadPaint::new(theme.color(Semantic::Ink).with_alpha(0.08))
                }),
            );
            tags.push(tag);
        }
        if chosen.is_empty() {
            let hint = self.placeholder.clone().unwrap_or_else(|| cx.labels().select.clone());
            let muted = state.ink(theme.color(Semantic::InkMuted));
            tags.push(util::label(cx, &hint, TextStyle::Body, None, muted));
        }
        let holder = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                spacing: 4.0,
                wrap: true,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &tags,
            Paint::None,
        );
        let chevron = util::icon(cx, IconName::ChevronDown, 16.0, state.ink(theme.color(Semantic::InkMuted)));
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: state.ink(if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base }),
            }
        };
        let node = cx.container(
            LayoutStyle {
                width: (!self.fill).then_some(self.width),
                min_height: Some(40.0),
                padding: Insets { left: 6.0, right: 12.0, top: 6.0, bottom: 6.0 },
                spacing: 8.0,
                align: Align::Center,
                align_self: Some(if self.fill { Align::Stretch } else { Align::Fit }),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[holder, chevron],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
            }),
        );
        let value = chosen.iter().map(|k| self.caption(k)).collect::<Vec<_>>().join(", ");
        let mut semantics = Semantics::new(Role::ComboBox).value(value).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }
        cx.focusable(node, theme.radii.md);
        if !state.accepts_input() {
            return node;
        }
        let toggle = open.clone();
        cx.on_click(node, Rc::new(move || toggle.update(|o| *o = !*o)));
        let (opener, selected) = (open.clone(), self.selected.clone());
        cx.on_key(
            node,
            Rc::new(move |key, _| match key {
                Key::ArrowDown | Key::ArrowUp => {
                    opener.set(true);
                    true
                }
                Key::Backspace => {
                    selected.update(|chosen| {
                        chosen.pop();
                    });
                    true
                }
                _ => false,
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close = open.clone();
            let list = TagPanel { field: self, open: open.clone() };
            cx.overlay(node, Placement::Below, &list, Some(Rc::new(move || close.set(false))));
        } else if was_open.replace(false) {
            cx.request_focus();
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flipping_takes_off_what_is_on_and_respects_the_limit() {
        let mut chosen = vec!["a".to_owned()];
        flip(&mut chosen, "b", Some(2));
        assert_eq!(chosen, ["a", "b"]);
        flip(&mut chosen, "c", Some(2));
        assert_eq!(chosen, ["a", "b"], "no room");
        flip(&mut chosen, "a", Some(2));
        assert_eq!(chosen, ["b"]);
    }
}
