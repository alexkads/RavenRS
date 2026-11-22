//! The screen as text.

use ravenrs_core::Role;
use ravenrs_e2e::{E2e, Query};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, TextField, VStack, ViewExt};

#[test]
fn the_snapshot_lists_every_part_with_a_reference_that_finds_it() {
    let name = State::new("Ana".to_owned());
    let e2e = E2e::new(
        VStack::new((
            TextField::new(name.binding()).placeholder("Name").test_id("name"),
            Button::new("Save").test_id("save"),
        )),
        Theme::raven_dark(),
    );
    let text = e2e.snapshot();
    assert_eq!(text, e2e.snapshot(), "the same screen reads the same");
    let save = text.lines().find(|l| l.contains("button \"Save\"")).expect("the button is listed");
    assert!(save.contains("#save"), "{save}");
    let field = text.lines().find(|l| l.contains("text_input")).expect("the field is listed");
    assert!(field.contains("value=\"Ana\"") && field.contains("#name"), "{field}");
    let reference = save.split("[ref=").nth(1).and_then(|r| r.split(']').next()).expect("a reference");
    let found = e2e.find(&Query::by_ref(reference)).unwrap();
    assert_eq!(found.role, Role::Button);
    assert_eq!(found.name.as_deref(), Some("Save"));
}
