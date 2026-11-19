//! The stories the components give through `ComponentBook`.

use raven_component_book::index;
use ravenrs::widgets::book::all;

#[test]
fn every_component_that_gives_stories_has_at_least_one_and_names_only_its_own_settings() {
    let metas = all();
    assert!(metas.len() >= 170, "the components give their stories");
    for meta in &metas {
        assert!(!meta.stories.is_empty(), "{} has no story", meta.title);
        assert!(meta.title.contains('/'), "{} has no family", meta.title);
        for story in &meta.stories {
            for (name, _) in story.args.iter() {
                assert!(
                    meta.arg_types.iter().any(|a| a.name == name),
                    "{} / {} sets {name}, which is not one of its settings",
                    meta.title,
                    story.name
                );
            }
        }
    }
}

#[test]
fn the_book_lists_every_component_of_the_gallery_once() {
    let entries = index::entries();
    for name in component_gallery::samples::NAMES {
        assert_eq!(entries.iter().filter(|e| e.name() == *name).count(), 1, "{name}");
    }
    let button = entries.iter().find(|e| e.name() == "Button").expect("Button");
    assert!(!button.pending, "Button gives its own stories");
    assert!(button.methods().contains(&"style"));
    assert!(button.variants().iter().any(|(e, v)| *e == "ButtonStyle" && v.contains(&"Ghost")));
}

#[test]
fn the_self_test_passes() {
    let said = raven_component_book::self_test().expect("the self-test");
    assert!(said.starts_with("Autoteste:"), "{said}");
}
