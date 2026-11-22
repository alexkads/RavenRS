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
#[doc(hidden)]
pub struct AccessMap {
    nodes: HashMap<u64, NodeId>,
    #[cfg(feature = "e2e-source")]
    sources: HashMap<u64, &'static std::panic::Location<'static>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
            pending_test_tag: None,
            #[cfg(feature = "e2e-source")]
            sources: HashMap::new(),
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
        let Builder {
            mut nodes,
            map,
            focus,
            active,
            #[cfg(feature = "e2e-source")]
            sources,
            ..
        } = builder;
        // What is being carried between components, said as it changes.
        if let Some(words) = self.carry_status() {
            // Out of the way of the ids widgets get.
            let id = u64::MAX - 1;
            let mut status = AccessNode::new(id, Role::Status, Rect::ZERO);
            status.name = Some(words);
            nodes.push(status);
            children.push(id);
        }
        nodes[0].children = children;
        if let (Some(focus), Some(active)) = (focus, active)
            && let Some(node) = nodes.iter_mut().find(|n| n.id == focus)
        {
            node.active_descendant = Some(active);
        }
        self.access_map = AccessMap {
            nodes: map,
            #[cfg(feature = "e2e-source")]
            sources,
        };
        AccessTree { root: ROOT_ID, nodes, focus }
    }

    /// The keyboard shortcuts the latest frame answers to, in the order they were registered,
    /// for tools that look for two commands on the same keys.
    pub fn shortcuts(&self) -> Vec<crate::Hotkey> {
        self.shortcuts
            .iter()
            .map(|s| {
                let key = match &s.key {
                    Key::Character(c) => Key::Character(c.to_lowercase()),
                    other => other.clone(),
                };
                crate::Hotkey::of(key, s.command, s.shift)
            })
            .collect()
    }

    /// The deepest node of the latest [`Ui::access_tree`] under `point`, as the pointer would
    /// find it: what a click there lands on. Tests use it to tell that something covers the
    /// node they mean to click.
    pub fn access_node_at(&self, point: Point) -> Option<u64> {
        let mut at = self.hit_test(point)?.0 as usize;
        let mut parent = vec![usize::MAX; self.elements.len()];
        for (i, e) in self.elements.iter().enumerate() {
            for child in &e.children {
                if let Some(slot) = parent.get_mut(child.0 as usize) {
                    *slot = i;
                }
            }
        }
        let ids: HashMap<usize, u64> =
            self.access_map.nodes.iter().map(|(id, node)| (node.0 as usize, *id)).collect();
        loop {
            if let Some(id) = ids.get(&at) {
                return Some(*id);
            }
            at = *parent.get(at)?;
            if at == usize::MAX {
                return None;
            }
        }
    }

    /// The frames of the parts that scroll around the node `id` of the latest
    /// [`Ui::access_tree`], the nearest first. A [`ScrollView`](crate::ScrollView) has no
    /// node of its own in the tree, so this is how tests learn what hides a node inside the
    /// window and where to turn the wheel to show it.
    pub fn scroll_frames(&self, id: u64) -> Vec<Rect> {
        let Some(node) = self.access_map.nodes.get(&id) else { return Vec::new() };
        let mut parent = vec![usize::MAX; self.elements.len()];
        for (i, e) in self.elements.iter().enumerate() {
            for child in &e.children {
                if let Some(slot) = parent.get_mut(child.0 as usize) {
                    *slot = i;
                }
            }
        }
        let mut frames = Vec::new();
        let mut at = node.0 as usize;
        while let Some(&up) = parent.get(at) {
            if up == usize::MAX {
                break;
            }
            if self.elements[up].scroll {
                frames.push(self.screen_frame(up));
            }
            at = up;
        }
        frames
    }

    /// Performs an action assistive technology asked for on the node with
    /// this id (from the latest [`Ui::access_tree`]). Returns true if the UI
    /// needs a redraw.
    pub fn perform_action(&mut self, id: u64, action: AccessAction) -> bool {
        self.perform(id, action, None)
    }

    /// Performs [`AccessAction::SetValue`] with this text on the node with this id, for a node
    /// that declared it with [`BuildCx::on_access`](crate::BuildCx::on_access). Returns true if
    /// the UI needs a redraw.
    pub fn set_access_value(&mut self, id: u64, value: &str) -> bool {
        self.perform(id, AccessAction::SetValue, Some(value))
    }

    fn perform(&mut self, id: u64, action: AccessAction, value: Option<&str>) -> bool {
        let Some(node) = self.access_map.nodes.get(&id).copied() else { return false };
        let Some(element) = self.elements.get(node.0 as usize) else { return false };
        let widget = element.widget;
        // What the node declared it does comes first.
        if let Some((actions, handler)) = &element.access
            && actions.contains(&action)
        {
            let handler = handler.clone();
            if handler(action, value) {
                self.needs_build = true;
                return self.needs_redraw();
            }
            return false;
        }
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
            // A node that opens, closes or is chosen with a click and declares nothing else
            // does it with that click, as screen readers did before these actions existed.
            AccessAction::Expand | AccessAction::Collapse | AccessAction::Select => {
                let Some(handler) = element.on_click.clone() else { return false };
                handler();
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

impl Ui {
    /// Where this node's test id was declared, or its instrumented view was constructed, as of the latest
    /// [`Self::access_tree`]. Returns `None` without `e2e-source`; not sent to screen readers.
    pub fn test_source(&self, id: u64) -> Option<&'static std::panic::Location<'static>> {
        #[cfg(feature = "e2e-source")]
        {
            self.access_map.sources.get(&id).copied()
        }
        #[cfg(not(feature = "e2e-source"))]
        {
            let _ = id;
            None
        }
    }
}

#[derive(Clone)]
struct TestTag {
    id: std::rc::Rc<str>,
    #[cfg(feature = "e2e-source")]
    source: Option<&'static std::panic::Location<'static>>,
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
    /// A test id from an element that shows nothing of its own, waiting for the first node
    /// inside it.
    pending_test_tag: Option<TestTag>,
    #[cfg(feature = "e2e-source")]
    sources: HashMap<u64, &'static std::panic::Location<'static>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
    /// The test id a new node takes: its element's own, or one waiting from a container.
    fn claim_test_tag(
        &mut self,
        own: Option<TestTag>,
        id: u64,
        #[cfg(feature = "e2e-source")] view_source: Option<&'static std::panic::Location<'static>>,
    ) -> Option<String> {
        let tag = own.or(self.pending_test_tag.take());
        #[cfg(feature = "e2e-source")]
        if let Some(source) = tag.as_ref().and_then(|tag| tag.source).or(view_source) {
            self.sources.insert(id, source);
        }
        #[cfg(not(feature = "e2e-source"))]
        let _ = id;
        tag.map(|tag| tag.id.to_string())
    }

    fn visit(&mut self, node: NodeId, out: &mut Vec<u64>, mut name: Option<&mut String>) {
        let ui = self.ui;
        let i = node.0 as usize;
        let e = &ui.elements[i];
        let semantics = e.semantics.as_deref();
        if semantics.is_some_and(|s| s.hidden) {
            return;
        }
        let own_tag = e.test_id.as_ref().map(|id| TestTag {
            id: id.clone(),
            #[cfg(feature = "e2e-source")]
            source: e.test_source,
        });
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
                    let mut label = AccessNode::new(id, Role::Label, ui.screen_frame(i));
                    label.name = Some(text);
                    label.test_id = self.claim_test_tag(
                        own_tag,
                        id,
                        #[cfg(feature = "e2e-source")]
                        e.view_source,
                    );
                    self.map.insert(id, node);
                    self.nodes.push(label);
                    out.push(id);
                }
            }
            return;
        }
        let focusable = e.focus_radius.is_some();
        if semantics.is_none() && !focusable {
            // A container passes its test id to the first node inside it.
            let before = self.nodes.len();
            let outer = own_tag.map(|tag| self.pending_test_tag.replace(tag));
            for child in &e.children {
                self.visit(*child, out, name.as_deref_mut());
            }
            if let Some(outer) = outer {
                self.pending_test_tag = if self.nodes.len() > before { None } else { outer };
            }
            return;
        }
        let role = semantics.map_or(Role::Button, |s| s.role);
        let id = self.id_for(e.widget);
        let index = self.nodes.len();
        let mut a = AccessNode::new(id, role, ui.screen_frame(i));
        a.focusable = focusable;
        a.test_id = self.claim_test_tag(
            own_tag,
            id,
            #[cfg(feature = "e2e-source")]
            e.view_source,
        );
        if let Some(s) = semantics {
            a.name = s.name.clone();
            a.description = s.description.clone();
            a.value = s.value.clone();
            a.numeric = s.numeric;
            a.toggled = s.toggled;
            a.selected = s.selected;
            a.expanded = s.expanded;
            a.disabled = s.disabled;
            a.read_only = s.read_only;
            a.modal = s.modal;
            a.level = s.level;
            a.protected = s.protected;
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
        if let Some((actions, _)) = &e.access {
            for action in actions {
                if !a.actions.contains(action) {
                    a.actions.push(*action);
                }
            }
        }
        a.busy = e.busy || e.busy_signal.as_ref().is_some_and(crate::context::BusySignal::is_busy);
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
    #[cfg(not(feature = "e2e-source"))]
    fn default_builds_do_not_record_view_origins() {
        use crate::{Button, Text, Ui, VStack};
        let mut ui = Ui::new(
            VStack::new((Button::new("Save"), Text::new("Plain"))),
            ravenrs_style::Theme::raven_light(),
        );
        ui.update();
        let tree = ui.access_tree();
        for node in &tree.nodes {
            assert!(ui.test_source(node.id).is_none());
        }
    }

    #[test]
    fn readable_text_drops_icons_and_collapses_spaces() {
        assert_eq!(readable("\u{e001}  Save\n file "), "Save file");
        assert_eq!(readable("\u{e001}"), "");
    }
}
