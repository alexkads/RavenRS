//! What the book writes for the design tool, measured from what RavenRS paints.

use raven_component_book::export::{anatomy_of, component_json, measure_entry, svg};
use raven_component_book::index;
use serde_json::Value;

fn entry(name: &str) -> index::Entry {
    index::entries().into_iter().find(|e| e.name() == name).expect(name)
}

fn row<'a>(rows: &'a [Value], name: &str) -> &'a Value {
    rows.iter().find(|r| r["name"] == name).unwrap_or_else(|| panic!("{name} in {rows:#?}"))
}

#[test]
fn a_button_measures_as_ravenrs_draws_it() {
    let stories = measure_entry(&entry("Button"));
    let rows = anatomy_of(&stories);
    let save = row(&rows, "Save");
    assert_eq!(save["height"], 40.0);
    assert_eq!(save["radius"], "radius-md");
    assert_eq!(save["fill"], "accent");
    assert_eq!(save["text"]["font"], "Inter");
    assert_eq!(save["text"]["weight"], 600);
    assert_eq!(save["text"]["size"], 15.0);
    assert_eq!(save["text"]["color"], "on-accent");
    let cancel = row(&rows, "Cancel");
    assert_eq!(cancel["fill"], "bg-300");
    assert_eq!(cancel["border"]["color"], "border");
    // The sizes, from the story that shows them side by side; Medium is the Save button's.
    for (name, height) in [("Mini", 22.0), ("Small", 32.0), ("Large", 48.0)] {
        assert_eq!(row(&rows, name)["height"], height, "{name}");
    }
    let delete = row(&rows, "Delete");
    assert_eq!(delete["icon"]["name"], "trash-2");
}

#[test]
fn the_json_of_a_component_has_its_settings_stories_and_both_themes() {
    let entry = entry("Button");
    let stories = measure_entry(&entry);
    let json = component_json(&entry, &stories);
    assert_eq!(json["title"], "Actions and inputs/Button");
    assert_eq!(json["pending"], false);
    let style = json["argTypes"].as_array().unwrap().iter().find(|a| a["name"] == "style").unwrap();
    assert_eq!(style["control"]["type"], "select");
    let first = &json["stories"][0];
    assert!(first["themes"]["dark"]["paint"].as_array().is_some_and(|p| !p.is_empty()));
    assert!(first["themes"]["light"]["parts"].as_array().is_some_and(|p| !p.is_empty()));
}

#[test]
fn the_preview_is_vector_with_colors_by_token() {
    let stories = measure_entry(&entry("Button"));
    let picture = svg(&stories[0].themes[0].1);
    assert!(picture.starts_with("<svg"));
    assert!(picture.contains("var(--accent)"));
    assert!(picture.contains(">Save</text>"));
    assert!(!picture.contains("<image"), "no pictures, only shapes");
}

#[test]
fn the_design_system_has_a_card_a_preview_and_a_json_for_every_component() {
    let dir = std::env::temp_dir().join(format!("raven-component-book-test-{}", std::process::id()));
    let written = raven_component_book::export::write_claude_design(&dir, "test").expect("written");
    let project = dir.join("project");
    for entry in index::entries() {
        let folder = project.join("components").join(entry.name());
        let preview = std::fs::read_to_string(folder.join("preview.html")).expect("preview");
        assert!(preview.starts_with("<!-- @dsCard group="), "{}", entry.name());
        // The design tool reads a preview of at most 256 KB.
        assert!(preview.len() <= 256 * 1024, "{} preview is {} bytes", entry.name(), preview.len());
        assert!(folder.join("README.md").exists());
        assert!(folder.join(format!("{}.json", entry.name())).exists());
    }
    let readme = std::fs::read_to_string(project.join("README.md")).expect("readme");
    assert!(readme.contains("Use only the components listed below"));
    assert!(readme.contains("| `Button` |"));
    let tokens: Value =
        serde_json::from_str(&std::fs::read_to_string(project.join("tokens.json")).unwrap()).unwrap();
    assert!(tokens["type"]["fonts"].as_array().unwrap().iter().any(|f| f["family"] == "RavenIcons"));
    assert_eq!(written.len(), 3 + index::entries().len() * 3);
    std::fs::remove_dir_all(&dir).ok();
}
