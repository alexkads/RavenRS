//! Where each component is shown working: the manifest of `tools/raven-feature-center`, the
//! showcase every component enters with all its features (#579).
//!
//! `recursos.json` has one entry per component. A shown one names its page and the public methods
//! the page calls; a waiting one names the issue it waits on and the methods it had then. The
//! page's id, which opens it on its own (#587), is read from the `PAGE` and `ID` constants of its
//! file in the embedded sources, so nothing here repeats what the center already says.

use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::RUST_SOURCES;

const MANIFEST: &str = include_str!("../../../tools/raven-feature-center/recursos.json");
/// Where the pages of the center are.
const SOURCES: &str = "tools/raven-feature-center/src/";

#[derive(Debug, Deserialize)]
struct Manifest {
    componentes: Vec<Entry>,
}

/// One component of the manifest, with the names it has there.
#[derive(Debug, Deserialize)]
pub struct Entry {
    #[serde(rename = "crate")]
    pub krate: String,
    #[serde(rename = "tipo")]
    pub name: String,
    #[serde(rename = "pagina")]
    page: Option<String>,
    #[serde(rename = "recursos", default)]
    shown: Vec<String>,
    #[serde(rename = "pendente", default)]
    waiting: bool,
    issue: Option<u64>,
    #[serde(rename = "conhecidos", default)]
    known: Vec<String>,
}

impl Entry {
    /// The page that shows it, once it has one.
    pub fn page(&self) -> Option<&str> {
        self.page.as_deref().filter(|_| !self.waiting)
    }
}

fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        // The coverage script of the check reads the same file, so a broken one never ships.
        serde_json::from_str::<Manifest>(MANIFEST).map(|manifest| manifest.componentes).unwrap_or_default()
    })
}

/// The entry of a component, by its name in the index and its crate there, where the widget
/// families all answer as `ravenrs-widgets`.
pub fn entry(name: &str, krate: &str) -> Option<&'static Entry> {
    entries().iter().find(|entry| {
        entry.name == name
            && (entry.krate == krate
                || (krate == "ravenrs-widgets" && entry.krate.starts_with("ravenrs-widgets")))
    })
}

/// The page a caption names: its file and, for the pages of the components, its id.
///
/// A page of the app framework is an entity of the center's model, declared by
/// `demo("<Entity>", "<caption>", …)` in `model.rs`: its file is that one, and it opens from the
/// navigation, so it has no id.
fn page_file(caption: &str) -> Option<(&'static str, Option<&'static str>)> {
    let declaration = format!("pub const PAGE: &str = {caption:?};");
    let sources = || RUST_SOURCES.iter().filter(|(path, _)| path.starts_with(SOURCES));
    sources()
        .find(|(_, text)| text.contains(&declaration))
        .map(|(path, text)| (*path, string_const(text, "ID")))
        .or_else(|| {
            sources()
                .find(|(path, text)| path.ends_with("/model.rs") && declares_demo(text, caption))
                .map(|(path, _)| (*path, None))
        })
}

/// Whether `text` calls `demo(name, caption, …)` with this caption as the second argument.
fn declares_demo(text: &str, caption: &str) -> bool {
    let quoted = format!("{caption:?},");
    text.match_indices(&quoted).any(|(at, _)| {
        // Before the caption: `demo(`, the entity's name between quotes and a comma.
        let before = text[..at].trim_end();
        let Some(before) = before.strip_suffix(',') else { return false };
        let before = before.trim_end();
        let Some(name_end) = before.strip_suffix('"') else { return false };
        let Some(open) = name_end.rfind('"') else { return false };
        name_end[..open].trim_end().ends_with("demo(")
    })
}

/// The value of `pub const NAME: &str = "…";` in a source file.
fn string_const<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!("pub const {name}: &str = \"");
    let rest = &text[text.find(&pattern)? + pattern.len()..];
    rest.find('"').map(|end| &rest[..end])
}

/// What a client shows about one component.
pub fn describe(entry: &Entry) -> Value {
    let Some(caption) = entry.page() else {
        return json!({
            "status": "waiting",
            "issue": entry.issue,
            "known_features": entry.known,
            "note": "Not in the feature center yet: its page comes with the issue. The methods are still in component_api.",
        });
    };
    let mut value = json!({ "status": "shown", "page": caption, "features": entry.shown });
    match page_file(caption) {
        Some((file, Some(id))) => {
            value["id"] = json!(id);
            value["source"] = json!(file);
            value["run"] = json!(format!(
                "cargo run --manifest-path tools/raven-feature-center/Cargo.toml -- --modo componentes --pagina {id}"
            ));
            value["web"] = json!(format!("index.html?modo=componentes&pagina={id}"));
        }
        Some((file, None)) => {
            value["source"] = json!(file);
            value["run"] = json!(format!(
                "cargo run --manifest-path tools/raven-feature-center/Cargo.toml, then open \"{caption}\" in the navigation"
            ));
        }
        None => {
            value["run"] = json!(format!(
                "cargo run --manifest-path tools/raven-feature-center/Cargo.toml, then open \"{caption}\" in the navigation"
            ));
        }
    }
    value
}

/// The entries matching every filter, with counts.
pub fn search(
    name: Option<&str>,
    query: Option<&str>,
    status: Option<&str>,
    limit: usize,
) -> Result<Value, String> {
    if let Some(status) = status
        && !["shown", "waiting"].contains(&status)
    {
        return Err(format!("unknown status '{status}': use 'shown' or 'waiting'"));
    }
    if let Some(name) = name {
        let found: Vec<&Entry> =
            entries().iter().filter(|entry| entry.name.eq_ignore_ascii_case(name.trim())).collect();
        if found.is_empty() {
            let lower = name.trim().to_lowercase();
            let near: Vec<&str> = entries()
                .iter()
                .map(|entry| entry.name.as_str())
                .filter(|candidate| {
                    let candidate = candidate.to_lowercase();
                    candidate.contains(&lower) || lower.contains(&candidate)
                })
                .take(10)
                .collect();
            return Err(format!(
                "'{name}' is not in tools/raven-feature-center/recursos.json, which lists every public component; did you mean one of {near:?}? Use list_components or search_api to find its name."
            ));
        }
        let matches: Vec<Value> = found.into_iter().map(listed).collect();
        return Ok(json!({ "total": matches.len(), "components": matches }));
    }

    let query = query.map(str::to_lowercase);
    let (mut shown, mut waiting) = (0, 0);
    let mut matches = Vec::new();
    for entry in entries() {
        let entry_status = if entry.page().is_some() { "shown" } else { "waiting" };
        if status.is_some_and(|status| status != entry_status) {
            continue;
        }
        let text = format!("{} {} {}", entry.name, entry.krate, entry.page.as_deref().unwrap_or_default());
        if query.as_ref().is_some_and(|query| !text.to_lowercase().contains(query)) {
            continue;
        }
        if entry_status == "shown" {
            shown += 1;
        } else {
            waiting += 1;
        }
        matches.push(entry);
    }
    let total = matches.len();
    // The shown ones first: they are what can be opened now.
    matches.sort_by_key(|entry| entry.page().is_none());
    let components: Vec<Value> = matches.into_iter().take(limit).map(listed).collect();
    Ok(json!({
        "total": total,
        "shown": shown,
        "waiting": waiting,
        "components": components,
        "modes": "cargo run --manifest-path tools/raven-feature-center/Cargo.toml opens the whole center (sign in as ana, bia or caio, password raven-demo); --modo componentes opens only the pages of the components, --modo playground the examples, and --incorporado only the page",
    }))
}

fn listed(entry: &Entry) -> Value {
    let mut value = describe(entry);
    value["name"] = json!(entry.name);
    value["crate"] = json!(entry.krate);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_parses_and_every_shown_page_has_a_file() {
        assert!(entries().len() > 100, "{}", entries().len());
        for entry in entries() {
            let Some(caption) = entry.page() else { continue };
            assert!(page_file(caption).is_some(), "{}: no file declares the page {caption:?}", entry.name);
        }
    }

    #[test]
    fn a_page_of_the_model_is_found_in_its_file_and_opens_from_the_navigation() {
        // ListIndicator is shown on an entity of the model, not on a page of its own.
        let (file, id) = page_file("Regras de aparência").expect("the model page");
        assert!(file.ends_with("/model.rs"), "{file}");
        assert_eq!(id, None);
        assert_eq!(page_file("Página que não existe"), None);
    }

    #[test]
    fn a_page_of_a_component_has_its_id() {
        let cables = entries().iter().find(|entry| entry.name == "CablePlug").unwrap();
        assert_eq!(describe(cables)["id"], "CablePlugDemo");
    }
}
