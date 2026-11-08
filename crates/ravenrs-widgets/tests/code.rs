//! The code editor and the playground, driven through the public `Ui` API.
#![cfg(feature = "code")]
#![allow(clippy::expect_used)]

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui_at(view: impl View + 'static, width: f32) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(width, 700.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn ui(view: impl View + 'static) -> Ui {
    ui_at(view, 1200.0)
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
}

fn key(ui: &mut Ui, key: Key) {
    press(ui, key, Modifiers::default());
}

fn typed(ui: &mut Ui, text: &str) {
    for c in text.chars() {
        key(ui, if c == ' ' { Key::Space } else { Key::Character(c.to_string()) });
    }
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn focus_editor(ui: &mut Ui) {
    let spot = {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("a text input").bounds.center()
    };
    click(ui, spot);
}

fn shows(ui: &mut Ui, words: &str) -> bool {
    let tree = ui.access_tree();
    tree.nodes.iter().any(|n| {
        n.name.as_deref().is_some_and(|s| s.contains(words))
            || n.value.as_deref().is_some_and(|s| s.contains(words))
    })
}

/// A text shown on its own, not inside the code.
fn result(ui: &mut Ui, words: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(words))
}

fn colored(spans: &[(usize, usize, Semantic)], text: &str, piece: &str) -> Option<Semantic> {
    let at = text.find(piece).expect("the piece is in the text");
    let start = text[..at].chars().count();
    spans.iter().find(|(from, to, _)| *from <= start && start < *to).map(|(_, _, s)| *s)
}

#[test]
fn rust_is_colored_by_token() {
    let text = "#[derive(Debug)]\nfn main() {\n    // note\n    let n = 42;\n    println!(\"hi {n}\");\n    Some(n)\n}";
    let spans = highlight_rust(text);
    assert_eq!(colored(&spans, text, "derive"), Some(Semantic::Chart4), "attributes");
    assert_eq!(colored(&spans, text, "fn"), Some(Semantic::FocusRing), "keywords");
    assert_eq!(colored(&spans, text, "// note"), Some(Semantic::InkMuted), "comments");
    assert_eq!(colored(&spans, text, "42"), Some(Semantic::Chart3), "numbers");
    assert_eq!(colored(&spans, text, "println!"), Some(Semantic::Chart4), "macros");
    assert_eq!(colored(&spans, text, "\"hi"), Some(Semantic::Success), "strings");
    assert_eq!(colored(&spans, text, "Some"), Some(Semantic::Chart2), "types and variants");
    assert_eq!(colored(&spans, text, "main"), None, "other names stay plain");
    // Unfinished code still colors, to its end.
    assert!(!highlight_rust("let s = \"open").is_empty());
}

#[test]
fn tab_indents_and_enter_keeps_the_indentation() {
    let code = State::new(String::new());
    let mut ui = ui(CodeEditor::new(code.binding()).rows(8));
    focus_editor(&mut ui);
    typed(&mut ui, "fn a() {");
    key(&mut ui, Key::Enter);
    typed(&mut ui, "x");
    assert_eq!(code.get(), "fn a() {\n    x", "Enter after a brace indents one step");
    key(&mut ui, Key::Enter);
    typed(&mut ui, "y");
    assert_eq!(code.get(), "fn a() {\n    x\n    y", "and keeps it after that");
    press(&mut ui, Key::Tab, Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(code.get(), "fn a() {\n    x\ny", "Shift+Tab takes a step back");
    key(&mut ui, Key::Home);
    key(&mut ui, Key::Tab);
    assert_eq!(code.get(), "fn a() {\n    x\n    y", "Tab inserts spaces and keeps the focus here");
    press(&mut ui, Key::Enter, Modifiers { ctrl: true, ..Modifiers::default() });
    assert_eq!(code.get(), "fn a() {\n    x\n    y", "Ctrl+Enter is left to shortcuts");
}

#[test]
fn a_diagnostic_marks_its_line() {
    let code = State::new("let a = 1;\nlet b = oops;\n".to_owned());
    let danger = Theme::raven_dark().color(Semantic::Danger);
    let marked = |diagnostics: Vec<CodeDiagnostic>| {
        let ui = ui(CodeEditor::new(code.binding()).diagnostics(diagnostics));
        ui.scene()
            .primitives()
            .iter()
            .any(|p| matches!(p, Primitive::Quad(q) if q.fill == ravenrs_core::Brush::Solid(danger)))
    };
    assert!(!marked(Vec::new()));
    assert!(marked(vec![CodeDiagnostic::error(2, 9, 4, "cannot find value `oops`")]));
}

fn runner(source: &str) -> Result<AnyView, Vec<CodeDiagnostic>> {
    match source.trim().strip_prefix("say ") {
        Some(words) => Ok(Box::new(Text::new(words.to_owned()))),
        None => Err(vec![CodeDiagnostic::error(1, 1, 3, "Start with say").hint("Write `say hello`")]),
    }
}

#[test]
fn the_playground_runs_shows_problems_and_keeps_the_last_result() {
    let source = State::new(String::new());
    let mut ui = ui(Playground::new(runner)
        .example("Hello", "say hello")
        .example("Bye", "say bye")
        .source(source.binding()));
    assert_eq!(source.get(), "say hello", "an empty code starts from the first example");
    assert!(result(&mut ui, "hello"), "it runs it as it opens");

    source.set("yell hello".to_owned());
    ui.update();
    press(&mut ui, Key::Enter, Modifiers { ctrl: true, ..Modifiers::default() });
    assert!(shows(&mut ui, "Line 1, column 1"), "the problem says where it is");
    assert!(shows(&mut ui, "Write `say hello`"), "and how to fix it");
    assert!(shows(&mut ui, "Last version that worked"));
    assert!(result(&mut ui, "hello"), "the last result stays");

    source.set("say again".to_owned());
    ui.update();
    press(&mut ui, Key::Enter, Modifiers { meta: true, ..Modifiers::default() });
    assert!(result(&mut ui, "again"));
    assert!(!shows(&mut ui, "Last version that worked"), "nothing to excuse once it works");
}

#[test]
fn the_playground_restores_the_example() {
    let source = State::new(String::new());
    let mut ui = ui(Playground::new(runner).example("Hello", "say hello").source(source.binding()));
    source.set("say changed".to_owned());
    ui.update();
    let spot = {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Restore"))
            .expect("a restore button")
            .bounds
            .center()
    };
    click(&mut ui, spot);
    assert_eq!(source.get(), "say hello");
}

#[test]
fn a_narrow_playground_shows_code_and_result_as_tabs() {
    let mut ui = ui_at(Playground::new(runner).example("Hello", "say hello"), 500.0);
    assert!(shows(&mut ui, "Code"));
    assert!(result(&mut ui, "hello"), "it opens on the result after running");
}

#[test]
fn an_action_gets_the_code() {
    let got = State::new(String::new());
    let keep = got.clone();
    let mut ui = ui(Playground::new(runner).example("Hello", "say hello").action(
        "Copy link",
        IconName::Link,
        move |code| keep.set(code.to_owned()),
    ));
    let spot = {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Copy link"))
            .expect("the action")
            .bounds
            .center()
    };
    click(&mut ui, spot);
    assert_eq!(got.get(), "say hello");
}
