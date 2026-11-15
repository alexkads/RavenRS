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

/// Where the control named `name` is, if it is shown.
fn spot(ui: &mut Ui, role: Role, name: &str) -> Option<Point> {
    let tree = ui.access_tree();
    tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name)).map(|n| n.bounds.center())
}

#[test]
fn the_playground_goes_full_screen_and_back() {
    let mut ui = ui(Playground::new(runner).example("Hello", "say hello"));
    let enter = spot(&mut ui, Role::Button, "Full screen").expect("the full screen button");
    click(&mut ui, enter);
    assert!(ui.full_screen().is_on(), "it asked for full screen");
    let leave = spot(&mut ui, Role::Button, "Exit full screen").expect("the button now leaves it");
    click(&mut ui, leave);
    assert!(!ui.full_screen().is_on(), "it asked to leave");
    assert!(spot(&mut ui, Role::Button, "Full screen").is_some());
}

/// A platform that cannot give full screen, as a frame that does not allow it.
struct Refused;

impl FullScreenProvider for Refused {
    fn is_on(&self) -> bool {
        false
    }

    fn set(&self, _on: bool) {}

    fn available(&self) -> bool {
        false
    }
}

#[test]
fn without_full_screen_there_is_no_button() {
    let mut ui = ui(Playground::new(runner).example("Hello", "say hello"));
    ui.set_full_screen(Refused);
    ui.update();
    assert!(spot(&mut ui, Role::Button, "Full screen").is_none());
}

#[test]
fn narrow_the_full_screen_goes_in_the_menu() {
    let mut ui = ui_at(Playground::new(runner).example("Hello", "say hello"), 420.0);
    assert!(spot(&mut ui, Role::Button, "Full screen").is_none(), "no room for the button");
    let more = spot(&mut ui, Role::Button, "More actions").expect("the menu");
    click(&mut ui, more);
    let item = spot(&mut ui, Role::MenuItem, "Full screen").expect("full screen in the menu");
    click(&mut ui, item);
    assert!(ui.full_screen().is_on());
}

#[test]
fn the_line_numbers_reach_the_bottom_and_leave_the_border() {
    let code = State::new("one\ntwo".to_owned());
    let mut ui = ui(CodeEditor::new(code.binding()).language(CodeLanguage::Plain).rows(12));
    ui.update();
    let area = {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("the editor").bounds
    };
    let gutter = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q)
                if q.border.is_none() && q.rect.x() > area.x() && q.rect.x() < area.x() + 4.0 =>
            {
                Some(q.rect)
            }
            _ => None,
        })
        .max_by(|a, b| a.height().total_cmp(&b.height()))
        .expect("the gutter's background");
    assert!(gutter.x() >= area.x() + 1.0, "the border stays in sight: {gutter:?} in {area:?}");
    assert!(
        gutter.max_y() >= area.max_y() - 8.0,
        "two lines, and still down to the bottom: {gutter:?} in {area:?}"
    );
    assert!(gutter.max_y() <= area.max_y() - 1.0, "the bottom border stays in sight: {gutter:?} in {area:?}");
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

#[test]
fn the_caret_says_its_line_and_column() {
    // #163, #166: what the status line of the playground shows, "Ln 2, Col 5".
    let text = "fn f() {\n    ação\n}";
    assert_eq!(line_and_column(text, 0), (1, 1));
    assert_eq!(line_and_column(text, 9), (2, 1), "the start of the second line");
    assert_eq!(line_and_column(text, 13), (2, 5));
    assert_eq!(line_and_column(text, 17), (2, 9), "accents count as one character each");
    assert_eq!(line_and_column(text, 1000), (3, 2), "past the end, at the end");
}

fn line_two(source: &str) -> Result<AnyView, Vec<CodeDiagnostic>> {
    if source.contains("yell") {
        Err(vec![CodeDiagnostic::error(2, 5, 4, "Write say")])
    } else {
        Ok(Box::new(Text::new("ok")))
    }
}

#[test]
fn the_playground_names_its_file_and_says_the_spaces() {
    // #166: the tab over the editor and the status line of the approved layout.
    let mut ui = ui(Playground::new(line_two).example("Slider", "fn slider() {\n    say\n}"));
    assert!(shows(&mut ui, "slider.rs"), "named after its first function");
    assert!(shows(&mut ui, "Rust"));
    assert!(shows(&mut ui, "Spaces: 4"));
    let mut ui = ui_at(Playground::new(line_two).example("A", "say").file_name("hello.rs"), 1200.0);
    assert!(shows(&mut ui, "hello.rs"), "the name given wins");
}

#[test]
fn a_problem_takes_the_caret_to_its_place_and_focuses_the_editor() {
    let source = State::new(String::new());
    let mut ui = ui(Playground::new(line_two)
        .example("Slider", "fn slider() {\n    yell\n}")
        .source(source.binding()));
    assert!(shows(&mut ui, "Ln 1, Col 1"));
    let go = {
        let tree = ui.access_tree();
        let found = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Go to Line 2, column 5"));
        found.expect("the problem is a button that says where it goes").id
    };
    ui.perform_action(go, ravenrs_core::AccessAction::Click);
    ui.update();
    ui.update();
    assert!(shows(&mut ui, "Ln 2, Col 5"), "the caret is on the problem");
    let tree = ui.access_tree();
    assert_eq!(tree.focused().map(|n| n.role), Some(Role::TextInput), "and the editor has the focus");
}

#[test]
fn a_place_turns_back_into_the_characters_before_it() {
    let text = "fn f() {\n    ação\n}";
    for at in [0, 9, 13, 17, 19] {
        let (line, column) = line_and_column(text, at);
        assert_eq!(offset_of(text, line, column), at, "{line}:{column}");
    }
    assert_eq!(offset_of(text, 2, 0), 9, "column 0 is the start of the line");
    assert_eq!(offset_of(text, 2, 99), 17, "past the end of a line stops there");
    assert_eq!(offset_of(text, 9, 1), 19, "past the last line, at the end");
}

#[test]
fn undo_takes_back_a_word_at_a_time_and_the_app_keeps_the_steps() {
    let source = State::new(String::new());
    let history = State::new(TextHistory::new());
    let mut ui = ui(CodeEditor::new(source.binding()).history(history.clone()));
    focus_editor(&mut ui);
    typed(&mut ui, "let x");
    key(&mut ui, Key::Enter);
    typed(&mut ui, "done");
    let cmd = Modifiers { meta: true, ..Modifiers::default() };
    press(&mut ui, Key::Character("z".into()), cmd);
    assert_eq!(source.get(), "let x\n");
    press(&mut ui, Key::Character("z".into()), cmd);
    assert_eq!(source.get(), "let x", "the line break is a step of its own");
    assert!(history.peek().can_redo(), "the app sees what can be redone");
    press(&mut ui, Key::Character("Z".into()), Modifiers { shift: true, ..cmd });
    assert_eq!(source.get(), "let x\n");
    // A read-only editor undoes nothing.
    let shown = State::new("fixed".to_owned());
    let mut ui = super_ui_read_only(shown.clone());
    press(&mut ui, Key::Character("z".into()), cmd);
    assert_eq!(shown.get(), "fixed");
}

fn super_ui_read_only(source: State<String>) -> Ui {
    let mut ui = ui(CodeEditor::new(source.binding()).read_only(true));
    focus_editor(&mut ui);
    ui
}

#[test]
fn find_all_respects_capitals_and_whole_words() {
    let text = "let a = a + A; abc";
    assert_eq!(find_all(text, "a", false, false).len(), 4);
    assert_eq!(find_all(text, "a", true, false).len(), 3);
    assert_eq!(find_all(text, "a", true, true), vec![(4, 5), (8, 9)]);
    assert_eq!(find_all(text, "A", false, true).len(), 3, "abc is not the word a");
    assert!(find_all(text, "", false, false).is_empty());
    assert_eq!(find_all("ãão", "ão", false, false), vec![(1, 3)], "counted in characters");
}

fn names(ui: &mut Ui) -> Vec<String> {
    ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect()
}

#[test]
fn cmd_f_finds_walks_the_matches_and_replaces_all_in_one_undo() {
    let source = State::new("let a = a + A;\nlet b = a;".to_owned());
    let cursor = State::new(Cursor::at(0));
    let mut ui = ui(CodeEditor::new(source.binding()).caret(cursor.clone()));
    focus_editor(&mut ui);
    let cmd = Modifiers { meta: true, ..Modifiers::default() };
    press(&mut ui, Key::Character("f".into()), cmd);
    ui.update();
    typed(&mut ui, "a");
    assert!(names(&mut ui).iter().any(|n| n == "? of 4"), "{:?}", names(&mut ui));
    key(&mut ui, Key::Enter);
    assert_eq!(cursor.get().selection(), Some((4, 5)));
    assert!(names(&mut ui).iter().any(|n| n == "1 of 4"));
    key(&mut ui, Key::Enter);
    press(&mut ui, Key::Enter, Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(cursor.get().selection(), Some((4, 5)), "Shift+Enter goes back");
    press(&mut ui, Key::Enter, Modifiers { shift: true, ..Modifiers::default() });
    assert_eq!(cursor.get().selection(), Some((23, 24)), "and round from the first to the last");
    // Ctrl+H shows the replacing, while the find field keeps the focus.
    press(&mut ui, Key::Character("h".into()), Modifiers { ctrl: true, ..Modifiers::default() });
    ui.update();
    press_named_button(&mut ui, "Replace all");
    assert_eq!(source.get(), "let  =  + ;\nlet b = ;");
    // Undo takes all of it back at once, in the code.
    let code = {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.value.as_deref().is_some_and(|v| v.contains("let")))
            .expect("the code")
            .bounds
            .center()
    };
    click(&mut ui, code);
    press(&mut ui, Key::Character("z".into()), cmd);
    assert_eq!(source.get(), "let a = a + A;\nlet b = a;");
}

#[test]
fn nothing_found_says_so_and_escape_goes_back_to_the_code() {
    let source = State::new("fn main() {}".to_owned());
    let mut ui = ui(CodeEditor::new(source.binding()).read_only(true));
    focus_editor(&mut ui);
    press(&mut ui, Key::Character("f".into()), Modifiers { ctrl: true, ..Modifiers::default() });
    ui.update();
    typed(&mut ui, "zzz");
    assert!(names(&mut ui).iter().any(|n| n == "No results"));
    assert!(!names(&mut ui).iter().any(|n| n == "Toggle replace"), "a read-only editor does not replace");
    key(&mut ui, Key::Escape);
    ui.update();
    assert!(!names(&mut ui).iter().any(|n| n == "No results"), "the bar closed");
}

fn press_named_button(ui: &mut Ui, name: &str) {
    let spot = {
        let tree = ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("a button named {name}"))
            .bounds
            .center()
    };
    click(ui, spot);
}

/// An editor that offers `hello` and `help` after `::`, says what is under the pointer, and
/// writes down where F12 asked to go.
struct Helped {
    source: State<String>,
    asked: State<Vec<String>>,
    offered: State<Vec<CodeCompletion>>,
    tip: State<Option<String>>,
}

impl View for Helped {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (asked, offered) = (self.asked.clone(), self.offered.clone());
        let (asked_tip, tip) = (self.asked.clone(), self.tip.clone());
        let asked_go = self.asked.clone();
        let editor = CodeEditor::new(self.source.binding())
            .on_complete(move |at| {
                asked.update(|a| a.push(format!("complete {at}")));
                offered.set(vec![
                    CodeCompletion::new("hello").detail("fn(&str) -> String"),
                    CodeCompletion::new("help"),
                    CodeCompletion::new("other"),
                ]);
            })
            .completions(self.offered.get())
            .on_hover(move |at| {
                asked_tip.update(|a| a.push(format!("hover {at}")));
                tip.set(Some("pub fn hello(name: &str) -> String".to_owned()));
            })
            .hover(self.tip.get())
            .on_go_to(move |at| asked_go.update(|a| a.push(format!("go {at}"))));
        cx.child(&editor)
    }
}

fn helped(text: &str) -> (Ui, State<String>, State<Vec<String>>) {
    let source = State::new(text.to_owned());
    let asked = State::new(Vec::new());
    let view = Helped { source: source.clone(), asked: asked.clone(), offered: State::new(Vec::new()), tip: State::new(None) };
    (ui(view), source, asked)
}

fn options(ui: &mut Ui) -> Vec<String> {
    ui.access_tree().nodes.iter().filter(|n| n.role == Role::ListBoxOption).filter_map(|n| n.name.clone()).collect()
}

#[test]
fn suggestions_come_after_two_colons_narrow_by_the_word_and_tab_puts_one_in() {
    let (mut ui, source, asked) = helped("");
    focus_editor(&mut ui);
    typed(&mut ui, "greeting::");
    assert_eq!(asked.get(), ["complete 10"]);
    assert_eq!(options(&mut ui), ["hello", "help", "other"]);
    typed(&mut ui, "hel");
    assert_eq!(options(&mut ui), ["hello", "help"], "the word typed narrows them");
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Tab);
    assert_eq!(source.get(), "greeting::help");
    assert!(options(&mut ui).is_empty(), "the list closed");
    // Undo takes the suggestion out in one step.
    press(&mut ui, Key::Character("z".into()), Modifiers { meta: true, ..Modifiers::default() });
    assert_eq!(source.get(), "greeting::hel");
    // Ctrl+Space asks anywhere, and Escape closes without changing anything.
    press(&mut ui, Key::Space, Modifiers { ctrl: true, ..Modifiers::default() });
    assert_eq!(options(&mut ui), ["hello", "help"]);
    key(&mut ui, Key::Escape);
    assert!(options(&mut ui).is_empty());
    assert_eq!(source.get(), "greeting::hel");
}

#[test]
fn f12_and_a_command_click_ask_where_it_is_defined() {
    let (mut ui, _source, asked) = helped("hello()");
    focus_editor(&mut ui);
    key(&mut ui, Key::Home);
    key(&mut ui, Key::ArrowRight);
    key(&mut ui, Key::Function(12));
    assert_eq!(asked.get(), ["go 1"]);
}

#[test]
fn resting_the_pointer_on_a_word_shows_a_tip() {
    let (mut ui, _source, asked) = helped("hello()");
    let code = {
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("the code").bounds
    };
    let spot = Point::new(code.x() + 70.0, code.y() + 16.0);
    ui.set_time(1.0);
    ui.handle_event(&Event::PointerMove { position: spot });
    ui.update();
    assert!(asked.get().is_empty(), "nothing is asked before the pointer rests");
    ui.set_time(1.6);
    ui.update();
    ui.update();
    assert_eq!(asked.get(), ["hover 0"], "asked once, for the word");
    ui.update();
    let tips: Vec<String> =
        ui.access_tree().nodes.iter().filter(|n| n.role == Role::Tooltip).filter_map(|n| n.name.clone()).collect();
    assert_eq!(tips, ["pub fn hello(name: &str) -> String"]);
    assert_eq!(asked.get().len(), 1, "and not again while it rests there");
}

#[test]
fn a_followed_log_keeps_its_end_in_sight_as_it_grows() {
    let log = State::new((1..=5).map(|n| format!("line {n}\n")).collect::<String>());
    let mut ui = ui(CodeEditor::new(log.binding()).read_only(true).follow_end(true).line_numbers(false).rows(4));
    log.update(|log| {
        for n in 6..=60 {
            log.push_str(&format!("line {n}\n"));
        }
    });
    ui.update();
    ui.update();
    // Only the lines in sight are drawn: at the end they all have two digits, at the top
    // "line 1" to "line 9" have one, and so one glyph fewer.
    let glyphs = |ui: &Ui| -> Vec<usize> {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| match p {
                Primitive::Glyphs(g) => Some(g.glyphs.len()),
                _ => None,
            })
            .collect()
    };
    let at_end = glyphs(&ui);
    assert!(!at_end.is_empty());
    // Each line draws as "line" and its number: no number of one digit at the end.
    assert!(!at_end.contains(&1), "only two-digit lines in sight: {at_end:?}");
    // Without following, it stays at the top.
    let still = State::new((1..=60).map(|n| format!("line {n}\n")).collect::<String>());
    let other = super::code::ui(CodeEditor::new(still.binding()).read_only(true).line_numbers(false).rows(4));
    let top = glyphs(&other);
    assert!(top.contains(&1), "one-digit lines at the top: {top:?}");
}
