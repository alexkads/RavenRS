//! Where each component is shown working: the manifest of `examples/feature-center`, the
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

const MANIFEST: &str = include_str!("../../../examples/feature-center/recursos.json");
/// Where the pages of the center are.
const SOURCES: &str = "examples/feature-center/src/";

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
fn page_file(caption: &str) -> Option<(&'static str, Option<&'static str>)> {
    let declaration = format!("pub const PAGE: &str = {caption:?};");
    RUST_SOURCES
        .iter()
        .find(|(path, text)| path.starts_with(SOURCES) && text.contains(&declaration))
        .map(|(path, text)| (*path, string_const(text, "ID")))
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
            value["run"] = json!(format!("cargo run -p feature-center -- --modo componentes --pagina {id}"));
            value["web"] = json!(format!("index.html?modo=componentes&pagina={id}"));
        }
        Some((file, None)) => {
            value["source"] = json!(file);
            value["run"] =
                json!(format!("cargo run -p feature-center, then open \"{caption}\" in the navigation"));
        }
        None => {
            value["run"] =
                json!(format!("cargo run -p feature-center, then open \"{caption}\" in the navigation"));
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
                "'{name}' is not in examples/feature-center/recursos.json, which lists every public component; did you mean one of {near:?}? Use list_components or search_api to find its name."
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
        "modes": "cargo run -p feature-center opens the whole center (sign in as ana, bia or caio, password raven-demo); --modo componentes opens only the pages of the components, --modo playground the examples, and --incorporado only the page",
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
    fn a_page_of_a_component_has_its_id() {
        let cables = entries().iter().find(|entry| entry.name == "CablePlug").unwrap();
        assert_eq!(describe(cables)["id"], "CablePlugDemo");
    }
}
