//! The lowest layer of the model, from the entities.

use std::collections::BTreeSet;

use super::node::ModelNode;
use crate::{EntityInfo, FieldInfo, ModelError};

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
    ModelNode::new(id)
        .with("Kind", "ListView")
        .with("Entity", entity.name.as_str())
        .with("Caption", entity.caption.as_str())
        .child(columns)
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
    let simple: Vec<&FieldInfo> =
        shown.iter().copied().filter(|f| !f.kind.is_tall() && !f.kind.is_collection()).collect();
    let tall: Vec<&FieldInfo> = shown.iter().copied().filter(|f| f.kind.is_tall()).collect();
    let lists: Vec<&FieldInfo> = shown.iter().copied().filter(|f| f.kind.is_collection()).collect();
    let mut node = ModelNode::new("Layout");
    let mut index = 0;
    if !simple.is_empty() {
        let columns = if simple.len() > FIELDS_IN_ONE_COLUMN { 2 } else { 1 };
        node = node.child(group("Main", index, columns, &simple));
        index += 1;
    }
    if !tall.is_empty() {
        node = node.child(group("Long", index, 1, &tall));
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
                .with("Editor", field.kind.name())
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
        members.child(
            ModelNode::new(&field.name)
                .with("Caption", field.caption.as_str())
                .with("Kind", field.kind.name())
                .with("Key", field.is_key)
                .with("ReadOnly", field.read_only),
        )
    });
    let mut node = ModelNode::new(&entity.name).with("Caption", entity.caption.as_str()).child(members);
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
