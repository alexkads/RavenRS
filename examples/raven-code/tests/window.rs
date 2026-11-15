//! The window on a project: open files, edit, save, and what the compiler found.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod common;

use std::fs;

use raven_code::RavenCode;
use raven_code::project::{Project, write_sample};
use ravenrs::core::{AccessNode, Event, Key, Modifiers};
use ravenrs::prelude::*;

fn app(name: &str) -> (RavenCode, Ui, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("raven-code-window-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_sample(&root).unwrap();
    let app = RavenCode::new(Project::open(&root).unwrap());
    let mut ui = Ui::new(app.clone(), Theme::raven_dark());
    ui.set_viewport(Size::new(1440.0, 900.0), 1.0);
    settle(&mut ui);
    (app, ui, root)
}

fn settle(ui: &mut Ui) {
    for _ in 0..6 {
        ui.update();
    }
}

fn node(ui: &mut Ui, name: &str) -> Option<AccessNode> {
    ui.access_tree().nodes.into_iter().find(|n| n.name.as_deref() == Some(name))
}

#[test]
fn it_opens_main_rs_with_the_explorer_terminal_and_problems_around_it() {
    let (app, mut ui, _) = app("opens");
    assert_eq!(app.active().as_deref(), Some("src/main.rs"));
    for name in
        ["Explorer", "Terminal", "Problems", "main.rs", "src", "Cargo.toml", "Build", "Run", "Save all"]
    {
        assert!(node(&mut ui, name).is_some(), "{name} is missing");
    }
    let area = node(&mut ui, "Workbench").expect("the dock");
    assert!(area.bounds.height() > 600.0 && area.bounds.width() > 1300.0, "{:?}", area.bounds);
}

#[test]
fn opening_a_file_from_the_tree_adds_its_tab() {
    let (app, mut ui, _) = app("tree");
    app.open_file("src/greeting.rs");
    settle(&mut ui);
    assert_eq!(app.active().as_deref(), Some("src/greeting.rs"));
    assert!(node(&mut ui, "greeting.rs").is_some());
    assert!(app.layout().side_of("src/greeting.rs") == Some(DockSide::Center));
}

#[test]
fn an_edit_shows_the_dot_and_the_keys_save_it() {
    let (app, mut ui, root) = app("save");
    app.edit("src/main.rs", "fn main() {}\n");
    settle(&mut ui);
    assert!(app.is_unsaved("src/main.rs"));
    assert_eq!(node(&mut ui, "main.rs").and_then(|n| n.description).as_deref(), Some("Unsaved changes"));
    ui.handle_event(&Event::KeyDown {
        key: Key::Character("s".into()),
        modifiers: Modifiers { ctrl: true, ..Modifiers::default() },
    });
    settle(&mut ui);
    assert!(!app.is_unsaved("src/main.rs"));
    assert_eq!(fs::read_to_string(root.join("src/main.rs")).unwrap(), "fn main() {}\n");
}

#[test]
fn what_the_compiler_found_is_listed_marked_and_leads_to_its_place() {
    let (app, mut ui, _) = app("problems");
    let error = r#"{"reason":"compiler-message","message":{"rendered":"error: cannot find function `hi`\n","level":"error","message":"cannot find function `hi`","spans":[{"file_name":"src/greeting.rs","line_start":3,"line_end":3,"column_start":5,"column_end":7,"is_primary":true}]}}"#;
    app.take_output(&[error, r#"{"reason":"build-finished","success":false}"#], false);
    settle(&mut ui);
    let row = "src/greeting.rs:3:5 · cannot find function `hi`";
    assert!(node(&mut ui, row).is_some(), "listed under Problems");
    assert!(node(&mut ui, "Build failed · 1 error, 0 warnings").is_some(), "and said in the status bar");
    let place = node(&mut ui, row).unwrap().bounds;
    ui.handle_event(&Event::PointerMove { position: place.center() });
    ui.handle_event(&Event::PointerDown {
        position: place.center(),
        button: ravenrs::core::PointerButton::Primary,
    });
    ui.handle_event(&Event::PointerUp {
        position: place.center(),
        button: ravenrs::core::PointerButton::Primary,
    });
    settle(&mut ui);
    assert_eq!(app.active().as_deref(), Some("src/greeting.rs"), "the click opens its file");
    let text = app.text_of("src/greeting.rs").unwrap();
    assert_eq!(app.caret_of("src/greeting.rs"), Some(offset_of(&text, 3, 5)), "with the caret on it");
}

fn click(ui: &mut Ui, name: &str) {
    let spot = node(ui, name).unwrap_or_else(|| panic!("{name} is missing")).bounds.center();
    ui.handle_event(&Event::PointerMove { position: spot });
    ui.handle_event(&Event::PointerDown { position: spot, button: ravenrs::core::PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: spot, button: ravenrs::core::PointerButton::Primary });
    settle(ui);
}

#[test]
fn closing_with_changes_asks_and_saving_closes() {
    let (app, mut ui, root) = app("quit");
    assert!(ui.may_close(), "nothing to lose, it closes");
    app.edit("src/main.rs", "fn main() { /* edited */ }\n");
    settle(&mut ui);
    assert!(!ui.may_close(), "with changes it asks first");
    settle(&mut ui);
    assert_eq!(app.asking().as_deref(), Some("unsaved"));
    assert!(node(&mut ui, "Save your changes?").is_some());
    click(&mut ui, "Cancel");
    assert!(app.asking().is_none() && !ui.take_close(), "cancel keeps it open");
    assert!(!ui.may_close());
    settle(&mut ui);
    click(&mut ui, "Save and close");
    assert!(ui.take_close(), "saved, it closes");
    assert_eq!(fs::read_to_string(root.join("src/main.rs")).unwrap(), "fn main() { /* edited */ }\n");
}

#[test]
fn closing_without_saving_leaves_the_disk_alone() {
    let (app, mut ui, root) = app("discard");
    let before = fs::read_to_string(root.join("src/main.rs")).unwrap();
    app.edit("src/main.rs", "broken");
    settle(&mut ui);
    assert!(!ui.may_close());
    settle(&mut ui);
    click(&mut ui, "Don't save and close");
    assert!(ui.take_close());
    assert_eq!(fs::read_to_string(root.join("src/main.rs")).unwrap(), before);
}

#[test]
fn a_file_that_cannot_be_written_keeps_the_window_open() {
    let (app, mut ui, root) = app("readonly");
    let file = root.join("src/main.rs");
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&file, permissions.clone()).unwrap();
    app.edit("src/main.rs", "fn main() {}\n");
    settle(&mut ui);
    assert!(!ui.may_close());
    settle(&mut ui);
    click(&mut ui, "Save and close");
    #[allow(clippy::permissions_set_readonly_false, reason = "the test puts it back")]
    permissions.set_readonly(false);
    fs::set_permissions(&file, permissions).unwrap();
    assert!(!ui.take_close(), "the save failed: it stays open");
    assert!(app.is_unsaved("src/main.rs"));
}

#[test]
fn closing_the_tab_of_a_changed_file_asks_about_it_alone() {
    let (app, mut ui, root) = app("tab");
    app.open_file("src/greeting.rs");
    app.edit("src/main.rs", "changed");
    settle(&mut ui);
    // A file with no changes closes at once.
    click(&mut ui, "Close greeting.rs");
    assert_eq!(app.open_files(), ["src/main.rs"]);
    assert!(app.asking().is_none());
    // The changed one asks, about itself only.
    click(&mut ui, "Close main.rs");
    assert_eq!(app.asking().as_deref(), Some("unsaved src/main.rs"));
    assert_eq!(app.open_files(), ["src/main.rs"], "still open while it asks");
    click(&mut ui, "Save and close it");
    assert!(app.open_files().is_empty());
    assert_eq!(fs::read_to_string(root.join("src/main.rs")).unwrap(), "changed");
    assert!(node(&mut ui, "Open a file from the Explorer").is_some(), "the empty center");
}

#[test]
fn the_explorer_makes_renames_and_deletes_files() {
    let (app, mut ui, root) = app("explorer");
    use raven_code::app::NamingFor;
    app.name_it(&NamingFor::File("src".into()), "util.rs").unwrap();
    settle(&mut ui);
    assert!(root.join("src/util.rs").exists());
    assert_eq!(app.active().as_deref(), Some("src/util.rs"), "a new file opens");
    assert!(node(&mut ui, "util.rs").is_some(), "and shows in the tree");
    // A name that is there, or a bad one, says why and changes nothing.
    assert!(app.name_it(&NamingFor::File("src".into()), "main.rs").is_err());
    assert_eq!(app.name_it(&NamingFor::Folder("".into()), "a/b"), Err("A name cannot have / or \\ in it".into()));
    // Renaming an open file moves its tab and its changes.
    app.open_file("src/greeting.rs");
    app.edit("src/greeting.rs", "pub fn hello(_: &str) -> String { String::new() }\n");
    app.name_it(&NamingFor::Rename("src/greeting.rs".into()), "hello.rs").unwrap();
    settle(&mut ui);
    assert!(app.open_files().contains(&"src/hello.rs".to_owned()));
    assert!(!app.open_files().contains(&"src/greeting.rs".to_owned()));
    assert!(app.is_unsaved("src/hello.rs"), "the changes came along");
    assert!(app.save("src/hello.rs"));
    assert!(fs::read_to_string(root.join("src/hello.rs")).unwrap().contains("String::new()"));
    // Deleting closes what was open.
    app.delete("src/util.rs").unwrap();
    settle(&mut ui);
    assert!(!root.join("src/util.rs").exists());
    assert!(!app.open_files().contains(&"src/util.rs".to_owned()));
}

#[test]
fn the_explorer_menu_asks_for_a_name() {
    let (app, mut ui, _) = app("menu");
    let spot = node(&mut ui, "src").unwrap().bounds.center();
    ui.handle_event(&Event::PointerMove { position: spot });
    ui.handle_event(&Event::PointerDown { position: spot, button: ravenrs::core::PointerButton::Secondary });
    settle(&mut ui);
    for item in ["New File…", "New Folder…", "Rename…", "Delete"] {
        assert!(node(&mut ui, item).is_some(), "{item} is in the menu");
    }
    click(&mut ui, "Delete");
    assert_eq!(app.asking().as_deref(), Some("delete src"));
    assert!(node(&mut ui, "Delete src?").is_some());
    click(&mut ui, "Cancel");
    assert!(app.asking().is_none());
}

#[test]
fn a_change_on_disk_shows_and_open_files_follow_it() {
    let (app, mut ui, root) = app("disk");
    fs::write(root.join("src/extra.rs"), "// extra\n").unwrap();
    fs::write(root.join("src/main.rs"), "fn main() { /* from outside */ }\n").unwrap();
    app.refresh();
    settle(&mut ui);
    let mut files = Vec::new();
    raven_code::project::collect_files(&app.tree(), &mut files);
    assert!(files.contains(&"src/extra.rs".to_owned()), "a new file comes in the tree");
    assert_eq!(app.text_of("src/main.rs").as_deref(), Some("fn main() { /* from outside */ }\n"));
    // With changes here, it asks which to keep.
    app.edit("src/main.rs", "mine");
    fs::write(root.join("src/main.rs"), "theirs").unwrap();
    app.refresh();
    settle(&mut ui);
    assert!(node(&mut ui, "This file changed on disk.").is_some());
    click(&mut ui, "Keep mine");
    assert_eq!(app.text_of("src/main.rs").as_deref(), Some("mine"));
}

#[test]
fn the_project_is_searched_and_a_hit_opens_its_place() {
    let (app, mut ui, _) = app("search");
    app.search("hello");
    app.layout();
    settle(&mut ui);
    let hits = app.hits();
    let call = hits.iter().find(|h| h.file == "src/main.rs").expect("the call");
    app.go_to_place(&call.file.clone(), call.line, call.column);
    settle(&mut ui);
    assert_eq!(app.active().as_deref(), Some("src/main.rs"));
    let text = app.text_of("src/main.rs").unwrap();
    assert_eq!(app.caret_of("src/main.rs"), Some(offset_of(&text, 4, 30)));
}

#[test]
fn a_session_comes_back_and_recent_projects_show_when_nothing_is_open() {
    use raven_code::config::Config;
    let (app, mut ui, root) = app("session");
    let config = Config::at(root.join("../raven-code-config-session"));
    let _ = fs::remove_dir_all(config.dir());
    let app = app.remembering(config.clone());
    app.open_file("src/greeting.rs");
    app.go_to_place("src/greeting.rs", 3, 5);
    settle(&mut ui);
    assert!(ui.may_close(), "nothing unsaved: it closes and keeps the session");
    // Opened again, it is as it was.
    let again = RavenCode::new(Project::open(&root).unwrap()).remembering(config.clone());
    assert_eq!(again.open_files(), ["src/main.rs", "src/greeting.rs"]);
    assert_eq!(again.active().as_deref(), Some("src/greeting.rs"));
    let text = again.text_of("src/greeting.rs").unwrap();
    assert_eq!(again.caret_of("src/greeting.rs"), Some(offset_of(&text, 3, 5)));
    assert_eq!(config.recent(), [root.clone()]);
    let _ = fs::remove_dir_all(config.dir());
}

#[test]
fn tests_say_how_many_passed_and_run_says_when_there_is_nothing_to_run() {
    let (app, mut ui, _) = app("tasks");
    app.take_output(&["running 1 test", "test result: ok. 1 passed; 0 failed; 0 ignored"], true);
    settle(&mut ui);
    // Taken as the output of a build: the counts show once a test ran.
    assert_eq!(app.report().tests, Some((1, 0)));
    app.set_binaries(&[]);
    app.run(raven_code::cargo::Task::Run);
    settle(&mut ui);
    assert!(node(&mut ui, "No binary to run").is_some());
    assert!(!app.report().running, "cargo was not called");
    app.set_binaries(&["server", "tool"]);
    settle(&mut ui);
    assert!(node(&mut ui, "Binary").is_some(), "with two, a choice shows");
}

#[test]
fn the_language_server_marks_while_typing_and_f12_goes_to_the_definition() {
    let (app, mut ui, root) = app("lsp");
    let (reader, writer) = common::fake_server(&root);
    let app = app.with_language_server_on(reader, writer);
    let wait = |ui: &mut Ui, what: &str, done: &dyn Fn(&mut Ui) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !done(ui) {
            assert!(std::time::Instant::now() < deadline, "waited too long for {what}");
            std::thread::sleep(std::time::Duration::from_millis(10));
            settle(ui);
        }
    };
    let text = app.text_of("src/main.rs").unwrap().replace("println!", "printn!");
    app.edit("src/main.rs", &text);
    settle(&mut ui);
    click(&mut ui, "Problems");
    let row = "src/main.rs:4:5 · cannot find macro `printn` in this scope";
    wait(&mut ui, "the problem", &|ui| node(ui, row).is_some());
    // F12 on `hello` opens greeting.rs at its definition.
    let at = text.find("hello").unwrap();
    app.go_to_place("src/main.rs", 4, text[..at].rsplit('\n').next().unwrap().chars().count() + 1);
    settle(&mut ui);
    ui.handle_event(&Event::KeyDown { key: Key::Function(12), modifiers: Modifiers::default() });
    wait(&mut ui, "the definition", &|ui| {
        settle(ui);
        app.active().as_deref() == Some("src/greeting.rs")
    });
    let greeting = app.text_of("src/greeting.rs").unwrap();
    assert_eq!(app.caret_of("src/greeting.rs"), Some(offset_of(&greeting, 2, 8)));
}
