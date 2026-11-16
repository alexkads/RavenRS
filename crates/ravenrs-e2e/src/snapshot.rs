//! The screen as text: one line per node, the way a test or an agent reads it.

use std::collections::HashMap;
use std::fmt::Write as _;

use ravenrs_core::{AccessNode, AccessTree, Role, Toggled};

/// A role as the snapshot writes it: `Role::TextInput` is `text_input`.
pub(crate) fn role_name(role: Role) -> String {
    let debug = format!("{role:?}");
    let mut out = String::with_capacity(debug.len() + 4);
    for (i, c) in debug.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// One node in one line: role, name, test id, reference, value and states.
pub(crate) fn line(node: &AccessNode, reference: Option<u32>, focused: bool) -> String {
    let mut out = role_name(node.role);
    if let Some(name) = &node.name {
        let _ = write!(out, " {name:?}");
    }
    if let Some(id) = &node.test_id {
        let _ = write!(out, " #{id}");
    }
    if let Some(r) = reference {
        let _ = write!(out, " [ref=e{r}]");
    }
    if node.protected {
        out.push_str(" value=protected");
    } else if let Some(value) = &node.value {
        let _ = write!(out, " value={value:?}");
    } else if let Some(n) = node.numeric {
        let _ = write!(out, " value={}", n.value);
    }
    match node.toggled {
        Some(Toggled::True) => out.push_str(" checked"),
        Some(Toggled::Mixed) => out.push_str(" mixed"),
        _ => {}
    }
    if node.selected == Some(true) {
        out.push_str(" selected");
    }
    match node.expanded {
        Some(true) => out.push_str(" expanded"),
        Some(false) => out.push_str(" collapsed"),
        None => {}
    }
    if node.disabled {
        out.push_str(" disabled");
    }
    if node.read_only {
        out.push_str(" read_only");
    }
    if node.modal {
        out.push_str(" modal");
    }
    if focused {
        out.push_str(" focused");
    }
    out
}

/// The whole tree, children indented under their parents, the window left out.
pub(crate) fn render(tree: &AccessTree, refs: &HashMap<u64, u32>) -> String {
    let by_id: HashMap<u64, &AccessNode> = tree.nodes.iter().map(|n| (n.id, n)).collect();
    let mut out = String::new();
    if let Some(root) = by_id.get(&tree.root) {
        for child in &root.children {
            walk(&by_id, refs, tree.focus, *child, 0, &mut out);
        }
    }
    out
}

fn walk(
    by_id: &HashMap<u64, &AccessNode>,
    refs: &HashMap<u64, u32>,
    focus: Option<u64>,
    id: u64,
    depth: usize,
    out: &mut String,
) {
    let Some(node) = by_id.get(&id) else { return };
    let _ = writeln!(
        out,
        "{:indent$}- {}",
        "",
        line(node, refs.get(&id).copied(), focus == Some(id)),
        indent = depth * 2
    );
    for child in &node.children {
        walk(by_id, refs, focus, *child, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_read_in_snake_case() {
        assert_eq!(role_name(Role::TextInput), "text_input");
        assert_eq!(role_name(Role::Button), "button");
        assert_eq!(role_name(Role::ListBoxOption), "list_box_option");
    }
}
