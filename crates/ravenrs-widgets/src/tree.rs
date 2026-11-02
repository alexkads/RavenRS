//! Items inside items: a tree that opens, selects, checks and filters.

use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Point, Role, Toggled};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::inputs::check_box_around;
use crate::{IconName, View, util};

const LEVEL_INDENT: f32 = 20.0;
const OVERSCAN: usize = 6;

type KeyHandler = Rc<dyn Fn(&str)>;

/// One item of a [`TreeView`], with the items under it.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    key: String,
    label: String,
    icon: Option<IconName>,
    children: Vec<TreeNode>,
    lazy: bool,
    disabled: bool,
}

impl TreeNode {
    /// An item the app knows by `key`, showing `label`.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
            children: Vec::new(),
            lazy: false,
            disabled: false,
        }
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Appends an item under this one.
    pub fn child(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self
    }

    /// Appends items under this one.
    pub fn children(mut self, children: impl IntoIterator<Item = TreeNode>) -> Self {
        self.children.extend(children);
        self
    }

    /// It can open although its items are not here yet; [`TreeView::on_expand`] brings them.
    pub fn lazy(mut self) -> Self {
        self.lazy = true;
        self
    }

    /// It can be seen and opened, but not selected or checked.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn can_open(&self) -> bool {
        self.lazy || !self.children.is_empty()
    }

    fn matches(&self, query: &str) -> bool {
        self.label.to_lowercase().contains(query) || self.children.iter().any(|child| child.matches(query))
    }

    /// The keys at the tips of the branch: what a check really holds.
    fn collect_tips(&self, out: &mut Vec<String>) {
        if self.children.is_empty() {
            if !self.disabled {
                out.push(self.key.clone());
            }
        } else {
            for child in &self.children {
                child.collect_tips(out);
            }
        }
    }

    fn check_state(&self, checked: &BTreeSet<String>) -> Toggled {
        let mut tips = Vec::new();
        self.collect_tips(&mut tips);
        let on = tips.iter().filter(|tip| checked.contains(*tip)).count();
        if on == 0 {
            Toggled::False
        } else if on == tips.len() {
            Toggled::True
        } else {
            Toggled::Mixed
        }
    }
}

/// A line of the tree as it stands: open branches are followed, closed ones are not.
#[derive(Clone, Debug)]
struct Line {
    node: TreeNode,
    level: u32,
    parent: Option<usize>,
    open: bool,
}

fn lines(nodes: &[TreeNode], expanded: &BTreeSet<String>, query: &str) -> Vec<Line> {
    fn walk(
        nodes: &[TreeNode],
        level: u32,
        parent: Option<usize>,
        expanded: &BTreeSet<String>,
        query: &str,
        out: &mut Vec<Line>,
    ) {
        for node in nodes {
            if !query.is_empty() && !node.matches(query) {
                continue;
            }
            // A filter has to show what it found, wherever it is.
            let open = node.can_open()
                && (expanded.contains(&node.key)
                    || (!query.is_empty() && node.children.iter().any(|c| c.matches(query))));
            let here = out.len();
            out.push(Line { node: node.clone(), level, parent, open });
            if open {
                walk(&node.children, level + 1, Some(here), expanded, query, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, 1, None, expanded, query, &mut out);
    out
}

/// A list of items that open to show more items: a chooser, a menu of a module, the
/// structure of a document. Arrows move and open, Enter opens the item, Space checks it
/// or selects it. With a [`height`](Self::height), only the visible lines are built.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{TreeNode, TreeView, Ui};
///
/// let selected = State::new(None::<String>);
/// let tree = TreeView::new(vec![
///     TreeNode::new("sales", "Sales").child(TreeNode::new("orders", "Orders")),
///     TreeNode::new("stock", "Stock"),
/// ])
/// .selected(selected.binding());
/// let mut ui = Ui::new(tree, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct TreeView {
    nodes: Vec<TreeNode>,
    selected: Option<Binding<Option<String>>>,
    expanded: Option<Binding<BTreeSet<String>>>,
    checked: Option<Binding<BTreeSet<String>>>,
    query: String,
    on_open: Option<KeyHandler>,
    on_expand: Option<KeyHandler>,
    height: Option<f32>,
    row_height: f32,
    label: Option<String>,
}

impl fmt::Debug for TreeView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TreeView").field("nodes", &self.nodes.len()).finish_non_exhaustive()
    }
}

impl TreeView {
    /// A tree of these items, all closed.
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        Self {
            nodes,
            selected: None,
            expanded: None,
            checked: None,
            query: String::new(),
            on_open: None,
            on_expand: None,
            height: None,
            row_height: 32.0,
            label: None,
        }
    }

    /// The item selected, by key.
    pub fn selected(mut self, selected: Binding<Option<String>>) -> Self {
        self.selected = Some(selected);
        self
    }

    /// The items open, by key, to keep or to set from outside.
    pub fn expanded(mut self, expanded: Binding<BTreeSet<String>>) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Shows a check box on every line, bound to the keys of the items at the tips.
    /// Checking a branch checks everything under it, and a branch partly under
    /// check says so.
    pub fn checked(mut self, checked: Binding<BTreeSet<String>>) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Shows only the items with this text, and the branches that lead to them.
    pub fn filter(mut self, text: impl Into<String>) -> Self {
        self.query = text.into().trim().to_lowercase();
        self
    }

    /// Runs with the key on Enter or on a double click.
    pub fn on_open(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// Runs with the key when an item opens, to bring what is under a [`TreeNode::lazy`] one.
    pub fn on_expand(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_expand = Some(Rc::new(handler));
        self
    }

    /// Scrolls inside this height, building only the lines in sight.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Height of a line in logical pixels. 32 by default.
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(24.0);
        self
    }

    /// The name screen readers announce.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[derive(Clone)]
struct TreeActions {
    selected: Option<Binding<Option<String>>>,
    expanded: Binding<BTreeSet<String>>,
    checked: Option<Binding<BTreeSet<String>>>,
    on_open: Option<KeyHandler>,
    on_expand: Option<KeyHandler>,
    active: State<Option<String>>,
}

impl TreeActions {
    fn set_open(&self, key: &str, open: bool) {
        self.expanded.update(|set| {
            if open {
                set.insert(key.to_owned());
            } else {
                set.remove(key);
            }
        });
        if open && let Some(on_expand) = &self.on_expand {
            on_expand(key);
        }
    }

    fn select(&self, node: &TreeNode) {
        self.active.set(Some(node.key.clone()));
        if !node.disabled
            && let Some(selected) = &self.selected
        {
            selected.set(Some(node.key.clone()));
        }
    }

    fn toggle_check(&self, node: &TreeNode) {
        let Some(checked) = &self.checked else { return };
        if node.disabled {
            return;
        }
        let mut tips = Vec::new();
        node.collect_tips(&mut tips);
        let all_on = node.check_state(&checked.get()) == Toggled::True;
        checked.update(|set| {
            for tip in tips {
                if all_on {
                    set.remove(&tip);
                } else {
                    set.insert(tip);
                }
            }
        });
    }

    fn open_item(&self, node: &TreeNode) {
        self.select(node);
        if !node.disabled
            && let Some(on_open) = &self.on_open
        {
            on_open(&node.key);
        }
    }
}

struct TreeLine<'a> {
    line: &'a Line,
    actions: &'a TreeActions,
    active: bool,
    focused: bool,
    selected: bool,
    check: Option<Toggled>,
    row_height: f32,
}

impl View for TreeLine<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let Line { node, level, open, .. } = self.line;
        let ink = theme.color(Semantic::Ink);
        let muted = theme.color(Semantic::InkMuted);
        let tone = |color: Color| if node.disabled { util::faded(color) } else { color };
        let mut parts = Vec::new();
        if node.can_open() {
            let chevron = if *open { IconName::ChevronDown } else { IconName::ChevronRight };
            let glyph = util::icon(cx, chevron, 16.0, muted);
            let button = cx.container(
                LayoutStyle {
                    width: Some(20.0),
                    height: Some(self.row_height),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[glyph],
                Paint::None,
            );
            let (actions, key, open) = (self.actions.clone(), node.key.clone(), *open);
            cx.on_click(button, Rc::new(move || actions.set_open(&key, !open)));
            cx.semantics(button, Semantics::hidden());
            parts.push(button);
        } else {
            parts.push(cx.container(
                LayoutStyle { width: Some(20.0), ..LayoutStyle::leaf() },
                &[],
                Paint::None,
            ));
        }
        if let Some(check) = self.check {
            let state = util::EditorState { disabled: node.disabled, read_only: false };
            let mark = match check {
                Toggled::True => Some(IconName::Check),
                Toggled::Mixed => Some(IconName::Minus),
                _ => None,
            }
            .map(|icon| util::icon(cx, icon, 14.0, theme.color(Semantic::OnAccent)));
            let square =
                check_box_around(cx, &theme, !matches!(check, Toggled::False), false, state, mark.as_slice());
            let (actions, item) = (self.actions.clone(), node.clone());
            cx.on_click(square, Rc::new(move || actions.toggle_check(&item)));
            cx.semantics(square, Semantics::hidden());
            parts.push(square);
        }
        if let Some(icon) = node.icon {
            parts.push(util::icon(cx, icon, 16.0, tone(muted)));
        }
        let text = cx.text_truncated(
            &node.label,
            &util::attrs(&theme, TextStyle::Body, None),
            tone(ink),
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::leaf() },
        );
        cx.semantics(text, Semantics::hidden());
        parts.push(text);
        let fill = if self.selected {
            theme.color(Semantic::Accent).with_alpha(0.14)
        } else if cx.is_hovered() {
            ink.with_alpha(0.05)
        } else {
            Color::TRANSPARENT
        };
        let border = (self.active && self.focused)
            .then(|| Border { width: 1.5, color: theme.color(Semantic::Accent) });
        let row = cx.container(
            LayoutStyle {
                height: Some(self.row_height),
                padding: Insets {
                    left: 4.0 + (*level as f32 - 1.0) * LEVEL_INDENT,
                    right: 8.0,
                    ..Insets::ZERO
                },
                spacing: 6.0,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, border, ..QuadPaint::new(fill) }),
        );
        let (select, item) = (self.actions.clone(), node.clone());
        cx.on_click(row, Rc::new(move || select.select(&item)));
        let (opener, item) = (self.actions.clone(), node.clone());
        cx.on_double_click(row, Rc::new(move || opener.open_item(&item)));
        cx.cursor(row, CursorIcon::Pointer);
        let mut semantics = Semantics::new(Role::TreeItem)
            .name(node.label.clone())
            .level(*level)
            .selected(self.selected)
            .disabled(node.disabled)
            .active(self.active);
        if node.can_open() {
            semantics = semantics.expanded(*open);
        }
        if let Some(check) = self.check {
            semantics = semantics.toggled(check);
        }
        cx.semantics(row, semantics);
        row
    }
}

fn key_handler(
    all: Rc<Vec<Line>>,
    at: usize,
    actions: TreeActions,
    reveal: State<bool>,
) -> crate::KeyHandler {
    Rc::new(move |key, _| {
        let Some(line) = all.get(at) else { return false };
        let go = |index: usize| {
            if let Some(line) = all.get(index) {
                actions.active.set(Some(line.node.key.clone()));
                reveal.set(true);
            }
        };
        match key {
            Key::ArrowDown => go((at + 1).min(all.len().saturating_sub(1))),
            Key::ArrowUp => go(at.saturating_sub(1)),
            Key::Home => go(0),
            Key::End => go(all.len().saturating_sub(1)),
            Key::ArrowRight if line.node.can_open() && !line.open => actions.set_open(&line.node.key, true),
            Key::ArrowRight if line.open => go(at + 1),
            Key::ArrowRight => {}
            Key::ArrowLeft if line.open => actions.set_open(&line.node.key, false),
            Key::ArrowLeft => {
                if let Some(parent) = line.parent {
                    go(parent);
                }
            }
            Key::Enter => actions.open_item(&line.node),
            Key::Space if actions.checked.is_some() => actions.toggle_check(&line.node),
            Key::Space => actions.select(&line.node),
            _ => return false,
        }
        true
    })
}

impl View for TreeView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let local: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let expanded = self.expanded.clone().unwrap_or_else(|| local.binding());
        let active: State<Option<String>> = cx.use_state(|| None);
        let reveal: State<bool> = cx.use_state(|| false);
        let all = Rc::new(lines(&self.nodes, &expanded.get(), &self.query));
        let selected = self.selected.as_ref().and_then(Binding::get);
        let checked = self.checked.as_ref().map(Binding::get);
        // The keyboard's place: where it was, else the selection, else the top.
        let wanted = active.get().or_else(|| selected.clone());
        let at = wanted.and_then(|key| all.iter().position(|l| l.node.key == key)).unwrap_or(0);
        let actions = TreeActions {
            selected: self.selected.clone(),
            expanded,
            checked: self.checked.clone(),
            on_open: self.on_open.clone(),
            on_expand: self.on_expand.clone(),
            active: active.clone(),
        };
        let rh = self.row_height;
        let focused = cx.is_focused();
        let offset = cx.scroll_offset();
        let range = match self.height {
            Some(height) => {
                let viewport = cx.scroll_viewport().map_or(height, |size| size.height);
                let top = at as f32 * rh;
                if *reveal.peek() {
                    reveal.set_if_changed(false);
                    if top < offset.y {
                        cx.set_scroll_offset(Point::new(0.0, top));
                    } else if top + rh > offset.y + viewport {
                        cx.set_scroll_offset(Point::new(0.0, top + rh - viewport));
                    }
                }
                let first = ((offset.y / rh).floor() as usize).saturating_sub(OVERSCAN).min(all.len());
                let last = (((offset.y + viewport) / rh).ceil() as usize + OVERSCAN).min(all.len());
                first..last
            }
            None => 0..all.len(),
        };
        let mut rows = Vec::with_capacity(range.len() + 2);
        rows.push(cx.container(
            LayoutStyle { height: Some(range.start as f32 * rh), ..LayoutStyle::leaf() },
            &[],
            Paint::None,
        ));
        for i in range.clone() {
            let line = &all[i];
            let row = TreeLine {
                line,
                actions: &actions,
                active: i == at,
                focused,
                selected: selected.as_deref() == Some(line.node.key.as_str()),
                check: checked.as_ref().map(|set| line.node.check_state(set)),
                row_height: rh,
            };
            rows.push(cx.keyed_child(util::hash_text(&line.node.key), &row));
        }
        rows.push(cx.container(
            LayoutStyle {
                height: Some(all.len().saturating_sub(range.end) as f32 * rh),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::None,
        ));
        let content = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        let body = cx.container(
            LayoutStyle {
                height: self.height,
                min_height: Some(0.0),
                overflow_hidden: self.height.is_some(),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        if self.height.is_some() {
            cx.scrollable_virtual(body, false);
        }
        let name = self.label.clone().unwrap_or_else(|| cx.labels().tree_name.clone());
        cx.semantics(body, Semantics::new(Role::Tree).name(name));
        cx.focusable(body, 4.0);
        cx.on_key(body, key_handler(all, at, actions, reveal));
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<TreeNode> {
        vec![
            TreeNode::new("a", "Alpha")
                .child(TreeNode::new("a1", "One").child(TreeNode::new("deep", "Deep")))
                .child(TreeNode::new("a2", "Two")),
            TreeNode::new("b", "Beta"),
        ]
    }

    #[test]
    fn closed_branches_are_not_followed() {
        let keys = |open: &[&str]| -> Vec<String> {
            let open = open.iter().map(|s| (*s).to_owned()).collect();
            lines(&sample(), &open, "").into_iter().map(|l| l.node.key).collect()
        };
        assert_eq!(keys(&[]), ["a", "b"]);
        assert_eq!(keys(&["a"]), ["a", "a1", "a2", "b"]);
        assert_eq!(keys(&["a", "a1"]), ["a", "a1", "deep", "a2", "b"]);
        assert_eq!(keys(&["a1"]), ["a", "b"], "a closed parent hides what is open inside");
    }

    #[test]
    fn a_filter_opens_the_way_to_what_it_finds() {
        let found = lines(&sample(), &BTreeSet::new(), "deep");
        let keys: Vec<_> = found.iter().map(|l| l.node.key.as_str()).collect();
        assert_eq!(keys, ["a", "a1", "deep"]);
        assert_eq!(found[2].parent, Some(1));
        assert_eq!(found[2].level, 3);
    }

    #[test]
    fn a_branch_is_checked_by_what_is_under_it() {
        let root = &sample()[0];
        let set = |keys: &[&str]| keys.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
        assert_eq!(root.check_state(&set(&[])), Toggled::False);
        assert_eq!(root.check_state(&set(&["deep"])), Toggled::Mixed);
        assert_eq!(root.check_state(&set(&["deep", "a2"])), Toggled::True);
    }
}
