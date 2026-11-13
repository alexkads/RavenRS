//! Choosing a record from a list too long to hold in memory.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Key, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::overlay::panel;
use crate::overlay::{fuzzy_match, search_row};
use crate::{ButtonSize, IconButton, IconName, TextField, View, util};

/// How many options a lookup asks for at a time, unless told otherwise.
const DEFAULT_LIMIT: usize = 50;
/// The least it can show, since it fills what it is in unless given a width.
const FIELD_MIN_WIDTH: f32 = 180.0;

/// An option of a [`LookupField`]: the key the app knows it by, and the text people see.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LookupItem {
    /// What identifies it, such as a primary key.
    pub key: String,
    /// What is shown.
    pub label: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LookupItem {
    /// An option with this key and text.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self { key: key.into(), label: label.into() }
    }
}

/// What a [`LookupField`] wants: up to `limit` options matching `query`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LookupRequest {
    /// What was typed in the search box; empty at first.
    pub query: String,
    /// The most options that will be shown.
    pub limit: usize,
}

/// Delivers the answer to a [`LookupRequest`], now or later on the UI thread. An
/// answer to a search that was since replaced is ignored.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LookupResponder {
    answers: State<Answer>,
    request: u64,
}

impl fmt::Debug for LookupResponder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LookupResponder").field("request", &self.request).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LookupResponder {
    /// Answers with these options.
    pub fn respond(&self, items: Vec<LookupItem>) {
        self.deliver(Loaded::Items(Rc::new(items)));
    }

    /// Answers that the options could not be loaded, in words for the user.
    pub fn fail(&self, message: impl Into<String>) {
        self.deliver(Loaded::Failed(message.into()));
    }

    fn deliver(&self, loaded: Loaded) {
        let current = self.answers.get();
        if current.request == self.request {
            self.answers.set(Answer { loaded, ..current });
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Loaded {
    Waiting,
    Items(Rc<Vec<LookupItem>>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
struct Answer {
    request: u64,
    query: String,
    loaded: Loaded,
}

type Loader = Rc<dyn Fn(LookupRequest, LookupResponder)>;

/// Picks a record from a list that is searched as you type, loaded from memory or
/// through a callback that can go to a database or a server. Arrows move through
/// the options, Enter picks and Escape closes. The chosen option can be cleared.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{LookupField, LookupItem, Ui};
///
/// let city = State::new(None);
/// let field = LookupField::remote(city.binding(), |request, responder| {
///     // Ask a database here; answer now, or later with a clone of the responder.
///     let found = LookupItem::new("1", format!("{} (first match)", request.query));
///     responder.respond(vec![found]);
/// });
/// let mut ui = Ui::new(field, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct LookupField {
    selected: Binding<Option<LookupItem>>,
    load: Loader,
    placeholder: Option<String>,
    limit: usize,
    width: Option<f32>,
    label: Option<String>,
    state: util::EditorState,
}

impl fmt::Debug for LookupField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LookupField").field("limit", &self.limit).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl LookupField {
    /// A lookup over options already in memory.
    pub fn new(selected: Binding<Option<LookupItem>>, items: Vec<LookupItem>) -> Self {
        Self::remote(selected, move |request, responder| {
            let found = items.iter().filter(|i| fuzzy_match(&i.label, &request.query));
            responder.respond(found.take(request.limit).cloned().collect());
        })
    }

    /// A lookup that asks `load` for its options when it opens and on every search.
    pub fn remote(
        selected: Binding<Option<LookupItem>>,
        load: impl Fn(LookupRequest, LookupResponder) + 'static,
    ) -> Self {
        Self {
            selected,
            load: Rc::new(load),
            placeholder: None,
            limit: DEFAULT_LIMIT,
            width: None,
            label: None,
            state: util::EditorState::default(),
        }
    }

    /// Text shown while nothing is chosen.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// The most options asked for at a time. 50 by default.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    /// Width in logical pixels. 240 by default.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The name screen readers announce. Inside a [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Turns the lookup off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the choice without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

struct LookupPanel<'a> {
    field: &'a LookupField,
    open: State<bool>,
}

impl View for LookupPanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let query: State<String> = cx.use_state(String::new);
        let highlight: State<usize> = cx.use_state(|| 0);
        let answers: State<Answer> =
            cx.use_state(|| Answer { request: 0, query: "\u{0}".into(), loaded: Loaded::Waiting });
        let q = query.get();
        if answers.peek().query != q {
            let request = answers.peek().request + 1;
            answers.set(Answer { request, query: q.clone(), loaded: Loaded::Waiting });
            highlight.set_if_changed(0);
            (self.field.load)(
                LookupRequest { query: q, limit: self.field.limit },
                LookupResponder { answers: answers.clone(), request },
            );
        }
        let loaded = answers.get().loaded;
        let items: Rc<Vec<LookupItem>> = match &loaded {
            Loaded::Items(items) => items.clone(),
            _ => Rc::new(Vec::new()),
        };
        let hl = highlight.get().min(items.len().saturating_sub(1));
        let pick = {
            let (selected, open, items) = (self.field.selected.clone(), self.open.clone(), items.clone());
            Rc::new(move |i: usize| {
                selected.set(items.get(i).cloned());
                open.set(false);
            })
        };
        let (h, count, p, open) = (highlight.clone(), items.len(), pick.clone(), self.open.clone());
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
                        _ => return false,
                    }
                    true
                })),
        );
        let mut rows = vec![search];
        let current = self.field.selected.get();
        for (i, item) in items.iter().enumerate() {
            let p = pick.clone();
            let selected = current.as_ref().is_some_and(|c| c.key == item.key);
            rows.push(search_row(cx, &item.label, selected, i == hl, Rc::new(move || p(i))));
        }
        let note = match &loaded {
            Loaded::Waiting => Some(cx.labels().loading.clone()),
            Loaded::Failed(message) => Some(message.clone()),
            Loaded::Items(items) if items.is_empty() => Some(cx.labels().no_results.clone()),
            Loaded::Items(_) => None,
        };
        if let Some(note) = note {
            let tone =
                if matches!(loaded, Loaded::Failed(_)) { Semantic::Danger } else { Semantic::InkMuted };
            rows.push(util::label(cx, &note, TextStyle::Callout, None, theme.color(tone)));
        }
        let node = panel(cx, &rows, 6.0, self.field.width.unwrap_or(FIELD_MIN_WIDTH));
        cx.semantics(node, Semantics::new(Role::ListBox));
        node
    }
}

impl View for LookupField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<bool> = cx.use_state(|| false);
        let state = self.state;
        let is_open = open.get() && state.accepts_input();
        let hovered = cx.is_hovered() && state.accepts_input();
        let chosen = self.selected.get();
        let (text, color) = match &chosen {
            Some(item) => (item.label.clone(), theme.color(Semantic::Ink)),
            None => (
                self.placeholder.clone().unwrap_or_else(|| cx.labels().select.clone()),
                theme.color(Semantic::InkMuted),
            ),
        };
        let label = cx.text_truncated(
            &text,
            &util::attrs(&theme, TextStyle::Body, None),
            state.ink(color),
            LayoutStyle::leaf(),
        );
        let label = cx.container(
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[label],
            Paint::None,
        );
        let mut children = vec![label];
        if chosen.is_some() && state.accepts_input() {
            let selected = self.selected.clone();
            children.push(
                cx.child(
                    &IconButton::new(IconName::X)
                        .label(cx.labels().clear.clone())
                        .size(ButtonSize::Small)
                        .on_click(move || selected.set(None)),
                ),
            );
        }
        children.push(util::icon(
            cx,
            IconName::ChevronDown,
            16.0,
            state.ink(theme.color(Semantic::InkMuted)),
        ));
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
                width: self.width,
                min_width: Some(FIELD_MIN_WIDTH),
                height: Some(40.0),
                padding: Insets::symmetric(0.0, 12.0),
                spacing: 8.0,
                align: Align::Center,
                align_self: Some(if self.width.is_some() { Align::Fit } else { Align::Stretch }),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
            }),
        );
        cx.semantics(label, Semantics::hidden());
        let value = chosen.as_ref().map_or_else(String::new, |item| item.label.clone());
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
        let opener = open.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let opens = matches!(key, Key::ArrowDown | Key::ArrowUp);
                if opens {
                    opener.set(true);
                }
                opens
            }),
        );
        cx.cursor(node, CursorIcon::Pointer);
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close = open.clone();
            let list = LookupPanel { field: self, open: open.clone() };
            cx.overlay(node, Placement::Below, &list, Some(Rc::new(move || close.set(false))));
        } else if was_open.replace(false) {
            cx.request_focus();
        }
        node
    }
}
