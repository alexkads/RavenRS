//! The lowest layer of the model, from the entities.

use std::collections::BTreeSet;

use super::node::ModelNode;
use crate::{DashboardContent, DashboardInfo, EntityInfo, FieldInfo, ModelError};

/// More simple fields than this and a form's group uses two columns.
const FIELDS_IN_ONE_COLUMN: usize = 4;

/// Every field gets a column, out of sight unless it belongs in this list, so showing
/// one is a matter of its index.
fn list_view(entity: &EntityInfo, id: String, shown: impl Fn(&FieldInfo) -> bool) -> ModelNode {
    let mut seen = 0_i64;
    let mut columns = ModelNode::new("Columns");
    for field in entity.fields.iter().filter(|f| !f.kind.is_collection()) {
        let index = if shown(field) {
            seen += 1;
            seen - 1
        } else {
            -1
        };
        columns = columns
            .child(ModelNode::new(&field.name).with("Caption", field.caption.as_str()).with("Index", index));
    }
    let node = ModelNode::new(id)
        .with("Kind", "ListView")
        .with("Entity", entity.name.as_str())
        .with("Caption", entity.caption.as_str())
        .child(columns);
    let node = match &entity.list_criteria {
        Some(criteria) => node.with("Criteria", criteria.as_str()),
        None => node,
    };
    let node = if entity.list_editable { node.with("AllowEdit", true) } else { node };
    let node = if entity.data_access == crate::DataAccess::Client {
        node
    } else {
        node.with("DataAccess", entity.data_access.name())
    };
    let node = if entity.filters.is_empty() {
        node
    } else {
        node.child(entity.filters.iter().enumerate().fold(ModelNode::new("Filters"), |all, (i, filter)| {
            let item =
                ModelNode::new(&filter.id).with("Caption", filter.caption.as_str()).with("Index", i as i64);
            all.child(match &filter.criteria {
                Some(criteria) => item.with("Filter", criteria.as_str()),
                None => item,
            })
        }))
    };
    let node = entity.editor_settings.iter().fold(node, |mut node, (settings, name, value)| {
        if node.get(settings).is_none() {
            node = node.child(ModelNode::new(settings));
        }
        if let Some(found) = node.get_mut(settings) {
            found.properties.insert(name.clone(), serde_json::Value::String(value.clone()));
        }
        node
    });
    if entity.variants.is_empty() {
        return node;
    }
    let variants =
        entity.variants.iter().enumerate().fold(ModelNode::new("Variants"), |variants, (i, variant)| {
            let item =
                ModelNode::new(&variant.id).with("Caption", variant.caption.as_str()).with("Index", i as i64);
            let item = match &variant.filter {
                Some(filter) => item.with("Filter", filter.as_str()),
                None => item,
            };
            let item = match &variant.editor {
                Some(editor) => item.with("EditorType", editor.as_str()),
                None => item,
            };
            variants.child(if variant.columns.is_empty() {
                item
            } else {
                item.child(
                    variant
                        .columns
                        .iter()
                        .enumerate()
                        .fold(ModelNode::new("Columns"), |columns, (at, field)| {
                            columns.child(ModelNode::new(field).with("Index", at as i64))
                        }),
                )
            })
        });
    node.child(variants)
}

/// What says what a field or an entity is called in the views and the navigation came from it. A
/// caption changed under `Entities`, as a translation does, follows there unless a view has its own.
pub(crate) fn follow_captions(root: &mut ModelNode, declared: &[EntityInfo]) {
    let Some(entities) = root.get("Entities").cloned() else { return };
    let set = |node: &mut ModelNode, was: &str, now: &str| {
        if node.text("Caption") == Some(was) && was != now {
            node.properties.insert("Caption".to_owned(), serde_json::Value::String(now.to_owned()));
        }
    };
    for entity in declared {
        let Some(in_model) = entities.get(&entity.name) else { continue };
        let caption = in_model.text("Caption").unwrap_or(&entity.caption);
        let is_of = |view: &ModelNode| view.text("Entity") == Some(entity.name.as_str());
        if let Some(views) = root.get_mut("Views") {
            for view in views.children.iter_mut().filter(|view| is_of(view)) {
                set(view, &entity.caption, caption);
                for part in ["Columns", "Items"] {
                    let Some(items) = view.get_mut(part) else { continue };
                    for item in &mut items.children {
                        let Some(field) = entity.field_named(&item.id) else { continue };
                        let now = in_model.at(&["Members", &item.id]).and_then(|m| m.text("Caption"));
                        set(item, &field.caption, now.unwrap_or(&field.caption));
                    }
                }
                // A list on its own tab is called what the field is.
                if let Some(tabs) = view.at_mut(&["Layout", "Tabs"]) {
                    for tab in &mut tabs.children {
                        let Some(field) = entity.field_named(&tab.id) else { continue };
                        let now = in_model.at(&["Members", &tab.id]).and_then(|m| m.text("Caption"));
                        set(tab, &field.caption, now.unwrap_or(&field.caption));
                    }
                }
            }
        }
        if let Some(navigation) = root.get_mut("NavigationItems") {
            for place in &mut navigation.children {
                for item in place.children.iter_mut().filter(|item| item.id == entity.name) {
                    set(item, &entity.caption, caption);
                }
            }
        }
    }
}

/// Gives every view of an entity the fields the layers added to it, where it has not said
/// otherwise: a column at the end of its list, and a place on its form with the other simple fields.
pub(crate) fn add_custom_fields(root: &mut ModelNode, declared: &[EntityInfo]) {
    let Some(entities) = root.get("Entities").cloned() else { return };
    for entity in declared {
        let Some(members) = entities.at(&[&entity.name, "Members"]) else { continue };
        let added: Vec<&ModelNode> =
            members.children.iter().filter(|member| FieldInfo::is_custom(member, entity)).collect();
        let Some(views) = root.get_mut("Views").filter(|_| !added.is_empty()) else { continue };
        for member in added {
            let caption = member.text("Caption").unwrap_or(&member.id);
            for view in views.children.iter_mut() {
                if view.id == format!("{}_DetailView", entity.name) {
                    add_to_form(view, member, caption);
                } else if view.text("Entity") == Some(entity.name.as_str()) {
                    let shown = view.id == format!("{}_ListView", entity.name);
                    add_to_list(view, member, caption, shown);
                }
            }
        }
    }
}

fn add_to_list(view: &mut ModelNode, member: &ModelNode, caption: &str, shown: bool) {
    let Some(columns) = view.get_mut("Columns") else { return };
    if columns.get(&member.id).is_some() {
        return;
    }
    let next = columns.children.iter().filter_map(|c| c.integer("Index")).max().unwrap_or(-1) + 1;
    columns.children.push(
        ModelNode::new(&member.id).with("Caption", caption).with("Index", if shown { next } else { -1 }),
    );
}

fn add_to_form(view: &mut ModelNode, member: &ModelNode, caption: &str) {
    let calculated = member.text("Expression").is_some();
    if let Some(items) = view.get_mut("Items")
        && items.get(&member.id).is_none()
    {
        items.children.push(
            ModelNode::new(&member.id)
                .with("Caption", caption)
                .with("Editor", member.text("Kind").unwrap_or("Text"))
                .with("ReadOnly", calculated),
        );
    }
    let Some(layout) = view.get_mut("Layout") else { return };
    if layout.get("Main").is_none() {
        layout
            .children
            .push(ModelNode::new("Main").with("Kind", "Group").with("Index", -1).with("Columns", 1));
    }
    let Some(main) = layout.get_mut("Main") else { return };
    if main.get(&member.id).is_none() {
        let next = main.children.iter().filter_map(|c| c.integer("Index")).max().unwrap_or(-1) + 1;
        main.children.push(ModelNode::new(&member.id).with("Field", member.id.as_str()).with("Index", next));
    }
}

/// Adds a dashboard to a model: its view, and its place in the navigation if it has one.
pub fn add_dashboard(root: &mut ModelNode, dashboard: &DashboardInfo) {
    let items = dashboard.items.iter().enumerate().fold(ModelNode::new("Items"), |items, (i, item)| {
        let node = ModelNode::new(&item.id).with("Caption", item.caption.as_str()).with("Index", i as i64);
        items.child(match &item.content {
            DashboardContent::View(view) => node.with("View", view.as_str()),
            DashboardContent::Control(control) => node.with("Control", control.as_str()),
        })
    });
    let view = ModelNode::new(format!("{}_DashboardView", dashboard.name))
        .with("Kind", "DashboardView")
        .with("Caption", dashboard.caption.as_str())
        .child(items);
    let view = if dashboard.variants.is_empty() {
        view
    } else {
        view.child(dashboard.variants.iter().enumerate().fold(
            ModelNode::new("Variants"),
            |variants, (i, (id, caption, other))| {
                variants.child(
                    ModelNode::new(id)
                        .with("Caption", caption.as_str())
                        .with("Index", i as i64)
                        .with("View", format!("{other}_DashboardView")),
                )
            },
        ))
    };
    if let Some(views) = root.get_mut("Views") {
        views.children.push(view);
    }
    let Some(place) = &dashboard.navigation else { return };
    let mut item = ModelNode::new(&dashboard.name)
        .with("Caption", dashboard.caption.as_str())
        .with("View", format!("{}_DashboardView", dashboard.name));
    if let Some(icon) = &dashboard.icon {
        item = item.with("Icon", icon.as_str());
    }
    let Some(navigation) = root.get_mut("NavigationItems") else { return };
    match navigation.get_mut(place) {
        Some(existing) => existing.children.push(item),
        None => navigation.children.push(ModelNode::new(place).with("Caption", place.as_str()).child(item)),
    }
}

fn group(id: &str, index: i64, columns: i64, fields: &[&FieldInfo]) -> ModelNode {
    fields.iter().enumerate().fold(
        ModelNode::new(id).with("Kind", "Group").with("Index", index).with("Columns", columns),
        |node, (i, field)| {
            node.child(ModelNode::new(&field.name).with("Field", field.name.as_str()).with("Index", i as i64))
        },
    )
}

/// The form's layout: simple fields in a group, in two columns when there are many; fields
/// that take the height they need in a group of their own; one list in a group, several in tabs.
fn layout(entity: &EntityInfo) -> ModelNode {
    let shown: Vec<&FieldInfo> = entity.fields.iter().filter(|f| f.in_detail).collect();
    let simple: Vec<&FieldInfo> = shown
        .iter()
        .copied()
        .filter(|f| !f.kind.is_tall() && !f.kind.is_collection() && f.group.is_none())
        .collect();
    let tall: Vec<&FieldInfo> = shown.iter().copied().filter(|f| f.kind.is_tall()).collect();
    let lists: Vec<&FieldInfo> = shown.iter().copied().filter(|f| f.kind.is_collection()).collect();
    let mut node = ModelNode::new("Layout");
    let mut index = 0;
    if !simple.is_empty() {
        let columns =
            entity.form_columns.map_or(if simple.len() > FIELDS_IN_ONE_COLUMN { 2 } else { 1 }, |columns| {
                i64::from(columns.max(1))
            });
        node = node.child(group("Main", index, columns, &simple));
        index += 1;
    }
    let mut titled: Vec<&str> = Vec::new();
    for field in &shown {
        if let Some(title) = field.group.as_deref()
            && !titled.contains(&title)
        {
            titled.push(title);
        }
    }
    for title in titled {
        let inside: Vec<&FieldInfo> = shown
            .iter()
            .copied()
            .filter(|f| f.group.as_deref() == Some(title) && !f.kind.is_tall() && !f.kind.is_collection())
            .collect();
        let columns = if inside.len() > FIELDS_IN_ONE_COLUMN { 2 } else { 1 };
        node = node.child(group(title, index, columns, &inside).with("Caption", title));
        index += 1;
    }
    if !tall.is_empty() {
        node = node.child(group("Long", index, 1, &tall));
        index += 1;
    }
    for name in &entity.form_actions {
        node = node.child(
            ModelNode::new(format!("Actions_{name}"))
                .with("Kind", "Group")
                .with("Index", index)
                .with("Columns", 1)
                .child(ModelNode::new(name).with("Kind", "ActionContainer").with("Category", name.as_str())),
        );
        index += 1;
    }
    match lists.as_slice() {
        [] => {}
        [one] => node = node.child(group("List", index, 1, &[*one])),
        many => {
            let tabs = many.iter().enumerate().fold(
                ModelNode::new("Tabs").with("Kind", "Tabs").with("Index", index),
                |tabs, (i, field)| {
                    tabs.child(
                        group(&field.name, i as i64, 1, &[*field]).with("Caption", field.caption.as_str()),
                    )
                },
            );
            node = node.child(tabs);
        }
    }
    node
}

fn detail_view(entity: &EntityInfo) -> ModelNode {
    let items = entity.fields.iter().filter(|f| f.in_detail).fold(ModelNode::new("Items"), |items, field| {
        items.child(
            ModelNode::new(&field.name)
                .with("Caption", field.caption.as_str())
                .with("Editor", field.editor.as_deref().unwrap_or(field.kind.name()))
                .with("Wide", field.wide)
                .with("ReadOnly", field.read_only),
        )
    });
    ModelNode::new(format!("{}_DetailView", entity.name))
        .with("Kind", "DetailView")
        .with("Entity", entity.name.as_str())
        .with("Caption", entity.caption.as_str())
        .child(items)
        .child(layout(entity))
}

fn entity_node(entity: &EntityInfo) -> ModelNode {
    let members = entity.fields.iter().fold(ModelNode::new("Members"), |members, field| {
        let member = ModelNode::new(&field.name)
            .with("Caption", field.caption.as_str())
            .with("Kind", field.kind.name())
            .with("Key", field.is_key)
            .with("ReadOnly", field.read_only);
        members.child(match &field.kind {
            crate::FieldKind::Enum(options) => {
                member.child(options.iter().fold(ModelNode::new("Options"), |all, option| {
                    all.child(ModelNode::new(&option.code).with("Caption", option.caption.as_str()))
                }))
            }
            _ => member,
        })
    });
    let mut node = ModelNode::new(&entity.name)
        .with("Caption", entity.caption.as_str())
        .child(members)
        .child(ModelNode::new("Rules"));
    if let Some(field) = entity.default_property() {
        node = node.with("DefaultProperty", field.name.as_str());
    }
    if let Some(icon) = &entity.icon {
        node = node.with("Icon", icon.as_str());
    }
    node
}

/// The model the entities give: their members, a list, a list to pick from and a
/// form for each, and the navigation.
pub fn generate_model(entities: &[EntityInfo]) -> Result<ModelNode, ModelError> {
    let mut names = BTreeSet::new();
    for entity in entities {
        entity.check()?;
        if !names.insert(entity.name.as_str()) {
            return Err(ModelError(format!("the entity {} is declared twice", entity.name)));
        }
    }
    let mut objects = ModelNode::new("Entities");
    let mut views = ModelNode::new("Views");
    let mut navigation = ModelNode::new("NavigationItems");
    for entity in entities {
        objects = objects.child(entity_node(entity));
        let lookup_field = entity.default_property().map(|f| f.name.clone());
        views = views
            .child(list_view(entity, format!("{}_ListView", entity.name), |f| f.in_list))
            .child(list_view(entity, format!("{}_LookupListView", entity.name), |f| {
                f.in_lookup || lookup_field.as_deref() == Some(&f.name)
            }))
            .child(detail_view(entity));
        let Some(place) = &entity.navigation else { continue };
        let mut item = ModelNode::new(&entity.name)
            .with("Caption", entity.caption.as_str())
            .with("View", format!("{}_ListView", entity.name));
        if let Some(icon) = &entity.icon {
            item = item.with("Icon", icon.as_str());
        }
        match navigation.get_mut(place) {
            Some(existing) => existing.children.push(item),
            None => {
                navigation =
                    navigation.child(ModelNode::new(place).with("Caption", place.as_str()).child(item))
            }
        }
    }
    Ok(ModelNode::new("Application").child(objects).child(views).child(navigation))
}

/// What in a model points to nothing, in words for whoever changed it: a view of an
/// entity that is gone, a column of a field that is not there, a menu item with no view.
pub fn check_model(root: &ModelNode) -> Vec<String> {
    let mut problems = Vec::new();
    let empty = ModelNode::default();
    let entities = root.get("Entities").unwrap_or(&empty);
    let views = root.get("Views").unwrap_or(&empty);
    for view in &views.children {
        for variant in view.get("Variants").map_or(&[][..], |n| n.children.as_slice()) {
            if let Some(shown) = variant.text("View")
                && views.get(shown).is_none()
            {
                problems.push(format!("the view {} can change to {shown}, which does not exist", view.id));
            }
        }
        if view.text("Kind") == Some("DashboardView") {
            for item in view.get("Items").map_or(&[][..], |n| n.children.as_slice()) {
                if let Some(shown) = item.text("View")
                    && views.get(shown).is_none()
                {
                    problems.push(format!("the dashboard {} shows {shown}, which does not exist", view.id));
                }
            }
            continue;
        }
        let Some(name) = view.text("Entity") else {
            problems.push(format!("the view {} says nothing about its entity", view.id));
            continue;
        };
        let Some(entity) = entities.get(name) else {
            problems.push(format!("the view {} is of {name}, which does not exist", view.id));
            continue;
        };
        let members = entity.get("Members").unwrap_or(&empty);
        for part in ["Columns", "Items"] {
            for item in view.get(part).map_or(&[][..], |n| n.children.as_slice()) {
                if members.get(&item.id).is_none() {
                    problems
                        .push(format!("the view {} shows {}, which {name} does not have", view.id, item.id));
                }
            }
        }
    }
    fn walk(node: &ModelNode, views: &ModelNode, problems: &mut Vec<String>) {
        for item in &node.children {
            if let Some(view) = item.text("View")
                && views.get(view).is_none()
            {
                problems.push(format!("the navigation item {} opens {view}, which does not exist", item.id));
            }
            walk(item, views, problems);
        }
    }
    walk(root.get("NavigationItems").unwrap_or(&empty), views, &mut problems);
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldKind;

    fn account() -> EntityInfo {
        EntityInfo::new("Account")
            .caption("Conta")
            .navigation("Cadastros")
            .icon("wallet")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
            .field(FieldInfo::new("notes", FieldKind::LongText))
            .field(FieldInfo::new("kind", FieldKind::Text))
    }

    fn visible(view: &ModelNode) -> Vec<&str> {
        view.get("Columns").map_or(vec![], |c| c.in_order().iter().map(|n| n.id.as_str()).collect())
    }

    #[test]
    fn each_entity_gets_a_list_a_lookup_and_a_form() {
        let root = generate_model(&[account()]).expect("valid");
        let views = root.get("Views").expect("views");
        assert_eq!(visible(views.get("Account_ListView").expect("list")), ["name", "kind"]);
        assert_eq!(visible(views.get("Account_LookupListView").expect("lookup")), ["name"]);
        let form = views.get("Account_DetailView").expect("form");
        assert_eq!(form.text("Kind"), Some("DetailView"));
        let id = root.at(&["Views", "Account_ListView", "Columns", "id"]).expect("a hidden column");
        assert_eq!(id.integer("Index"), Some(-1), "out of sight, but there to be shown");
        assert!(check_model(&root).is_empty(), "{:?}", check_model(&root));
    }

    #[test]
    fn the_navigation_groups_entities() {
        let other = EntityInfo::new("Bank")
            .navigation("Cadastros")
            .field(FieldInfo::new("id", FieldKind::Integer).key());
        let hidden = EntityInfo::new("Log").field(FieldInfo::new("id", FieldKind::Integer).key());
        let root = generate_model(&[account(), other, hidden]).expect("valid");
        let group = root.at(&["NavigationItems", "Cadastros"]).expect("the group");
        assert_eq!(group.children.len(), 2);
        assert_eq!(group.get("Account").and_then(|n| n.text("View")), Some("Account_ListView"));
        assert_eq!(group.get("Account").and_then(|n| n.text("Icon")), Some("wallet"));
        assert!(root.at(&["NavigationItems", "Log"]).is_none());
    }

    fn form_layout(entity: EntityInfo) -> ModelNode {
        layout(&entity)
    }

    #[test]
    fn a_form_goes_to_two_columns_with_more_than_four_fields() {
        let few =
            (0..4).fold(account_key(), |e, i| e.field(FieldInfo::new(format!("f{i}"), FieldKind::Text)));
        assert_eq!(form_layout(few).get("Main").and_then(|g| g.integer("Columns")), Some(1));
        let many =
            (0..5).fold(account_key(), |e, i| e.field(FieldInfo::new(format!("f{i}"), FieldKind::Text)));
        assert_eq!(form_layout(many).get("Main").and_then(|g| g.integer("Columns")), Some(2));
    }

    fn account_key() -> EntityInfo {
        EntityInfo::new("A").field(FieldInfo::new("id", FieldKind::Integer).key())
    }

    #[test]
    fn tall_fields_and_lists_get_their_own_place() {
        let list = |name: &str| FieldInfo::new(name, FieldKind::Collection("B".into()));
        let one = account_key().field(FieldInfo::new("notes", FieldKind::LongText)).field(list("items"));
        let form = form_layout(one);
        assert!(form.get("Main").is_none(), "no simple fields");
        assert!(form.get("Long").and_then(|g| g.get("notes")).is_some());
        assert!(form.get("List").and_then(|g| g.get("items")).is_some());
        let two = account_key().field(list("items")).field(list("payments"));
        let form = form_layout(two);
        let tabs = form.get("Tabs").expect("several lists are tabs");
        assert_eq!(tabs.children.len(), 2);
        assert_eq!(tabs.get("payments").and_then(|g| g.text("Caption")), Some("Payments"));
    }

    #[test]
    fn a_bad_declaration_or_a_repeated_entity_stops_it() {
        assert!(generate_model(&[EntityInfo::new("A")]).is_err());
        assert!(generate_model(&[account(), account()]).is_err_and(|e| e.0.contains("twice")));
    }

    #[test]
    fn the_model_says_what_points_to_nothing() {
        let root = generate_model(&[account()]).expect("valid");
        let broken = ModelNode::new("Application")
            .child(root.get("Entities").expect("entities").clone())
            .child(
                ModelNode::new("Views")
                    .child(ModelNode::new("X").with("Entity", "Gone"))
                    .child(ModelNode::new("Y"))
                    .child(
                        ModelNode::new("Z")
                            .with("Entity", "Account")
                            .child(ModelNode::new("Columns").child(ModelNode::new("nope"))),
                    ),
            )
            .child(
                ModelNode::new("NavigationItems")
                    .child(ModelNode::new("G").child(ModelNode::new("I").with("View", "Missing"))),
            );
        let problems = check_model(&broken);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("Gone")));
        assert!(problems.iter().any(|p| p.contains("nope")));
        assert!(problems.iter().any(|p| p.contains("Missing")));
    }
}
