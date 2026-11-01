//! Screen reader support in the browser: the UI's [`AccessTree`] mirrored as
//! invisible ARIA elements over the canvas. Keyboard focus stays on the hidden
//! text input, which points at the focused element with
//! `aria-activedescendant`; status messages are announced through a live region.

use std::collections::HashSet;

use ravenrs_core::{AccessNode, AccessTree, Role, Toggled};
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement, HtmlTextAreaElement};

use crate::WebError;

/// Prefix of mirror element ids; the rest is the node id.
pub(crate) const ID_PREFIX: &str = "ravenrs-a11y-";

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
    if n.modal {
        out.push(("aria-modal", "true".to_owned()));
    }
    if let Some(level) = n.level {
        out.push(("aria-level", level.to_string()));
    }
    out
}

/// The text a status message reads: its name, else the text inside it.
pub(crate) fn spoken_text(tree: &AccessTree, node: &AccessNode) -> String {
    if let Some(name) = &node.name {
        return name.clone();
    }
    let mut parts = Vec::new();
    let mut stack: Vec<u64> = node.children.iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        if let Some(n) = tree.get(id) {
            if let Some(name) = &n.name {
                parts.push(name.clone());
            } else {
                stack.extend(n.children.iter().rev());
            }
        }
    }
    parts.join(". ")
}

/// The invisible DOM mirror of the accessibility tree.
pub(crate) struct Mirror {
    root: HtmlElement,
    live: HtmlElement,
    last: Option<AccessTree>,
}

fn hide_visually(style: &web_sys::CssStyleDeclaration) -> Result<(), WebError> {
    for (k, v) in [("opacity", "0"), ("pointer-events", "none"), ("overflow", "hidden"), ("margin", "0")] {
        style.set_property(k, v)?;
    }
    Ok(())
}

impl Mirror {
    /// Creates the mirror and the live region next to the canvas.
    pub(crate) fn new(doc: &Document, canvas: &HtmlCanvasElement) -> Result<Self, WebError> {
        let root: HtmlElement =
            doc.create_element("div")?.dyn_into().map_err(|_| WebError::Js("div".into()))?;
        root.set_attribute("data-ravenrs", "accessibility")?;
        let style = root.style();
        style.set_property("position", "fixed")?;
        hide_visually(&style)?;
        let live: HtmlElement =
            doc.create_element("div")?.dyn_into().map_err(|_| WebError::Js("div".into()))?;
        live.set_attribute("aria-live", "polite")?;
        live.set_attribute("role", "status")?;
        let style = live.style();
        for (k, v) in
            [("position", "fixed"), ("width", "1px"), ("height", "1px"), ("left", "0"), ("top", "0")]
        {
            style.set_property(k, v)?;
        }
        hide_visually(&style)?;
        if let Some(parent) = canvas.parent_node() {
            parent.insert_before(&root, canvas.next_sibling().as_ref())?;
            parent.insert_before(&live, root.next_sibling().as_ref())?;
        }
        Ok(Self { root, live, last: None })
    }

    /// The node id of a mirror element, for clicks from assistive technology.
    pub(crate) fn node_of(target: &Element) -> Option<u64> {
        let mut el = Some(target.clone());
        while let Some(e) = el {
            if let Some(id) = e.id().strip_prefix(ID_PREFIX).and_then(|s| s.parse().ok()) {
                return Some(id);
            }
            el = e.parent_element();
        }
        None
    }

    /// The element whose clicks are routed to the UI.
    pub(crate) fn element(&self) -> &HtmlElement {
        &self.root
    }

    /// Rebuilds the mirror if the tree changed, points the text input at the
    /// focused element and announces new status messages.
    pub(crate) fn update(
        &mut self,
        tree: AccessTree,
        canvas: &HtmlCanvasElement,
        input: &HtmlTextAreaElement,
    ) -> Result<(), WebError> {
        if self.last.as_ref() == Some(&tree) {
            return Ok(());
        }
        let doc = self.root.owner_document().ok_or_else(|| WebError::Js("no document".into()))?;
        let rect = canvas.get_bounding_client_rect();
        let style = self.root.style();
        style.set_property("left", &format!("{}px", rect.left()))?;
        style.set_property("top", &format!("{}px", rect.top()))?;
        style.set_property("width", &format!("{}px", rect.width()))?;
        style.set_property("height", &format!("{}px", rect.height()))?;
        self.root.set_inner_html("");
        if let Some(root) = tree.get(tree.root) {
            for child in &root.children {
                self.append(&doc, &tree, *child, &self.root, (0.0, 0.0))?;
            }
        }
        let active = tree.focused().map(|f| f.active_descendant.unwrap_or(f.id));
        match active {
            Some(id) => input.set_attribute("aria-activedescendant", &format!("{ID_PREFIX}{id}"))?,
            None => input.remove_attribute("aria-activedescendant")?,
        }
        let label = tree.focused().and_then(|f| f.name.clone()).unwrap_or_else(|| "Application".into());
        input.set_attribute("aria-label", &label)?;
        let before: HashSet<u64> =
            self.last.as_ref().map(|t| t.nodes.iter().map(|n| n.id).collect()).unwrap_or_default();
        let fresh: Vec<String> = tree
            .nodes
            .iter()
            .filter(|n| matches!(n.role, Role::Status | Role::Alert) && !before.contains(&n.id))
            .map(|n| spoken_text(&tree, n))
            .filter(|t| !t.is_empty())
            .collect();
        if self.last.is_some() && !fresh.is_empty() {
            self.live.set_text_content(Some(&fresh.join(". ")));
        }
        self.last = Some(tree);
        Ok(())
    }

    /// Appends node `id` under `parent`, whose top-left corner is `origin`
    /// in canvas coordinates.
    fn append(
        &self,
        doc: &Document,
        tree: &AccessTree,
        id: u64,
        parent: &Element,
        origin: (f32, f32),
    ) -> Result<(), WebError> {
        let Some(n) = tree.get(id) else { return Ok(()) };
        let el: HtmlElement =
            doc.create_element("div")?.dyn_into().map_err(|_| WebError::Js("div".into()))?;
        el.set_id(&format!("{ID_PREFIX}{}", n.id));
        for (k, v) in aria_attributes(n) {
            el.set_attribute(k, &v)?;
        }
        let text = match n.role {
            Role::Label => n.name.as_deref(),
            Role::TextInput | Role::MultilineTextInput | Role::ComboBox => n.value.as_deref(),
            _ => None,
        };
        if let Some(text) = text {
            el.set_text_content(Some(text));
        }
        // Positioned where they are drawn, so touch exploration finds them.
        let style = el.style();
        let b = n.bounds;
        style.set_property("position", "absolute")?;
        style.set_property("left", &format!("{}px", b.x() - origin.0))?;
        style.set_property("top", &format!("{}px", b.y() - origin.1))?;
        style.set_property("width", &format!("{}px", b.width()))?;
        style.set_property("height", &format!("{}px", b.height()))?;
        parent.append_child(&el)?;
        for child in &n.children {
            self.append(doc, tree, *child, &el, (b.x(), b.y()))?;
        }
        Ok(())
    }

    /// Removes the mirror from the page.
    pub(crate) fn remove(&self) {
        self.root.remove();
        self.live.remove();
    }
}
