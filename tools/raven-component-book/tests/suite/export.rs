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
    // The index, the guide, the tokens and their CSS, three fonts and the cover, then three
    // files per component.
    assert_eq!(written.len(), 8 + index::entries().len() * 3);
    // The card says how a component sizes, and the preview shows it beside the narrowest parent.
    let field = project.join("components/TextField");
    let card = std::fs::read_to_string(field.join("README.md")).expect("card");
    assert!(card.contains("## Sizing\n\nFills the width of its parent"), "{card}");
    let preview = std::fs::read_to_string(field.join("preview.html")).expect("preview");
    assert!(preview.contains("320 pt parent"), "the reference beside the narrowest parent");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_button_hugs_its_label_in_every_parent() {
    let entry = entry("Button");
    let json = component_json(&entry, &measure_entry(&entry));
    let sizing = &json["stories"][0]["sizing"];
    assert_eq!(sizing["width"], "hug", "{sizing}");
    for parent in ["320", "760", "1200"] {
        assert_eq!(sizing["sizes"][parent], serde_json::json!([72.0, 40.0]), "{parent}: {sizing}");
    }
    assert_eq!(json["sizing"], *sizing, "the component sizes as its first story");
}

#[test]
fn a_text_field_fills_the_width_of_its_parent() {
    let entry = entry("TextField");
    let stories = measure_entry(&entry);
    let sizing = component_json(&entry, &stories)["sizing"].clone();
    assert_eq!(sizing["width"], "fill", "{sizing}");
    // The stage leaves a margin of 16 on each side.
    for (parent, width) in [("320", 288.0), ("760", 728.0), ("1200", 1168.0)] {
        assert_eq!(sizing["sizes"][parent][0], width, "{parent}: {sizing}");
    }
    assert!(
        stories[0].fit.sentence().starts_with("Fills the width of its parent"),
        "{}",
        stories[0].fit.sentence()
    );
}

#[test]
fn what_a_component_declares_wins_over_what_is_measured() {
    use raven_component_book::measure::fit_of;
    use ravenrs::widgets::book::{Extent, Sizing};
    let sizes = [(320.0, (288.0, 40.0)), (760.0, (300.0, 40.0)), (1200.0, (300.0, 40.0))];
    let measured = fit_of(&Sizing::new(), &sizes, None);
    assert_eq!(measured.width, Extent::Hug);
    assert_eq!(measured.narrows_to, Some(288.0), "the phone squeezed it");
    let declared = fit_of(&Sizing::new().width(Extent::Fixed(300.0)).min_width(120.0), &sizes, Some(728.0));
    assert_eq!(declared.width, Extent::Fixed(300.0));
    assert_eq!(declared.min_width, Some(120.0));
    assert_eq!(declared.json()["width"]["fixed"], 300.0);
    assert!(!declared.stretches, "a declared width does not stretch");
    // Hugging its content, but taking the whole column of a parent that stretches it.
    assert!(fit_of(&Sizing::new(), &sizes, Some(728.0)).stretches);
    // Filling the phone and the reference, stopping short of the wide window: a limit.
    let capped = fit_of(
        &Sizing::new(),
        &[(320.0, (288.0, 40.0)), (760.0, (728.0, 40.0)), (1200.0, (960.0, 40.0))],
        None,
    );
    assert_eq!((capped.width, capped.max_width), (Extent::Fill, Some(960.0)));
}

#[test]
fn the_anatomy_has_the_surfaces_and_texts_around_the_controls() {
    let rows = anatomy_of(&measure_entry(&entry("Dialog")));
    let panel = rows.iter().find(|r| r["part"] == "Dialog").unwrap_or_else(|| panic!("{rows:#?}"));
    assert_eq!(panel["radius"], "radius-xl");
    assert_eq!(panel["fill"], "bg-200");
    assert_eq!(panel["padding_left"], "space-6", "the title starts 24 in");
    let title = rows.iter().find(|r| r["part"] == "Heading").expect("title");
    assert_eq!((title["text"]["size"].clone(), title["text"]["weight"].clone()), (22.0.into(), 600.into()));
    let rows = anatomy_of(&measure_entry(&entry("FormField")));
    let label = row(&rows, "Work email");
    assert!(
        rows.iter().any(|r| r["part"] == "Label" && r["text"]["size"] == 13.0 && r["text"]["weight"] == 600),
        "{label}"
    );
}

#[test]
fn a_form_field_hugs_alone_and_takes_the_column_of_a_form() {
    let stories = measure_entry(&entry("FormField"));
    let fit = &stories[0].fit;
    assert_eq!(fit.width, ravenrs::widgets::book::Extent::Hug, "{fit:?}");
    assert!(fit.stretches, "{fit:?}");
    assert!(fit.sentence().contains("it takes the whole width"), "{}", fit.sentence());
}

#[test]
fn an_open_dialog_sizes_by_its_panel_not_by_the_button_that_opened_it() {
    let stories = measure_entry(&entry("Dialog"));
    let open = stories.iter().find(|s| s.name == "Open").expect("Open");
    assert_eq!(open.fit.width, ravenrs::widgets::book::Extent::Hug, "{:?}", open.fit);
    assert_eq!(open.fit.sizes[1].1.0, 460.0, "the width the story gives it");
}
