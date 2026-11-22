//! Optional origins of explicit test ids and instrumented view constructors.
#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use ravenrs_e2e::{E2e, ErrorCode, Query};
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, TextField, VStack, ViewExt};

#[test]
fn query_errors_keep_their_codes_and_uninstrumented_controls_have_no_source() {
    let e2e = E2e::new(VStack::new((Button::new("Save"), Button::new("Save"))), Theme::raven_light());
    assert_eq!(e2e.source(&Query::text("Save")).unwrap_err().code(), ErrorCode::Ambiguous);
    assert_eq!(e2e.source(&Query::test_id("missing")).unwrap_err().code(), ErrorCode::NotFound);
    let field = TextField::new(ravenrs_reactive::State::new(String::new()).binding());
    let e2e = E2e::new(field, Theme::raven_light());
    assert_eq!(
        e2e.source(&Query::role(ravenrs_core::Role::TextInput)).unwrap_err().code(),
        ErrorCode::NoSource
    );
}

#[test]
fn a_tagged_target_has_its_origin_or_an_explicit_unavailable_error() {
    let line = line!() + 1;
    let e2e = E2e::new(Button::new("Save").test_id("save"), Theme::raven_light());
    // Cargo can enable the base's feature through another facade in the same build.
    match e2e.source(&Query::test_id("save")) {
        Ok(source) => {
            assert_eq!(source.file(), file!());
            assert_eq!(source.line(), line);
        }
        Err(error) => assert_eq!(error.code(), ErrorCode::NoSource),
    }
}

#[cfg(feature = "e2e-source")]
mod enabled {
    use super::*;
    use ravenrs_widgets::{Text, View};

    fn untagged() -> (Button, u32) {
        let line = line!() + 1;
        let button = Button::new("Save");
        (button, line)
    }

    #[test]
    fn an_untagged_button_keeps_its_constructor_when_reordered_and_found_by_role_or_text() {
        let (button, line) = untagged();
        let first = E2e::new(VStack::new((button, Button::new("Save"))), Theme::raven_light());
        let source = first.source(&Query::text("Save").nth(0)).unwrap();
        assert_eq!(source.file(), file!());
        assert_eq!(source.line(), line);
        assert!(source.column() > 0);
        let (button, _) = untagged();
        let next = E2e::new(VStack::new((Button::new("Other"), button)), Theme::raven_light());
        assert_eq!(next.source(&Query::role(ravenrs_core::Role::Button).name("Save")).unwrap(), source);
        assert!(!next.snapshot().contains(file!()));
        assert!(next.tree().nodes.iter().all(|node| node.test_id.is_none()));
    }

    #[test]
    fn every_text_constructor_records_the_apps_call_site_across_rebuilds() {
        use ravenrs_reactive::State;
        let state = State::new(0_u32);
        let dynamic = state.clone();
        let static_line = line!() + 1;
        let plain = Text::new("Static");
        let bound_line = line!() + 1;
        let bound = Text::bound(&state, |n| format!("Bound {n}"));
        let dynamic_line = line!() + 1;
        let computed = Text::dynamic(move || format!("Dynamic {}", dynamic.get()));
        let mut e2e = E2e::new(VStack::new((plain, bound, computed)), Theme::raven_light());
        for (text, line) in [("Static", static_line), ("Bound 0", bound_line), ("Dynamic 0", dynamic_line)] {
            let source = e2e.source(&Query::text(text)).unwrap();
            assert_eq!(source.file(), file!());
            assert_eq!(source.line(), line);
            assert!(source.column() > 0);
        }
        state.set(1);
        e2e.advance(0.0);
        assert_eq!(e2e.source(&Query::text("Bound 1")).unwrap().line(), bound_line);
        assert_eq!(e2e.source(&Query::text("Dynamic 1")).unwrap().line(), dynamic_line);
        assert!(!e2e.snapshot().contains(file!()));
    }

    #[test]
    fn an_explicit_tag_overrides_the_constructor_origin() {
        let button = Button::new("Save");
        let line = line!() + 1;
        let tagged = button.test_id("save");
        let e2e = E2e::new(tagged, Theme::raven_light());
        assert_eq!(e2e.source(&Query::test_id("save")).unwrap().line(), line);
    }

    fn save() -> (impl View, u32) {
        let line = line!() + 1;
        let view = Button::new("Save").test_id("save");
        (view, line)
    }

    #[test]
    fn a_tag_points_to_its_declaration_even_when_its_position_changes() {
        let (button, line) = save();
        let before = E2e::new(VStack::new((button, Button::new("Other"))), Theme::raven_light());
        let first = before.source(&Query::test_id("save")).unwrap();
        assert_eq!(first.file(), file!());
        assert_eq!(first.line(), line);
        assert!(first.column() > 0);
        let (button, _) = save();
        let after = E2e::new(VStack::new((Button::new("New before it"), button)), Theme::raven_light());
        assert_eq!(after.source(&Query::test_id("save")).unwrap(), first);
        assert!(!after.snapshot().contains(file!()), "source metadata is not in the screen reader's tree");
    }

    #[test]
    fn a_container_passes_the_origin_with_its_id_and_an_inner_id_keeps_its_own() {
        let line = line!() + 1;
        let container = VStack::new((Button::new("Save"),)).test_id("container");
        let e2e = E2e::new(container, Theme::raven_light());
        assert_eq!(e2e.source(&Query::test_id("container")).unwrap().line(), line);

        let line = line!() + 1;
        let inner = Button::new("Own id").test_id("inner");
        let constructor_line = line!() + 1;
        let plain = Button::new("No id");
        let outer = VStack::new((inner, plain)).test_id("outer");
        let e2e = E2e::new(outer, Theme::raven_light());
        assert_eq!(e2e.source(&Query::test_id("inner")).unwrap().line(), line);
        assert_eq!(e2e.source(&Query::text("No id")).unwrap().line(), constructor_line);
    }

    struct Custom;
    impl View for Custom {
        fn build(&self, cx: &mut ravenrs_widgets::BuildCx<'_>) -> ravenrs_widgets::NodeId {
            let node = cx.child(&Button::new("Custom"));
            cx.test_id(node, "custom");
            node
        }
    }

    #[test]
    fn a_components_direct_tag_records_the_components_call_site() {
        let e2e = E2e::new(Custom, Theme::raven_light());
        let source = e2e.source(&Query::test_id("custom")).unwrap();
        assert_eq!(source.file(), file!());
        let line = include_str!("source.rs").lines().nth(source.line() as usize - 1).unwrap();
        assert!(line.contains("cx.test_id(node, \"custom\")"), "{source}: {line}");
    }
}
