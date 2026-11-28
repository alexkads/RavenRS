//! RavenComponentBook guards the design system: the tokens it writes are the ones RavenRS draws
//! with, the words about them cover every token, and what it writes is the whole system.

use raven_component_book::{export, tokens};
use serde_json::Value;

/// A value as the guide and the code may write it alike: `0` and `0px`, `0.30` and `0.3`.
fn plain(v: &Value) -> String {
    let text = match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    text.to_lowercase().replace("0px", "0").replace(".30)", ".3)").replace(".40)", ".4)").replace(' ', "")
}

#[test]
fn the_tokens_in_the_docs_are_the_ones_ravenrs_style_draws_with() {
    let docs: Value =
        serde_json::from_str(include_str!("../../../../docs/design-system/tokens.json")).unwrap();
    let code = tokens::tokens("test");
    let mut drift = Vec::new();
    for family in ["color", "spacing", "radius", "shadow", "duration", "spring"] {
        for t in code[family]["tokens"].as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            let Some(doc) =
                docs[family]["tokens"].as_array().and_then(|d| d.iter().find(|d| d["name"] == name))
            else {
                drift.push(format!(
                    "{family}/{name}: in ravenrs-style, not in docs/design-system/tokens.json"
                ));
                continue;
            };
            let (a, b) = (&t["value"], &doc["value"]);
            let same = match (a, b) {
                // The themes of the applications are theirs, not ravenrs-style's: the docs speak
                // only of the three of RavenRS.
                (Value::Object(x), Value::Object(y)) => x
                    .iter()
                    .filter(|(k, _)| !tokens::APP_THEMES.iter().any(|(id, _)| id == k))
                    .all(|(k, v)| y.get(k).is_some_and(|w| plain(v) == plain(w))),
                _ => plain(a) == plain(b),
            };
            if !same {
                drift.push(format!("{family}/{name}: ravenrs-style {a}, docs {b}"));
            }
        }
    }
    assert!(drift.is_empty(), "the docs disagree with ravenrs-style:\n{}", drift.join("\n"));
}

#[test]
fn every_token_says_what_it_is_for() {
    let missing = tokens::without_usage();
    assert!(missing.is_empty(), "tokens with no usage in docs/design-system/tokens.json: {missing:?}");
}

#[test]
fn the_written_system_is_whole_and_locked_to_the_code() {
    let dir = std::env::temp_dir().join(format!("raven-component-book-guardian-{}", std::process::id()));
    let written = export::write_claude_design(&dir, "test").unwrap();
    for needed in [
        "project/design-system.json",
        "project/tokens.json",
        "project/tokens.css",
        "project/fonts/Inter-Regular.ttf",
        "project/fonts/Inter-SemiBold.ttf",
        "project/fonts/RavenIcons.ttf",
        "project/components/Cover/preview.html",
        "project/README.md",
    ] {
        assert!(written.iter().any(|w| w == needed), "{needed} is not written");
    }
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("project/design-system.json")).unwrap())
            .unwrap();
    assert_eq!(index["editing"], "code", "the page must not change what the book writes");
    assert_eq!(index["createdOnFiles"]["at"], "2026-10-31T22:28:36Z");
    // The cover is bare: no README beside it, so the design tool keeps it as the cover.
    assert!(!dir.join("project/components/Cover/README.md").exists());
    // A copy of what was written compares clean; one changed file and one stray file do not.
    // Compared with the copy just written, not written again: measuring every component is what
    // takes the time, and `verify` is writing and then comparing (cargo xtask pr runs it).
    let copy = dir.with_extension("copy");
    copy_folder(&dir, &copy);
    assert!(export::compare(&copy, &dir, &written).unwrap().is_clean());
    std::fs::write(copy.join("project/components/Button/README.md"), "changed by hand").unwrap();
    std::fs::write(copy.join("project/stray.md"), "not the book's").unwrap();
    let drift = export::compare(&copy, &dir, &written).unwrap();
    assert_eq!(drift.changed, vec!["project/components/Button/README.md".to_owned()]);
    assert_eq!(drift.extra, vec!["project/stray.md".to_owned()]);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&copy).ok();
}

fn copy_folder(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for item in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(item.file_name());
        if item.path().is_dir() {
            copy_folder(&item.path(), &target);
        } else {
            std::fs::copy(item.path(), target).unwrap();
        }
    }
}
