//! Builds the accessibility tree from the element tree and performs the
//! actions assistive technology requests.

use std::collections::HashMap;

use ravenrs_core::{AccessAction, AccessNode, AccessTree, Key, Modifiers, Point, Rect, Role};

use super::Ui;
use crate::context::{NodeId, WidgetId};

/// Id of the window node; widget-derived ids never collide with it in practice.
const ROOT_ID: u64 = 1;

/// Maps accessibility ids back to elements for [`Ui::perform_action`].
#[derive(Debug, Default)]
pub(crate) struct AccessMap {
    nodes: HashMap<u64, NodeId>,
}

impl Ui {
    /// The accessibility tree of the latest frame: roles, names, states and
    /// frames of everything on screen, for screen readers. Call it after
    /// [`Ui::update`]; platform layers send it to the operating system when it
    /// changes. While a modal layer is open, only that layer is exposed.
    pub fn access_tree(&mut self) -> AccessTree {
        let mut builder = Builder {
            ui: self,
            nodes: Vec::new(),
            seen: HashMap::new(),
            map: HashMap::new(),
            focus: None,
            active: None,
        };
        builder.nodes.push(AccessNode::new(
            ROOT_ID,
            Role::Window,
            Rect::from_origin_size(Point::ZERO, self.viewport),
        ));
        let mut roots = Vec::new();
        match self.modal_root() {
            Some(modal) => {
                let from = self.overlays.iter().position(|o| o.root == modal).unwrap_or(0);
                roots.extend(self.overlays[from..].iter().map(|o| o.root));
            }
            None => {
                roots.extend(self.root_node);
                roots.extend(self.overlays.iter().map(|o| o.root));
            }
        }
        let mut children = Vec::new();
        for root in roots {
            builder.visit(root, &mut children, None);
        }
        let Builder { mut nodes, map, focus, active, .. } = builder;
        nodes[0].children = children;
        if let (Some(focus), Some(active)) = (focus, active)
            && let Some(node) = nodes.iter_mut().find(|n| n.id == focus)
        {
            node.active_descendant = Some(active);
        }
        self.access_map = AccessMap { nodes: map };
        AccessTree { root: ROOT_ID, nodes, focus }
    }

    /// Performs an action assistive technology asked for on the node with
    /// this id (from the latest [`Ui::access_tree`]). Returns true if the UI
    /// needs a redraw.
    pub fn perform_action(&mut self, id: u64, action: AccessAction) -> bool {
        let Some(node) = self.access_map.nodes.get(&id).copied() else { return false };
        let Some(element) = self.elements.get(node.0 as usize) else { return false };
        let widget = element.widget;
        match action {
            AccessAction::Click => {
                if let Some(handler) = element.on_click.clone() {
                    handler();
                } else if element.focus_radius.is_some() {
                    self.focus_widget(widget);
                }
            }
            AccessAction::Focus => {
                if element.focus_radius.is_some() {
                    self.focus_widget(widget);
                }
            }
            AccessAction::Increment | AccessAction::Decrement => {
                let key = if action == AccessAction::Increment { Key::ArrowUp } else { Key::ArrowDown };
                if let Some(handler) = element.on_key.clone() {
                    handler(&key, Modifiers::default());
                }
            }
            _ => return false,
        }
        self.needs_redraw()
    }

    fn focus_widget(&mut self, widget: WidgetId) {
        if self.interaction.focused != Some(widget) {
            self.interaction.focused = Some(widget);
            self.interaction.focus_visible = true;
            self.needs_build = true;
        }
    }
}

struct Builder<'a> {
    ui: &'a Ui,
    nodes: Vec<AccessNode>,
    /// How many nodes each widget produced so far, to tell them apart.
    seen: HashMap<WidgetId, u64>,
    map: HashMap<u64, NodeId>,
    focus: Option<u64>,
    /// The highlighted option of a combobox-style list.
    active: Option<u64>,
}

impl Builder<'_> {
    fn id_for(&mut self, widget: WidgetId) -> u64 {
        let n = self.seen.entry(widget).or_insert(0);
        *n += 1;
        let mut h = widget.raw() ^ n.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h = (h ^ (h >> 31)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        // Keep ids away from the root's.
        (h ^ (h >> 29)).max(ROOT_ID + 1)
    }

    /// Adds `node`'s accessible content to `out`. Text goes to `name` instead
    /// when an ancestor takes its name from its content.
    fn visit(&mut self, node: NodeId, out: &mut Vec<u64>, mut name: Option<&mut String>) {
        let ui = self.ui;
        let i = node.0 as usize;
        let e = &ui.elements[i];
        let semantics = e.semantics.as_deref();
        if semantics.is_some_and(|s| s.hidden) {
            return;
        }
        if semantics.is_none()
            && let Some(text) = &e.text
        {
            let text = readable(text);
            if text.is_empty() {
                return;
            }
            match name {
                Some(name) => {
                    if !name.is_empty() {
                        name.push(' ');
                    }
                    name.push_str(&text);
                }
                None => {
                    let id = self.id_for(e.widget);
                    let mut label = AccessNode::new(id, Role::Label, ui.frames[i]);
                    label.name = Some(text);
                    self.map.insert(id, node);
                    self.nodes.push(label);
                    out.push(id);
                }
            }
            return;
        }
        let focusable = e.focus_radius.is_some();
        if semantics.is_none() && !focusable {
            for child in &e.children {
                self.visit(*child, out, name.as_deref_mut());
            }
            return;
        }
        let role = semantics.map_or(Role::Button, |s| s.role);
        let id = self.id_for(e.widget);
        let index = self.nodes.len();
        let mut a = AccessNode::new(id, role, ui.frames[i]);
        a.focusable = focusable;
        if let Some(s) = semantics {
            a.name = s.name.clone();
            a.description = s.description.clone();
            a.value = s.value.clone();
            a.numeric = s.numeric;
            a.toggled = s.toggled;
            a.selected = s.selected;
            a.expanded = s.expanded;
            a.disabled = s.disabled;
            a.modal = s.modal;
            a.level = s.level;
            if s.active {
                self.active = Some(id);
            }
        }
        if e.on_click.is_some() {
            a.actions.push(AccessAction::Click);
        }
        if focusable {
            a.actions.push(AccessAction::Focus);
            if ui.interaction.focused == Some(e.widget) {
                self.focus = Some(id);
            }
        }
        if e.on_key.is_some() && matches!(role, Role::Slider | Role::SpinButton) {
            a.actions.extend([AccessAction::Increment, AccessAction::Decrement]);
        }
        self.map.insert(id, node);
        self.nodes.push(a);
        let mut content = e.text.as_deref().map(readable).unwrap_or_default();
        // Text inside a control that takes its name from content is part of
        // that name, even when an explicit name replaces it.
        let from_content = role.takes_name_from_content();
        let mut children = Vec::new();
        for child in &e.children {
            self.visit(*child, &mut children, from_content.then_some(&mut content));
        }
        let node = &mut self.nodes[index];
        node.children = children;
        if from_content && node.name.is_none() && !content.is_empty() {
            node.name = Some(content.clone());
        }
        // A named control inside a named control (a button in a grid cell)
        // still reads as part of the outer name.
        if let Some(name) = name
            && let Some(own) = &node.name
        {
            if !name.is_empty() {
                name.push(' ');
            }
            name.push_str(own);
        }
        out.push(id);
    }
}

/// Text as a screen reader should read it: icon glyphs (private-use code
/// points) dropped and whitespace collapsed.
fn readable(text: &str) -> String {
    let kept: String =
        text.chars().filter(|c| !matches!(*c as u32, 0xE000..=0xF8FF | 0xF0000..=0x10FFFF)).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::readable;

    #[test]
    fn readable_text_drops_icons_and_collapses_spaces() {
        assert_eq!(readable("\u{e001}  Save\n file "), "Save file");
        assert_eq!(readable("\u{e001}"), "");
    }
}
