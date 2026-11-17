//! Optional origins belong to the declared test id, including when containers pass it on.
#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use ravenrs_e2e::{E2e, ErrorCode, Query};
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, VStack, ViewExt};

#[test]
fn an_untagged_target_has_no_source_and_query_errors_keep_their_codes() {
    let e2e = E2e::new(VStack::new((Button::new("Save"), Button::new("Save"))), Theme::raven_light());
    assert_eq!(e2e.source(&Query::text("Save").nth(0)).unwrap_err().code(), ErrorCode::NoSource);
    assert_eq!(e2e.source(&Query::text("Save")).unwrap_err().code(), ErrorCode::Ambiguous);
    assert_eq!(e2e.source(&Query::test_id("missing")).unwrap_err().code(), ErrorCode::NotFound);
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
    use ravenrs_widgets::View;

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
        let outer = VStack::new((inner, Button::new("No id"))).test_id("outer");
        let e2e = E2e::new(outer, Theme::raven_light());
        assert_eq!(e2e.source(&Query::test_id("inner")).unwrap().line(), line);
        assert_eq!(e2e.source(&Query::text("No id")).unwrap_err().code(), ErrorCode::NoSource);
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
