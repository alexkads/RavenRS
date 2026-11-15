//! Looking at JSON items four ways: a table with a column per key, a tree that opens, the JSON
//! itself and the schema of what the items hold. The same item and the same field stay chosen
//! from one way to the next, and the field chosen is said as a path (`cliente.nome`,
//! `itens[0].preço`) the app can turn into an expression.
//!
//! A field that is absent and one that is `null` are told apart everywhere. Whole numbers stay
//! exact, as the JSON had them. A list longer than the limit shows its first items and says so.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_layout::Align;
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle};
use serde_json::Value;

use crate::context::{BuildCx, NodeId, Semantics};
use crate::tree::{TreeNode, TreeView};
use crate::{
    AnyView, Badge, Button, ButtonSize, ButtonStyle, Column, FieldDrag, HStack, IconButton, IconName,
    ScrollView, SegmentedControl, Spacer, Table, Text, VStack, View,
};

/// How many items, and how many entries of a list inside one, are shown before the rest is
/// left out, unless [`DataInspector::limit`] says otherwise.
pub const SAMPLE: usize = 1000;

/// The path of a field inside an item: keys joined by dots, rows in brackets, and a key that
/// is not a plain word in quotes, as `cliente.nome`, `itens[0].preço` or `["nome completo"]`.
pub fn field_path(steps: &[PathStep]) -> String {
    let mut out = String::new();
    for step in steps {
        match step {
            PathStep::Row(i) => out.push_str(&format!("[{i}]")),
            PathStep::Key(k) if plain(k) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(k);
            }
            PathStep::Key(k) => out.push_str(&format!("[{}]", Value::String(k.clone()))),
        }
    }
    out
}

/// A key that reads as it is in a path: letters, digits and `_`, not starting with a digit.
fn plain(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// One step of a [`field_path`]: a key of an object or a row of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum PathStep {
    /// A key of an object.
    Key(String),
    /// A row of a list.
    Row(usize),
}

/// Hears the path of a field chosen.
type Pick = Rc<dyn Fn(&str)>;

/// Looks at JSON items as a table, a tree, the JSON itself or their schema.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DataInspector {
    items: Rc<Vec<Value>>,
    total: usize,
    mode: Option<Binding<usize>>,
    item: Option<Binding<usize>>,
    selected: Option<Binding<Option<String>>>,
    on_pick: Option<Pick>,
    limit: usize,
    label: Option<String>,
}

impl std::fmt::Debug for DataInspector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataInspector").field("items", &self.items.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DataInspector {
    /// The items of the JSON `text`: each entry of a list, or the one object. Text that is not
    /// JSON shows as no items.
    pub fn new(text: impl Into<String>) -> Self {
        let value: Value = serde_json::from_str(&text.into()).unwrap_or(Value::Null);
        let items = match value {
            Value::Array(items) => items,
            Value::Null => Vec::new(),
            other => vec![other],
        };
        Self::of(items)
    }

    /// The items as the app has them.
    pub fn of(items: Vec<Value>) -> Self {
        Self {
            total: items.len(),
            items: Rc::new(items),
            mode: None,
            item: None,
            selected: None,
            on_pick: None,
            limit: SAMPLE,
            label: None,
        }
    }

    /// Which way it shows them: 0 the table, 1 the tree, 2 the JSON, 3 the schema.
    pub fn mode(mut self, mode: Binding<usize>) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Which item the tree and the JSON show, from 0.
    pub fn item(mut self, item: Binding<usize>) -> Self {
        self.item = Some(item);
        self
    }

    /// The path of the field chosen, the same in every way of showing.
    pub fn selected(mut self, selected: Binding<Option<String>>) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Hears the path of a field chosen in the tree or the schema.
    pub fn on_pick(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    /// How many items, and entries of a list inside one, it shows before saying the rest was
    /// left out.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    /// What screen readers call it.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A value as a cell says it: text as it is, numbers exact, a list or an object by its size.
fn cell(value: Option<&Value>, labels: &crate::Labels) -> String {
    match value {
        None => labels.inspector_absent.clone(),
        Some(Value::Null) => "null".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => format!("[{}]", a.len()),
        Some(Value::Object(o)) => format!("{{{}}}", o.len()),
        Some(other) => other.to_string(),
    }
}

/// The tree of `value`, its items keyed by their paths, lists cut at `limit`.
fn tree(
    steps: &mut Vec<PathStep>,
    name: String,
    value: &Value,
    limit: usize,
    labels: &crate::Labels,
) -> TreeNode {
    let key = field_path(steps);
    match value {
        Value::Object(map) => {
            let mut node = TreeNode::new(key, format!("{name} {{{}}}", map.len()));
            for (k, v) in map {
                steps.push(PathStep::Key(k.clone()));
                node = node.child(tree(steps, k.clone(), v, limit, labels));
                steps.pop();
            }
            node
        }
        Value::Array(list) => {
            let mut node = TreeNode::new(key.clone(), format!("{name} [{}]", list.len()));
            for (i, v) in list.iter().enumerate().take(limit) {
                steps.push(PathStep::Row(i));
                node = node.child(tree(steps, format!("[{i}]"), v, limit, labels));
                steps.pop();
            }
            if list.len() > limit {
                let left = labels.inspector_more.replace("{0}", &(list.len() - limit).to_string());
                node = node.child(TreeNode::new(format!("{key}…"), left).disabled(true));
            }
            node
        }
        leaf => TreeNode::new(key, format!("{name}: {}", cell(Some(leaf), labels))),
    }
}

/// The keys that reach each path of `value`, by the path as people read it.
fn keys_of(value: &Value, steps: &mut Vec<PathStep>, out: &mut BTreeMap<String, Vec<String>>) {
    if !steps.is_empty() {
        let keys = steps
            .iter()
            .map(|s| match s {
                PathStep::Key(k) => k.clone(),
                PathStep::Row(i) => i.to_string(),
            })
            .collect();
        out.insert(field_path(steps), keys);
    }
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                steps.push(PathStep::Key(k.clone()));
                keys_of(v, steps, out);
                steps.pop();
            }
        }
        Value::Array(list) => {
            for (i, v) in list.iter().enumerate().take(SAMPLE) {
                steps.push(PathStep::Row(i));
                keys_of(v, steps, out);
                steps.pop();
            }
        }
        _ => {}
    }
}

/// The field chosen, as a tag to carry with the pointer to where an expression is written.
struct Carry {
    field: FieldDrag,
    words: String,
}

impl View for Carry {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = cx.child(
            &HStack::new((
                crate::Icon::new(IconName::MousePointer),
                Badge::new(self.field.path.clone()).tone(crate::Tone::Accent),
            ))
            .spacing(6.0)
            .align(Align::Center),
        );
        cx.focusable(node, 6.0);
        cx.semantics(node, Semantics::new(ravenrs_core::Role::Button).name(self.words.clone()));
        cx.drag_source(node, self.field.payload());
        node
    }
}

/// Every path of the items, with the kinds seen at it and in how many items it is.
fn schema(items: &[Value]) -> BTreeMap<String, (BTreeSet<String>, usize)> {
    fn walk(
        steps: &mut Vec<PathStep>,
        value: &Value,
        seen: &mut BTreeMap<String, (BTreeSet<String>, usize)>,
        first: bool,
    ) {
        if !steps.is_empty() {
            let entry = seen.entry(field_path(steps)).or_default();
            entry.0.insert(kind_word(value).to_owned());
            if first {
                entry.1 += 1;
            }
        }
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    steps.push(PathStep::Key(k.clone()));
                    walk(steps, v, seen, first);
                    steps.pop();
                }
            }
            // The entries of a list share one path, the first entry's, so a list of records
            // reads as its records.
            Value::Array(list) => {
                for (i, v) in list.iter().enumerate() {
                    steps.push(PathStep::Row(0));
                    walk(steps, v, seen, first && i == 0);
                    steps.pop();
                }
            }
            _ => {}
        }
    }
    let mut seen = BTreeMap::new();
    for item in items {
        walk(&mut Vec::new(), item, &mut seen, true);
    }
    seen
}

/// The kind of a value, as the schema keys it before the words.
fn kind_word(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "text",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

impl View for DataInspector {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let own_mode = cx.use_state(|| 0_usize);
        let own_item = cx.use_state(|| 0_usize);
        let own_selected = cx.use_state(|| None::<String>);
        let expanded = cx.use_state(BTreeSet::<String>::new);
        let mode = self.mode.clone().unwrap_or_else(|| own_mode.binding());
        let item = self.item.clone().unwrap_or_else(|| own_item.binding());
        let selected = self.selected.clone().unwrap_or_else(|| own_selected.binding());

        let shown = self.items.len().min(self.limit);
        let at = item.get().min(shown.saturating_sub(1));
        let mut parts: Vec<AnyView> = Vec::new();
        parts.push(Box::new(
            SegmentedControl::new(
                mode.clone(),
                [
                    labels.inspector_table.clone(),
                    labels.inspector_tree.clone(),
                    labels.inspector_json.clone(),
                    labels.inspector_schema.clone(),
                ],
            )
            .label(self.label.clone().unwrap_or_else(|| labels.inspector_mode.clone())),
        ));
        if self.total > shown {
            parts.push(Box::new(
                Text::new(
                    labels
                        .inspector_sampled
                        .replace("{0}", &shown.to_string())
                        .replace("{1}", &self.total.to_string()),
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::Warning),
            ));
        }
        if self.items.is_empty() {
            parts.push(Box::new(Text::new(labels.inspector_empty.clone()).foreground(Semantic::InkMuted)));
            return cx.child(&VStack::new(parts).spacing(8.0).align(Align::Stretch));
        }
        // Tree and JSON show one item at a time.
        if matches!(mode.get(), 1 | 2) && shown > 1 {
            let (back, next) = (item.clone(), item.clone());
            let last = shown - 1;
            parts.push(Box::new(
                HStack::new((
                    IconButton::new(IconName::ChevronLeft)
                        .label(labels.inspector_previous.clone())
                        .size(ButtonSize::Small)
                        .style(ButtonStyle::Ghost)
                        .disabled(at == 0)
                        .on_click(move || back.set(at.saturating_sub(1))),
                    Text::new(
                        labels
                            .inspector_item
                            .replace("{0}", &(at + 1).to_string())
                            .replace("{1}", &shown.to_string()),
                    )
                    .style(TextStyle::Callout),
                    IconButton::new(IconName::ChevronRight)
                        .label(labels.inspector_next.clone())
                        .size(ButtonSize::Small)
                        .style(ButtonStyle::Ghost)
                        .disabled(at == last)
                        .on_click(move || next.set((at + 1).min(last))),
                    Spacer::new(),
                ))
                .spacing(4.0)
                .align(Align::Center),
            ));
        }
        let items = &self.items[..shown];
        let body: AnyView = match mode.get() {
            1 => {
                let picked = selected.clone();
                let on_pick = self.on_pick.clone();
                let roots: Vec<TreeNode> = match &items[at] {
                    Value::Object(map) => map
                        .iter()
                        .map(|(k, v)| {
                            tree(&mut vec![PathStep::Key(k.clone())], k.clone(), v, self.limit, &labels)
                        })
                        .collect(),
                    other => vec![tree(&mut Vec::new(), "item".to_owned(), other, self.limit, &labels)],
                };
                // A pick in the tree is heard as the path chosen.
                let read = picked.clone();
                let heard = Binding::new(
                    move || read.get(),
                    move |path: Option<String>| {
                        if let (Some(path), Some(on_pick)) = (&path, &on_pick) {
                            on_pick(path);
                        }
                        picked.set(path);
                    },
                );
                Box::new(
                    TreeView::new(roots)
                        .selected(heard)
                        .expanded(expanded.binding())
                        .label(self.label.clone().unwrap_or_else(|| labels.inspector_tree.clone())),
                )
            }
            2 => Box::new(ScrollView::new((Text::new(
                serde_json::to_string_pretty(&items[at]).unwrap_or_default(),
            )
            .style(TextStyle::Callout),))),
            3 => {
                let mut table = Table::new([
                    Column::new(labels.inspector_path.clone()),
                    Column::new(labels.inspector_kind.clone()),
                    Column::new(labels.inspector_in.clone()),
                ]);
                for (path, (kinds, count)) in schema(items) {
                    let words: Vec<String> = kinds
                        .iter()
                        .map(|k| match k.as_str() {
                            "null" => "null".to_owned(),
                            "boolean" => labels.inspector_boolean.clone(),
                            "number" => labels.inspector_number.clone(),
                            "text" => labels.inspector_text.clone(),
                            "list" => labels.inspector_list.clone(),
                            _ => labels.inspector_object.clone(),
                        })
                        .collect();
                    let mark = if selected.get().as_deref() == Some(path.as_str()) { "▸ " } else { "" };
                    table = table.row([
                        format!("{mark}{path}"),
                        words.join(" · "),
                        labels
                            .inspector_present
                            .replace("{0}", &count.to_string())
                            .replace("{1}", &items.len().to_string()),
                    ]);
                }
                Box::new(table)
            }
            _ => {
                // A column per key, in the order the keys sort (as JSON objects keep them).
                let mut keys: Vec<String> = Vec::new();
                for it in items {
                    if let Value::Object(map) = it {
                        for k in map.keys() {
                            if !keys.contains(k) {
                                keys.push(k.clone());
                            }
                        }
                    }
                }
                let mut table = Table::new(keys.iter().map(|k| Column::new(k.clone())));
                for it in items {
                    table = table.row(keys.iter().map(|k| cell(it.get(k.as_str()), &labels)));
                }
                Box::new(table)
            }
        };
        parts.push(body);
        if let Some(path) = selected.get() {
            parts.push(Box::new(
                Text::new(labels.inspector_chosen.replace("{0}", &path))
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ));
            // The field chosen can be carried to an expression, or copied as one from the keyboard.
            let mut keys = BTreeMap::new();
            keys_of(&items[at], &mut Vec::new(), &mut keys);
            if let Some(found) = keys.get(&path) {
                let field = FieldDrag::new(path.clone(), crate::field_expression(found));
                let (clipboard, expression) = (cx.clipboard(), field.expression.clone());
                parts.push(Box::new(
                    HStack::new((
                        Carry { words: labels.inspector_drag.replace("{0}", &path), field },
                        Button::new(labels.inspector_copy.clone())
                            .icon(IconName::Copy)
                            .style(ButtonStyle::Secondary)
                            .size(ButtonSize::Small)
                            .on_click(move || clipboard.write(&expression)),
                    ))
                    .spacing(8.0)
                    .align(Align::Center),
                ));
            }
        }
        cx.child(&VStack::new(parts).spacing(8.0).align(Align::Stretch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_quotes_what_is_not_a_plain_word() {
        let steps =
            [PathStep::Key("cliente".into()), PathStep::Key("nome completo".into()), PathStep::Row(2)];
        assert_eq!(field_path(&steps), r#"cliente["nome completo"][2]"#);
        assert_eq!(
            field_path(&[PathStep::Key("itens".into()), PathStep::Row(0), PathStep::Key("preço".into())]),
            "itens[0].preço"
        );
    }

    #[test]
    fn the_schema_counts_in_how_many_items_a_field_is() {
        let items = vec![serde_json::json!({"a": 1, "b": null}), serde_json::json!({"a": "x"})];
        let seen = schema(&items);
        assert_eq!(seen["a"].1, 2);
        assert_eq!(seen["a"].0.len(), 2, "a number and a text");
        assert_eq!(seen["b"].1, 1, "absent in the second");
    }
}
