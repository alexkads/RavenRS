//! A vertical timeline: each item a date, a round mark joined to the next by a thin line, a
//! title, a detail and a badge, in the order given.

use std::rc::Rc;

use ravenrs_core::{Key, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets_base::{Badge, BuildCx, NodeId, Paint, QuadPaint, Semantics, Text, Tone, VStack, View};

/// The width of the column the dates go in, in logical pixels.
const DATE_WIDTH: f32 = 96.0;
/// The side of the round mark.
const MARK: f32 = 10.0;

/// One moment of a [`Chronology`].
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ChronologyItem {
    /// What the app knows it by, handed to [`Chronology::on_open`].
    pub id: u64,
    /// When it happened, as the app writes it: `14/11/2026`, `Hoje 10:30`.
    pub when: String,
    /// What happened.
    pub title: String,
    /// A line more about it.
    pub detail: Option<String>,
    /// A word in a badge beside the title, such as a state.
    pub badge: Option<String>,
    /// The color of the mark and of the badge.
    pub tone: Tone,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ChronologyItem {
    /// A moment with its date and title, a neutral mark and nothing else.
    pub fn new(id: u64, when: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id, when: when.into(), title: title.into(), detail: None, badge: None, tone: Tone::Neutral }
    }

    /// A line more about it, under the title.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A word in a badge beside the title.
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// The color of the mark and of the badge.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

/// A list of moments down a line: the history of a person, the decisions on a request, the
/// corrections of a record. Up and Down move between them, and a click or Enter opens one.
///
/// ```
/// use ravenrs_widgets::{Chronology, ChronologyItem, Tone};
///
/// let history = Chronology::new(vec![
///     ChronologyItem::new(1, "02/03/2024", "Batismo").tone(Tone::Accent),
///     ChronologyItem::new(2, "10/08/2025", "Membro ativo").detail("Pela assembleia").badge("Aprovado"),
/// ])
/// .label("Marcos")
/// .on_open(|id| println!("open {id}"));
/// # let _ = history;
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Chronology {
    items: Vec<ChronologyItem>,
    label: Option<String>,
    on_open: Option<Rc<dyn Fn(u64)>>,
}

impl std::fmt::Debug for Chronology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chronology")
            .field("items", &self.items)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Chronology {
    /// The moments, shown in this order, the first at the top.
    pub fn new(items: Vec<ChronologyItem>) -> Self {
        Self { items, label: None, on_open: None }
    }

    /// The name screen readers announce for the list.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Called with the item's id when one is clicked or gets Enter.
    pub fn on_open(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl View for Chronology {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // The item the arrows sent the focus to, until it takes it.
        let goes_to: State<Option<usize>> = cx.use_state(|| None);
        let count = self.items.len();
        let rows: Vec<NodeId> = self
            .items
            .iter()
            .enumerate()
            .map(|(at, item)| {
                cx.child(&Row {
                    item: item.clone(),
                    at,
                    last: at + 1 == count,
                    count,
                    goes_to: goes_to.clone(),
                    on_open: self.on_open.clone(),
                })
            })
            .collect();
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        let mut semantics = Semantics::new(Role::List);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
        node
    }
}

/// One item, a view of its own so the arrows can hand it the focus.
struct Row {
    item: ChronologyItem,
    at: usize,
    last: bool,
    count: usize,
    goes_to: State<Option<usize>>,
    on_open: Option<Rc<dyn Fn(u64)>>,
}

impl View for Row {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        if self.goes_to.get().is_some_and(|to| to == self.at) {
            self.goes_to.set(None);
            cx.request_focus();
        }
        let item = &self.item;
        let hidden = |cx: &mut BuildCx<'_>, node: NodeId| {
            cx.semantics(node, Semantics::hidden());
            node
        };
        let when =
            cx.child(&Text::new(item.when.clone()).style(TextStyle::Caption).foreground(Semantic::InkMuted));
        let when = hidden(cx, when);
        let date = cx.container(
            LayoutStyle {
                width: Some(DATE_WIDTH),
                padding: ravenrs_core::Insets { top: 2.0, ..Default::default() },
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[when],
            Paint::None,
        );
        // The mark, and the line down to the next one.
        let mark = cx.container(
            LayoutStyle { width: Some(MARK), height: Some(MARK), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint { corner_radius: MARK / 2.0, ..QuadPaint::new(item.tone.color(&theme)) }),
        );
        let mut marks = vec![mark];
        if !self.last {
            marks.push(cx.container(
                LayoutStyle { width: Some(2.0), grow: 1.0, ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint::new(theme.color(Semantic::Border))),
            ));
        }
        let rail = cx.container(
            LayoutStyle {
                width: Some(16.0),
                spacing: 4.0,
                align: Align::Center,
                padding: ravenrs_core::Insets { top: 4.0, ..Default::default() },
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &marks,
            Paint::None,
        );
        let title = cx.child(&Text::new(item.title.clone()).font_weight(Weight::SEMIBOLD));
        let mut heading = vec![hidden(cx, title)];
        if let Some(badge) = &item.badge {
            let node = cx.child(&Badge::new(badge.clone()).tone(item.tone));
            heading.push(hidden(cx, node));
        }
        let heading = cx.container(
            LayoutStyle { spacing: 8.0, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &heading,
            Paint::None,
        );
        let mut content = vec![heading];
        if let Some(detail) = &item.detail {
            let node = cx.child(&VStack::new((Text::new(detail.clone()).foreground(Semantic::InkMuted),)));
            content.push(hidden(cx, node));
        }
        let content = cx.container(
            LayoutStyle {
                grow: 1.0,
                spacing: 2.0,
                padding: ravenrs_core::Insets { bottom: 16.0, ..Default::default() },
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &content,
            Paint::None,
        );
        let row = cx.container(
            LayoutStyle { spacing: 12.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Horizontal) },
            &[date, rail, content],
            Paint::None,
        );
        let description = [Some(item.when.clone()), item.detail.clone(), item.badge.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        cx.semantics(row, Semantics::new(Role::ListItem).name(item.title.clone()).description(description));
        cx.focusable(row, theme.radii.md);
        let (id, at, count) = (item.id, self.at, self.count);
        if let Some(open) = self.on_open.clone() {
            cx.on_click(row, Rc::new(move || open(id)));
        }
        let (open, goes_to) = (self.on_open.clone(), self.goes_to.clone());
        cx.on_key(
            row,
            Rc::new(move |key, _| match key {
                Key::ArrowDown if at + 1 < count => {
                    goes_to.set(Some(at + 1));
                    true
                }
                Key::ArrowUp if at > 0 => {
                    goes_to.set(Some(at - 1));
                    true
                }
                Key::Enter => {
                    if let Some(open) = &open {
                        open(id);
                    }
                    true
                }
                _ => false,
            }),
        );
        row
    }
}
