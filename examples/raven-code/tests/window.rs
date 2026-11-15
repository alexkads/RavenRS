//! The window on a project: open files, edit, save, and what the compiler found.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

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
