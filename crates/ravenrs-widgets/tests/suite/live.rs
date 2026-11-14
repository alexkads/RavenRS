//! Examples run by the interpreter, through `LiveRunner`.
#![cfg(feature = "live")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ravenrs_core::{Event, Point, PointerButton, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn shown(view: AnyView) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn says(ui: &mut Ui, words: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(words))
}

#[test]
fn an_example_runs_and_its_closures_change_its_state() {
    let source = r#"
        use ravenrs::prelude::*;

        fn counter() -> impl View {
            let clicks = State::new(0);
            let add = clicks.clone();
            VStack::new((
                Button::new("Add").style(ButtonStyle::Secondary).on_click(move || add.update(|n| *n += 1)),
                Text::bound(&clicks, |n| format!("{n} clicks")).foreground(Semantic::InkMuted),
            ))
            .spacing(8.0)
            .padding(12.0)
        }
    "#;
    let view = LiveRunner::new().run(source).unwrap_or_else(|e| panic!("{e:?}"));
    let mut ui = shown(view);
    assert!(says(&mut ui, "0 clicks"));
    let spot: Point = {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.name.as_deref() == Some("Add")).expect("the button").bounds.center()
    };
    for _ in 0..2 {
        ui.handle_event(&Event::PointerMove { position: spot });
        ui.handle_event(&Event::PointerDown { position: spot, button: PointerButton::Primary });
        ui.handle_event(&Event::PointerUp { position: spot, button: PointerButton::Primary });
        ui.update();
    }
    assert!(says(&mut ui, "2 clicks"), "the closure ran in the interpreter");
}

#[test]
fn what_it_does_not_know_comes_back_with_its_place() {
    let problems =
        LiveRunner::new().run("fn f() -> impl View {\n    Gizmo::new()\n}").err().expect("refused");
    assert_eq!(problems[0].line, 2);
    assert!(problems[0].message.contains("Gizmo"), "{}", problems[0].message);
    let problems = LiveRunner::new()
        .run("fn f() -> impl View {\n    Button::new(\"a\").wobble()\n}")
        .err()
        .expect("refused");
    assert!(problems[0].message.contains("wobble"), "{}", problems[0].message);
}

#[test]
fn a_struct_with_hundreds_of_fields_is_written_from_another() {
    // `Labels` is built field by field, so an optimized build of it stays quick.
    let source = r#"
        fn f() -> impl View {
            let labels = Labels { close: "Fechar".to_owned(), ..Labels::default() };
            Text::new(labels.close)
        }
    "#;
    let view = LiveRunner::new().run(source).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(says(&mut shown(view), "Fechar"));

    let problems = LiveRunner::new()
        .run("fn f() -> impl View {\n    let labels = Labels { close: \"x\".to_owned() };\n    Text::new(labels.close)\n}")
        .err()
        .expect("refused");
    assert!(problems[0].message.contains("is missing"), "{}", problems[0].message);
    let problems = LiveRunner::new()
        .run("fn f() -> impl View {\n    let labels = Labels { shut: \"x\".to_owned(), ..Labels::default() };\n    Text::new(labels.close)\n}")
        .err()
        .expect("refused");
    assert!(problems[0].message.contains("no field `shut`"), "{}", problems[0].message);
}

#[test]
fn a_println_in_a_build_does_not_ask_for_a_build_every_frame() {
    // #167: the console used to build the playground again for each line, and the code printed
    // one more line on each build.
    let code = r#"fn ticking() -> impl View {
    Text::dynamic(move || {
        println!("built");
        "Ticking".to_string()
    })
}"#;
    let playground =
        Playground::new(LiveRunner::new()).example("Ticking", code).output_open(true).height(600.0);
    let mut ui = Ui::new(playground, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let settled = ui.stats().builds;
    for _ in 0..10 {
        ui.update();
    }
    assert_eq!(ui.stats().builds, settled, "nothing new to show, so nothing is built");
    // What it printed reached the console, said once that it comes on every build, and the
    // console stopped growing.
    let lines =
        ui.access_tree().nodes.iter().find_map(|n| n.name.clone().filter(|n| n.starts_with("Console ")));
    for _ in 0..10 {
        ui.update();
    }
    let later =
        ui.access_tree().nodes.iter().find_map(|n| n.name.clone().filter(|n| n.starts_with("Console ")));
    assert!(lines.as_deref().is_some_and(|n| n != "Console 0"), "{lines:?}");
    assert_eq!(lines, later, "the console does not grow by itself");
}
