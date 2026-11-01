//! Screen reader support: the UI's [`AccessTree`] translated to AccessKit,
//! which talks to VoiceOver on macOS, Narrator and NVDA on Windows, and Orca
//! on Linux.

use accesskit::{Action, Affine, Live, Node, NodeId, Rect, TreeId, TreeInfo, TreeUpdate};
use ravenrs_core::{AccessAction, AccessNode, AccessTree, Role, Toggled};

/// The AccessKit role for a RavenRS role.
pub(crate) fn role(role: Role) -> accesskit::Role {
    use accesskit::Role as A;
    match role {
        Role::Window => A::Window,
        Role::Group => A::Group,
        Role::Label => A::Label,
        Role::Heading => A::Heading,
        Role::Button => A::Button,
        Role::Link => A::Link,
        Role::CheckBox => A::CheckBox,
        Role::Switch => A::Switch,
        Role::RadioGroup => A::RadioGroup,
        Role::RadioButton => A::RadioButton,
        Role::TextInput => A::TextInput,
        Role::MultilineTextInput => A::MultilineTextInput,
        Role::Slider => A::Slider,
        Role::SpinButton => A::SpinButton,
        Role::ComboBox => A::ComboBox,
        Role::ListBox => A::ListBox,
        Role::ListBoxOption => A::ListBoxOption,
        Role::Menu => A::Menu,
        Role::MenuBar => A::MenuBar,
        Role::MenuItem => A::MenuItem,
        Role::TabList => A::TabList,
        Role::Tab => A::Tab,
        Role::TabPanel => A::TabPanel,
        Role::Dialog => A::Dialog,
        Role::Alert => A::Alert,
        Role::Status => A::Status,
        Role::Tooltip => A::Tooltip,
        Role::Toolbar => A::Toolbar,
        Role::Grid => A::Grid,
        Role::Table => A::Table,
        Role::Row => A::Row,
        Role::Cell => A::Cell,
        Role::ColumnHeader => A::ColumnHeader,
        Role::List => A::List,
        Role::ListItem => A::ListItem,
        Role::Image => A::Image,
        Role::ProgressIndicator => A::ProgressIndicator,
        Role::Separator => A::Splitter,
        Role::ScrollView => A::ScrollView,
        Role::Calendar => A::Grid,
        Role::ContentInfo => A::ContentInfo,
        _ => A::GenericContainer,
    }
}

/// The RavenRS action for an AccessKit request, if it has one.
pub(crate) fn action(action: Action) -> Option<AccessAction> {
    match action {
        Action::Click => Some(AccessAction::Click),
        Action::Focus => Some(AccessAction::Focus),
        Action::Increment => Some(AccessAction::Increment),
        Action::Decrement => Some(AccessAction::Decrement),
        // Expanding and collapsing a disclosure or menu is the same as clicking it.
        Action::Expand | Action::Collapse => Some(AccessAction::Click),
        _ => None,
    }
}

fn node(n: &AccessNode, scale: f64, is_root: bool) -> Node {
    let mut node = Node::new(role(n.role));
    if is_root {
        // Bounds are in logical pixels; the root scales them to the window's.
        node.set_transform(Affine::scale(scale));
    }
    let b = n.bounds;
    node.set_bounds(Rect::new(
        f64::from(b.x()),
        f64::from(b.y()),
        f64::from(b.max_x()),
        f64::from(b.max_y()),
    ));
    if let Some(name) = &n.name {
        node.set_label(name.as_str());
    }
    if let Some(d) = &n.description {
        node.set_description(d.as_str());
    }
    if let Some(v) = &n.value {
        node.set_value(v.as_str());
    }
    if let Some(v) = n.numeric {
        node.set_numeric_value(v.value);
        node.set_min_numeric_value(v.min);
        node.set_max_numeric_value(v.max);
        if let Some(step) = v.step {
            node.set_numeric_value_step(step);
        }
    }
    if let Some(t) = n.toggled {
        node.set_toggled(match t {
            Toggled::True => accesskit::Toggled::True,
            Toggled::Mixed => accesskit::Toggled::Mixed,
            _ => accesskit::Toggled::False,
        });
    }
    if let Some(s) = n.selected {
        node.set_selected(s);
    }
    if let Some(e) = n.expanded {
        node.set_expanded(e);
    }
    if n.disabled {
        node.set_disabled();
    }
    if n.modal {
        node.set_modal();
    }
    if let Some(level) = n.level {
        node.set_level(level as usize);
    }
    if let Some(active) = n.active_descendant {
        node.set_active_descendant(NodeId(active));
    }
    match n.role {
        Role::Status => node.set_live(Live::Polite),
        Role::Alert => node.set_live(Live::Assertive),
        _ => {}
    }
    for a in &n.actions {
        node.add_action(match a {
            AccessAction::Click => Action::Click,
            AccessAction::Focus => Action::Focus,
            AccessAction::Increment => Action::Increment,
            AccessAction::Decrement => Action::Decrement,
            _ => continue,
        });
    }
    if n.expanded.is_some() && n.actions.contains(&AccessAction::Click) {
        node.add_action(if n.expanded == Some(true) { Action::Collapse } else { Action::Expand });
    }
    node.set_children(n.children.iter().map(|c| NodeId(*c)).collect::<Vec<_>>());
    node
}

/// A complete tree update for AccessKit. `scale` converts logical to
/// physical pixels.
pub(crate) fn tree_update(tree: &AccessTree, scale: f64) -> TreeUpdate {
    let nodes = tree.nodes.iter().map(|n| (NodeId(n.id), node(n, scale, n.id == tree.root))).collect();
    let mut info = TreeInfo::new(NodeId(tree.root));
    info.toolkit_name = Some("RavenRS".into());
    info.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
    TreeUpdate {
        nodes,
        tree: Some(info),
        tree_id: TreeId::ROOT,
        focus: NodeId(tree.focus.unwrap_or(tree.root)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravenrs_core::{Point, Size};

    fn rect(x: f32, y: f32, w: f32, h: f32) -> ravenrs_core::Rect {
        ravenrs_core::Rect::from_origin_size(Point::new(x, y), Size::new(w, h))
    }

    #[test]
    fn the_footer_landmark_keeps_its_role() {
        assert_eq!(role(Role::ContentInfo), accesskit::Role::ContentInfo);
    }

    #[test]
    fn tree_update_carries_roles_state_and_focus() {
        let mut root = AccessNode::new(1, Role::Window, rect(0.0, 0.0, 800.0, 600.0));
        root.children = vec![2, 3];
        let mut check = AccessNode::new(2, Role::CheckBox, rect(10.0, 10.0, 100.0, 20.0));
        check.name = Some("Accept".into());
        check.toggled = Some(Toggled::True);
        check.focusable = true;
        check.actions = vec![AccessAction::Click, AccessAction::Focus];
        let mut toast = AccessNode::new(3, Role::Status, rect(0.0, 0.0, 10.0, 10.0));
        toast.name = Some("Saved".into());
        let tree = AccessTree { root: 1, nodes: vec![root, check, toast], focus: Some(2) };

        let update = tree_update(&tree, 2.0);
        assert_eq!(update.focus, NodeId(2));
        assert_eq!(update.tree.as_ref().map(|t| t.root), Some(NodeId(1)));
        let (_, root) = &update.nodes[0];
        assert_eq!(root.role(), accesskit::Role::Window);
        assert_eq!(root.transform().copied(), Some(Affine::scale(2.0)));
        assert_eq!(root.children(), &[NodeId(2), NodeId(3)]);
        let (_, check) = &update.nodes[1];
        assert_eq!(check.role(), accesskit::Role::CheckBox);
        assert_eq!(check.label(), Some("Accept"));
        assert_eq!(check.toggled(), Some(accesskit::Toggled::True));
        assert!(check.supports_action(Action::Click));
        assert_eq!(check.bounds(), Some(Rect::new(10.0, 10.0, 110.0, 30.0)));
        let (_, toast) = &update.nodes[2];
        assert_eq!(toast.live(), Some(Live::Polite), "status messages are announced");
    }

    #[test]
    fn actions_map_back() {
        assert_eq!(action(Action::Click), Some(AccessAction::Click));
        assert_eq!(action(Action::Expand), Some(AccessAction::Click));
        assert_eq!(action(Action::Increment), Some(AccessAction::Increment));
        assert_eq!(action(Action::ScrollDown), None);
    }
}
