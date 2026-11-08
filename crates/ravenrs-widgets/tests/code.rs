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

/// What was saved: names and bytes.
type Saved = std::rc::Rc<std::cell::RefCell<Vec<(String, Vec<u8>)>>>;

/// Keeps what the playground hands over to save.
struct Saver(Saved);

impl FilePickerProvider for Saver {
    fn pick(&self, _request: &FileRequest, done: Box<dyn FnOnce(Vec<PickedFile>)>) {
        done(Vec::new());
    }

    fn save(&self, name: &str, bytes: &[u8]) {
        self.0.borrow_mut().push((name.to_owned(), bytes.to_vec()));
    }
}

#[test]
fn download_saves_what_the_code_makes_or_marks_why_not() {
    let source = State::new(String::new());
    let make = |code: &str| match code.strip_prefix("say ") {
        Some(words) => Ok((format!("{words}.zip"), words.as_bytes().to_vec())),
        None => Err(vec![CodeDiagnostic::error(1, 1, 3, "Nothing to save")]),
    };
    let mut ui =
        ui(Playground::new(runner).example("Hello", "say hello").source(source.binding()).download(make));
    let saved: Saved = std::rc::Rc::default();
    ui.set_file_picker(Saver(std::rc::Rc::clone(&saved)));
    ui.update();
    let spot = |ui: &mut Ui| {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Download project"))
            .expect("the button")
            .bounds
            .center()
    };
    let at = spot(&mut ui);
    click(&mut ui, at);
    assert_eq!(*saved.borrow(), [("hello.zip".to_owned(), b"hello".to_vec())]);

    source.set("yell".to_owned());
    ui.update();
    let at = spot(&mut ui);
    click(&mut ui, at);
    assert_eq!(saved.borrow().len(), 1, "nothing more is saved");
    assert!(shows(&mut ui, "Nothing to save"), "and the code says why");
}

/// Where a runner keeps the playground's wake.
type Wake = std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<dyn Fn()>>>>;

/// A runner that prints what it says, and can be told later that something failed.
#[derive(Clone, Default)]
struct Talking {
    heard: std::rc::Rc<std::cell::RefCell<RunOutput>>,
    wake: Wake,
}

impl Talking {
    fn later(&self, line: Option<&str>, problem: Option<CodeDiagnostic>) {
        let mut heard = self.heard.borrow_mut();
        heard.lines.extend(line.map(str::to_owned));
        heard.problems.extend(problem);
        drop(heard);
        let wake = self.wake.borrow().clone();
        if let Some(wake) = wake {
            wake();
        }
    }
}

impl Runner for Talking {
    fn run(&self, source: &str) -> Result<AnyView, Vec<CodeDiagnostic>> {
        let view = runner(source)?;
        self.heard.borrow_mut().lines.push(format!("ran {}", source.trim()));
        Ok(view)
    }

    fn take_output(&self) -> RunOutput {
        std::mem::take(&mut *self.heard.borrow_mut())
    }

    fn watch(&self, wake: std::rc::Rc<dyn Fn()>) {
        *self.wake.borrow_mut() = Some(wake);
    }
}

fn press_named(ui: &mut Ui, name: &str) {
    let spot = {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name}"))
            .bounds
            .center()
    };
    click(ui, spot);
}

#[test]
fn the_output_shows_what_the_code_printed_and_what_failed_later() {
    let talking = Talking::default();
    let mut ui = ui(Playground::new(talking.clone()).example("Hello", "say hello"));
    assert!(shows(&mut ui, "ran say hello"), "what the run printed");
    assert!(shows(&mut ui, "Console 1"));
    assert!(shows(&mut ui, "No problems"));

    talking.later(Some("clicked"), None);
    ui.update();
    assert!(shows(&mut ui, "clicked"), "a line printed later shows as it comes");
    assert!(shows(&mut ui, "Console 2"));

    talking.later(None, Some(CodeDiagnostic::error(3, 5, 2, "a closure failed")));
    ui.update();
    assert!(shows(&mut ui, "Problems: 1"), "what failed later counts as a problem");
    press_named(&mut ui, "Clear the output");
    assert!(shows(&mut ui, "Console 0") && shows(&mut ui, "No problems"));

    press_named(&mut ui, "Hide the output");
    assert!(shows(&mut ui, "Show the output"), "the output folds away");
}

#[test]
fn a_problem_opens_the_output_on_its_list() {
    let source = State::new(String::new());
    let mut ui = ui(Playground::new(runner).example("Hello", "say hello").source(source.binding()));
    press_named(&mut ui, "Hide the output");
    source.set("yell".to_owned());
    ui.update();
    press(&mut ui, Key::Enter, Modifiers { ctrl: true, ..Modifiers::default() });
    assert!(shows(&mut ui, "Hide the output"), "it opens again");
    assert!(shows(&mut ui, "Line 1, column 1"), "on the problems");
}

#[test]
fn picking_an_example_runs_it() {
    let mut ui = ui(Playground::new(runner).example("Hello", "say hello").example("Bye", "say bye"));
    assert!(result(&mut ui, "hello"));
    press_named(&mut ui, "Example");
    press_named(&mut ui, "Bye");
    assert!(result(&mut ui, "bye"), "with no Run pressed");
}

#[test]
fn a_narrow_playground_has_three_tabs_a_menu_and_run_at_the_bottom() {
    let mut ui = ui_at(
        Playground::new(runner)
            .example("Hello", "say hello")
            .action("Copy link", IconName::Link, |_| {})
            .height(700.0),
        390.0,
    );
    assert!(shows(&mut ui, "Code") && shows(&mut ui, "Result") && shows(&mut ui, "Output"));
    assert!(shows(&mut ui, "More actions"), "the other actions go in a menu");
    assert!(!shows(&mut ui, "Copy link"), "and not in the bar");
    let tree = ui.access_tree();
    let run = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Run")).expect("Run");
    assert!(run.bounds.y() > 600.0, "Run sits at the bottom: {:?}", run.bounds);
}

#[test]
fn the_output_can_start_folded_and_a_problem_opens_it() {
    let source = State::new(String::new());
    let mut ui =
        ui(Playground::new(runner).example("Hello", "say hello").source(source.binding()).output_open(false));
    assert!(shows(&mut ui, "Show the output"), "it starts folded");
    source.set("yell".to_owned());
    ui.update();
    press(&mut ui, Key::Enter, Modifiers { ctrl: true, ..Modifiers::default() });
    assert!(shows(&mut ui, "Hide the output"), "a problem opens it");
}

#[test]
fn the_code_gets_the_wider_side() {
    let mut ui = ui(Playground::new(runner).example("Hello", "say hello"));
    let tree = ui.access_tree();
    let code = tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("the editor").bounds;
    assert!(code.width() > 1200.0 * 0.55, "the code takes most of the width: {code:?}");
}

/// Where each run of glyphs starts, with how many glyphs it has.
fn runs(ui: &Ui) -> Vec<(f32, usize)> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Glyphs(g) => g.glyphs.first().map(|first| (first.x, g.glyphs.len())),
            _ => None,
        })
        .collect()
}

/// Where the longest run starts: the long line of code.
fn long_line_x(ui: &Ui) -> f32 {
    runs(ui).into_iter().max_by_key(|(_, count)| *count).expect("the long line").0
}

/// Where the line numbers start: the runs of one glyph.
fn numbers_x(ui: &Ui) -> Vec<f32> {
    runs(ui).into_iter().filter(|(_, count)| *count == 1).map(|(x, _)| x).collect()
}

fn long_code() -> State<String> {
    State::new(format!("fn a() {{}}\nlet x = {};", "1234567890".repeat(20)))
}

#[test]
fn a_long_line_scrolls_sideways_and_the_numbers_stay() {
    let code = long_code();
    let mut ui = ui_at(CodeEditor::new(code.binding()).language(CodeLanguage::Plain).rows(4), 400.0);
    let (line, numbers) = (long_line_x(&ui), numbers_x(&ui));
    assert_eq!(numbers.len(), 2, "two line numbers: {numbers:?}");
    let spot = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput)
        .expect("the editor")
        .bounds
        .center();
    ui.handle_event(&Event::Scroll { position: spot, delta: Point::new(-120.0, 0.0) });
    ui.update();
    assert!(
        (line - long_line_x(&ui) - 120.0).abs() < 0.5,
        "the code moved sideways: {line} to {}",
        long_line_x(&ui)
    );
    assert_eq!(numbers_x(&ui), numbers, "the line numbers stay put");
}

#[test]
fn the_caret_brings_its_column_into_view() {
    let code = State::new(format!("let x = {};", "1234567890".repeat(20)));
    let mut ui = ui_at(CodeEditor::new(code.binding()).language(CodeLanguage::Plain).rows(4), 400.0);
    let start = long_line_x(&ui);
    focus_editor(&mut ui);
    key(&mut ui, Key::End);
    ui.update();
    let width = 208.0 * 7.0;
    assert!(long_line_x(&ui) < start - width * 0.5, "End shows the end: {start} to {}", long_line_x(&ui));
    key(&mut ui, Key::Home);
    ui.update();
    assert!((long_line_x(&ui) - start).abs() < 0.5, "Home shows the start again");
}
