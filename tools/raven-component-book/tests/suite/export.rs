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
    // … and three per pattern.
    let patterns = raven_component_book::patterns::entries();
    assert_eq!(written.len(), 8 + (index::entries().len() + patterns.len()) * 3);
    assert_eq!(patterns.len(), 5);
    for p in &patterns {
        let folder = project.join("patterns").join(p.name());
        let preview = std::fs::read_to_string(folder.join("preview.html")).expect("pattern preview");
        assert!(preview.starts_with("<!-- @dsCard group=\"Patterns\""), "{}", p.name());
        assert!(preview.contains("<svg"), "{} draws", p.name());
        assert!(readme.contains(&format!("| `{}` |", p.name())), "the guide lists {}", p.name());
    }
    // The card says how a component sizes, and the preview shows it beside the narrowest parent.
    let field = project.join("components/TextField");
    let card = std::fs::read_to_string(field.join("README.md")).expect("card");
    assert!(card.contains("## Sizing\n\nFills the width of its parent"), "{card}");
    let preview = std::fs::read_to_string(field.join("preview.html")).expect("preview");
    assert!(preview.contains("320 pt parent"), "the reference beside the narrowest parent");
    assert!(preview.contains("<svg data-rcb=\"TextField/Empty\" "), "each picture says what it draws");
    assert!(readme.contains("data-rcb"), "the guide asks the design to keep the marks");
    assert!(
        readme.contains("## Checking a screen") && readme.contains("querySelectorAll('[data-rcb]')"),
        "the guide gives the script that writes a screen"
    );
    // Every icon has its character in RavenIcons on the card of Icon.
    let icons = std::fs::read_to_string(project.join("components/Icon/README.md")).expect("Icon");
    assert!(icons.contains("| `wallet` | `Wallet` | `&#xe204;` |"), "{icons}");
    // The card says what a person does with a component.
    let select = std::fs::read_to_string(project.join("components/Select/README.md")).expect("Select");
    assert!(select.contains("## Behavior") && select.contains("`ArrowDown`"), "{select}");
    // The interface follows the theme, glass included: no fixed color in the previews of the
    // pieces a screen is made of. Data keeps its own colors (pixels, swatches, channels).
    for name in [
        "Button",
        "TextField",
        "Select",
        "Checkbox",
        "Toggle",
        "Card",
        "Dialog",
        "ConfirmDialog",
        "Sheet",
        "Tabs",
        "Sidebar",
        "FormField",
    ] {
        let preview =
            std::fs::read_to_string(project.join("components").join(name).join("preview.html")).expect(name);
        let fixed: Vec<&str> = preview
            .match_indices(":#")
            .map(|(i, _)| &preview[i.saturating_sub(6)..(i + 11).min(preview.len())])
            .filter(|around| {
                around.contains("fill:#") || around.contains("stroke:#") || around.contains("color:#")
            })
            .collect();
        assert!(fixed.len() <= 1, "{name} has fixed colors: {fixed:?}");
    }
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

#[test]
fn a_button_says_what_changes_under_the_pointer_the_focus_and_a_press() {
    let entry = entry("Button");
    let json = component_json(&entry, &measure_entry(&entry));
    let interaction = &json["interaction"];
    assert_eq!(interaction["reacts_to"], serde_json::json!(["Hover", "Focus", "Pressed"]), "{interaction}");
    let hover = interaction["Hover"]["changes"].to_string();
    assert!(hover.contains("accent-hover"), "{hover}");
    let focus = interaction["Focus"]["changes"].to_string();
    assert!(focus.contains("focus-ring"), "the ring has the focus token: {focus}");
    assert_eq!(interaction["Hover"]["control"], "Save");
}

#[test]
fn a_divider_has_no_control_and_no_states() {
    let entry = entry("Divider");
    let json = component_json(&entry, &measure_entry(&entry));
    assert!(json["interaction"].is_null(), "{}", json["interaction"]);
}

#[test]
fn every_story_is_measured_in_glass_too() {
    let entry = entry("Card");
    let json = component_json(&entry, &measure_entry(&entry));
    for story in json["stories"].as_array().expect("stories") {
        for theme in ["dark", "light", "glass"] {
            assert!(story["themes"][theme]["paint"].as_array().is_some_and(|p| !p.is_empty()), "{theme}");
        }
    }
}

#[test]
fn a_color_between_two_tokens_is_named_by_them() {
    use raven_component_book::measure::token_of;
    use ravenrs::prelude::*;
    let theme = Theme::raven_dark();
    let (ink, border) = (theme.color(Semantic::Ink), theme.color(Semantic::Border));
    let quarter = Color::rgba(
        border.r + (ink.r - border.r) * 0.25,
        border.g + (ink.g - border.g) * 0.25,
        border.b + (ink.b - border.b) * 0.25,
        1.0,
    );
    let name = token_of(quarter, &theme);
    assert!(name.starts_with("mix("), "{name}");
    assert!(name.ends_with(", 25)") || name.ends_with(", 75)"), "a quarter of one over the other: {name}");
}

#[test]
fn the_card_says_what_a_person_does_with_a_component() {
    let select = entry("Select");
    let json = component_json(&select, &measure_entry(&select));
    let behavior = &json["behavior"];
    assert_eq!(behavior["role"], "ComboBox", "{behavior}");
    let keys = behavior["keys"].to_string();
    for key in ["ArrowDown", "Enter", "Escape"] {
        assert!(keys.contains(key), "{key} opens or moves the list: {keys}");
    }
    assert_eq!(behavior["focus_order"][0]["role"], "ComboBox", "{behavior}");
    let button = entry("Button");
    let json = component_json(&button, &measure_entry(&button));
    let behavior = &json["behavior"];
    assert!(behavior["events"].to_string().contains("on_click"), "{behavior}");
    // Its click calls `on_click`, which the story does not show; Escape only takes the focus.
    assert!(!behavior["keys"].to_string().contains("Escape"), "{behavior}");
    let divider = entry("Divider");
    let json = component_json(&divider, &measure_entry(&divider));
    assert_eq!(json["behavior"]["keys"], serde_json::json!([]), "{}", json["behavior"]);
}

#[test]
fn an_icon_measures_as_large_as_it_draws() {
    let stories = measure_entry(&entry("Icon"));
    let (w, h) = stories[0].themes[0].1.size;
    // Three icons, the first 24 pt, side by side 12 apart: a row, not a point.
    assert!(w >= 90.0 && h >= 20.0, "{w} × {h}");
}

#[test]
fn the_sidebar_shows_its_compact_items_and_sections() {
    let stories = measure_entry(&entry("Sidebar"));
    let compact = stories.iter().find(|s| s.name == "Compact, with sections").expect("story");
    let parts = &compact.themes[0].1.parts;
    let item = parts.iter().find(|p| p["name"] == "Transactions").expect("an item");
    assert_eq!(item["bounds"][3], 32.0, "a compact item is 32 pt tall: {item}");
}

#[test]
fn the_pointer_goes_to_a_control_at_rest() {
    // The selected tab looks the same under the pointer; another tab shows how tabs react.
    let tabs = entry("Tabs");
    let json = component_json(&tabs, &measure_entry(&tabs));
    let interaction = &json["interaction"];
    assert!(interaction["reacts_to"].to_string().contains("Hover"), "{interaction}");
    assert_ne!(interaction["Hover"]["control"], interaction["Focus"]["control"], "{interaction}");
}

#[test]
fn the_sidebar_page_folds_its_sidebar_in_a_narrow_window() {
    let page = raven_component_book::patterns::entries()
        .into_iter()
        .find(|p| p.name() == "SidebarPage")
        .expect("pattern");
    let stories = measure_entry(&page);
    let navigation_at_320 = stories[0]
        .narrow
        .parts
        .iter()
        .any(|p| p["role"] == "Navigation" && p["bounds"][2].as_f64().unwrap_or(0.0) > 100.0);
    assert!(!navigation_at_320, "at 320 pt the sidebar opens as a sheet and takes no room");
    let navigation_at_760 = stories[0].themes[0].1.parts.iter().any(|p| p["role"] == "Navigation");
    assert!(navigation_at_760, "at 760 pt it sits beside the page");
}

const FASE0: &str = include_str!("../fixtures/fase0-novo-lancamento.rcb.json");

#[test]
fn the_screen_of_phase_zero_is_built_as_it_was_drawn() {
    // The boxes are those RavenRS laid the "Novo lançamento" dialog of Raven Finanças out in.
    let replay = raven_component_book::replay::replay(FASE0).expect("a screen");
    assert_eq!(replay.compared, 4);
    assert!(replay.divergences.is_empty(), "{:#?}", replay.divergences);
}

#[test]
fn a_piece_drawn_another_size_is_pointed_out() {
    // The Save button drawn 48 pt tall, as no size of Button is with these settings.
    let altered = FASE0.replace("\"box\": [221, 456, 195, 40]", "\"box\": [221, 456, 195, 48]");
    let replay = raven_component_book::replay::replay(&altered).expect("a screen");
    assert_eq!(replay.divergences.len(), 1, "{:#?}", replay.divergences);
    let said = replay.divergences[0].to_string();
    assert!(
        said.contains("Button/Primary") && said.contains("height 48 in the design, 40 in RavenRS"),
        "{said}"
    );
}

#[test]
fn a_piece_the_library_does_not_have_is_pointed_out() {
    let invented = r#"{"elements": [
        {"rcb": "Chip/Example", "box": [0, 0, 60, 24]},
        {"rcb": "Button/Huge", "box": [0, 0, 60, 24]},
        {"rcb": "Button/Primary", "args": {"glow": true}, "box": [0, 0, 72, 40]},
        {"rcb": "Button/Primary", "args": {"style": "Neon"}, "box": [0, 0, 72, 40]}
    ]}"#;
    let replay = raven_component_book::replay::replay(invented).expect("a screen");
    let said: Vec<String> = replay.divergences.iter().map(ToString::to_string).collect();
    assert_eq!(said.len(), 4, "{said:#?}");
    assert!(said[0].contains("no component \"Chip\""), "{said:#?}");
    assert!(said[1].contains("no story \"Huge\""), "{said:#?}");
    assert!(said[2].contains("no setting \"glow\""), "{said:#?}");
    assert!(said[3].contains("no option \"Neon\""), "{said:#?}");
}

#[test]
fn the_replay_exits_with_one_when_a_piece_diverges() {
    let dir = std::env::temp_dir().join(format!("rcb-replay-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("folder");
    let (good, bad) = (dir.join("good.rcb.json"), dir.join("bad.rcb.json"));
    std::fs::write(&good, FASE0).expect("write");
    std::fs::write(&bad, FASE0.replace("[110, 456, 101, 40]", "[110, 456, 101, 48]")).expect("write");
    let run = |file: &std::path::Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_raven-component-book"))
            .arg("--replay")
            .arg(file)
            .output()
            .expect("runs")
    };
    assert_eq!(run(&good).status.code(), Some(0), "{}", String::from_utf8_lossy(&run(&good).stdout));
    let failed = run(&bad);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stdout).contains("Button/Secondary"));
    let _ = std::fs::remove_dir_all(&dir);
}
