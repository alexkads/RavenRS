//! The ARIA a node of the [`AccessTree`](ravenrs_core::AccessTree) gets in the browser: its
//! role and its attributes. Plain data, so it is tested on every target.

use ravenrs_core::{AccessNode, Role, Toggled};

/// The ARIA role for a RavenRS role (`None`: a plain element).
pub(crate) fn aria_role(role: Role) -> Option<&'static str> {
    Some(match role {
        Role::Window => "application",
        Role::Group => "group",
        Role::Label => return None,
        Role::Heading => "heading",
        Role::Button => "button",
        Role::Link => "link",
        Role::CheckBox => "checkbox",
        Role::Switch => "switch",
        Role::RadioGroup => "radiogroup",
        Role::RadioButton => "radio",
        Role::TextInput | Role::MultilineTextInput => "textbox",
        Role::Slider => "slider",
        Role::SpinButton => "spinbutton",
        Role::ComboBox => "combobox",
        Role::ListBox => "listbox",
        Role::ListBoxOption => "option",
        Role::Menu => "menu",
        Role::MenuBar => "menubar",
        Role::MenuItem => "menuitem",
        Role::TabList => "tablist",
        Role::Tab => "tab",
        Role::TabPanel => "tabpanel",
        Role::Dialog => "dialog",
        Role::Alert => "alert",
        Role::Status => "status",
        Role::Tooltip => "tooltip",
        Role::Toolbar => "toolbar",
        Role::Grid | Role::Calendar => "grid",
        Role::Tree => "tree",
        Role::TreeItem => "treeitem",
        Role::Table => "table",
        Role::Row => "row",
        Role::Cell => "cell",
        Role::ColumnHeader => "columnheader",
        Role::List => "list",
        Role::ListItem => "listitem",
        Role::Image => "img",
        Role::ProgressIndicator => "progressbar",
        Role::Separator => "separator",
        Role::ScrollView => "region",
        Role::ContentInfo => "contentinfo",
        Role::Navigation => "navigation",
        _ => "group",
    })
}

/// The ARIA attributes for a node, in a stable order.
pub(crate) fn aria_attributes(n: &AccessNode) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some(role) = aria_role(n.role) {
        out.push(("role", role.to_owned()));
    }
    if n.role != Role::Label
        && let Some(name) = &n.name
    {
        out.push(("aria-label", name.clone()));
    }
    if let Some(d) = &n.description {
        out.push(("aria-description", d.clone()));
    }
    if let Some(v) = n.numeric {
        out.push(("aria-valuenow", v.value.to_string()));
        out.push(("aria-valuemin", v.min.to_string()));
        out.push(("aria-valuemax", v.max.to_string()));
    }
    // Text fields and combo boxes show their value as content instead.
    if let (Some(v), Some(_)) = (&n.value, n.numeric) {
        out.push(("aria-valuetext", v.clone()));
    }
    if let Some(t) = n.toggled {
        let value = match t {
            Toggled::True => "true",
            Toggled::Mixed => "mixed",
            _ => "false",
        };
        let attr = if matches!(n.role, Role::Button) { "aria-pressed" } else { "aria-checked" };
        out.push((attr, value.to_owned()));
    }
    if let Some(s) = n.selected {
        let attr = if matches!(n.role, Role::Button) { "aria-current" } else { "aria-selected" };
        out.push((attr, s.to_string()));
    }
    if let Some(e) = n.expanded {
        out.push(("aria-expanded", e.to_string()));
    }
    if n.disabled {
        out.push(("aria-disabled", "true".to_owned()));
    }
    if n.read_only {
        out.push(("aria-readonly", "true".to_owned()));
    }
    if n.modal {
        out.push(("aria-modal", "true".to_owned()));
    }
    if n.busy {
        out.push(("aria-busy", "true".to_owned()));
    }
    if let Some(level) = n.level {
        out.push(("aria-level", level.to_string()));
    }
    // For tests and automation only; screen readers do not read data attributes.
    if let Some(test_id) = &n.test_id {
        out.push(("data-testid", test_id.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use ravenrs_core::{AccessNode, Rect, Role};

    use super::aria_attributes;

    fn has(n: &AccessNode, attr: &str, value: &str) -> bool {
        aria_attributes(n).iter().any(|(k, v)| *k == attr && v == value)
    }

    #[test]
    fn an_open_node_is_expanded_and_a_busy_one_is_busy() {
        let mut n = AccessNode::new(3, Role::TreeItem, Rect::ZERO);
        n.expanded = Some(true);
        n.busy = true;
        assert!(has(&n, "aria-expanded", "true"));
        assert!(has(&n, "aria-busy", "true"));
        n.busy = false;
        assert!(!aria_attributes(&n).iter().any(|(k, _)| *k == "aria-busy"));
    }
}
