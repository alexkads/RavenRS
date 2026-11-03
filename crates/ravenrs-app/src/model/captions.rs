//! What people see in the model, in and out of a file for translating.

use serde_json::{Map, Value as Json};

use super::layers::Diff;
use super::node::ModelNode;
use crate::{EntityInfo, ModelError};

/// A cell of a CSV line, quoted when it has to be.
fn cell(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// The cells of a whole CSV text, line by line, where a quoted cell can hold a line break.
fn lines_of(text: &str) -> Vec<Vec<String>> {
    let mut lines = Vec::new();
    let (mut line, mut current, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
            }
            ('"', _) => quoted = !quoted,
            (',', false) => line.push(std::mem::take(&mut current)),
            ('\n', false) => {
                line.push(std::mem::take(&mut current));
                lines.push(std::mem::take(&mut line));
            }
            ('\r', false) => {}
            (c, _) => current.push(c),
        }
    }
    if !current.is_empty() || !line.is_empty() {
        line.push(current);
        lines.push(line);
    }
    lines
}

fn add(path: &[&str], node: &ModelNode, lines: &mut Vec<String>) {
    if let Some(caption) = node.text("Caption") {
        lines.push(format!("{},{},", cell(&path.join("/")), cell(caption)));
    }
}

fn walk(path: &mut Vec<String>, node: &ModelNode, lines: &mut Vec<String>) {
    let parts: Vec<&str> = path.iter().map(String::as_str).collect();
    add(&parts, node, lines);
    for child in node.in_order_all() {
        path.push(child.id.clone());
        walk(path, child, lines);
        path.pop();
    }
}

/// A line per caption worth translating. What follows from an entity is left out, since it comes
/// along: the views of the entity, their columns, and its place in the navigation.
pub(crate) fn captions_of(root: &ModelNode, entities: &[EntityInfo], lines: &mut Vec<String>) {
    if let Some(all) = root.get("Entities") {
        for entity in &all.children {
            walk(&mut vec!["Entities".to_owned(), entity.id.clone()], entity, lines);
        }
    }
    if let Some(views) = root.get("Views") {
        for view in &views.children {
            let is_dashboard = view.text("Kind") == Some("DashboardView");
            if is_dashboard {
                add(&["Views", &view.id], view, lines);
            }
            for part in &view.children {
                if part.id == "Columns" || (part.id == "Items" && !is_dashboard) {
                    continue;
                }
                walk(&mut vec!["Views".to_owned(), view.id.clone(), part.id.clone()], part, lines);
            }
        }
    }
    if let Some(navigation) = root.get("NavigationItems") {
        for place in &navigation.children {
            if entities.iter().any(|entity| entity.name == place.id) {
                continue;
            }
            add(&["NavigationItems", &place.id], place, lines);
            for item in &place.children {
                if !entities.iter().any(|entity| entity.name == item.id) {
                    add(&["NavigationItems", &place.id, &item.id], item, lines);
                }
            }
        }
    }
}

/// Writes a caption at this path of ids, making the nodes on the way.
fn place(map: &mut Map<String, Json>, path: &[&str], caption: &str) {
    match path.split_first() {
        None => {
            map.insert("Caption".to_owned(), Json::String(caption.to_owned()));
        }
        Some((id, rest)) => {
            let entry = map.entry((*id).to_owned()).or_insert_with(|| Json::Object(Map::new()));
            if let Json::Object(next) = entry {
                place(next, rest, caption);
            }
        }
    }
}

/// The difference a file of captions asks for, from its filled-in lines.
pub(crate) fn translation_of(csv: &str) -> Result<Diff, ModelError> {
    let mut root = Map::new();
    for (at, line) in lines_of(csv).iter().enumerate().skip(1) {
        let [path, _, translation] = line.as_slice() else {
            return Err(ModelError(format!(
                "line {} of the captions has {} cells, and not 3",
                at + 1,
                line.len()
            )));
        };
        if translation.is_empty() {
            continue;
        }
        let ids: Vec<&str> = path.split('/').collect();
        place(&mut root, &ids, translation);
    }
    Diff::parse(&Json::Object(root).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_with_commas_and_quotes_go_there_and_back() {
        let line = format!("a,{},{}\n", cell("b, \"c\""), cell("d\ne"));
        assert_eq!(lines_of(&line), vec![vec!["a".to_owned(), "b, \"c\"".to_owned(), "d\ne".to_owned()]]);
    }

    #[test]
    fn only_lines_filled_in_change_anything() {
        let diff =
            translation_of("path,caption,translation\nEntities/A,A,Conta\nEntities/B,B,\n").expect("reads");
        assert!(diff.to_json().contains("Conta"));
        assert!(!diff.to_json().contains("\"B\""));
        assert!(translation_of("path,caption,translation\nx,y\n").is_err());
    }
}
