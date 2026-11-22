//! `ParameterForm`: a form made from a schema, driven through what screen readers see: lists of
//! rows, options that depend on another field, a failed load, Descartar and expressions.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, PointerButton, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;
use serde_json::{Value, json};

fn ui(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 900.0), 1.0);
    settle(&mut ui);
    ui
}

fn settle(ui: &mut Ui) {
    for _ in 0..3 {
        ui.update();
    }
}

fn has(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree()
        .nodes
        .iter()
        .any(|n| [&n.name, &n.value, &n.description].into_iter().flatten().any(|t| t.contains(text)))
}

fn act(ui: &mut Ui, name: &str, action: AccessAction) {
    let id = ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).id;
    ui.perform_action(id, action);
    settle(ui);
}

fn press(ui: &mut Ui, name: &str) {
    act(ui, name, AccessAction::Click);
}

/// Types `text` at the end of the field `name`.
fn type_into(ui: &mut Ui, name: &str, text: &str) {
    let at = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name) && n.role == Role::TextInput)
        .expect(name)
        .bounds
        .center();
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.handle_event(&Event::KeyDown { key: Key::End, modifiers: Modifiers::default() });
    ui.update();
    for c in text.chars() {
        let key = if c == ' ' { Key::Space } else { Key::Character(c.to_string()) };
        ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        ui.update();
    }
    settle(ui);
}

fn map(value: Value) -> ParamValues {
    ParamValues::from(value.as_object().cloned().unwrap_or_default())
}

#[test]
fn a_list_of_headers_grows_and_shrinks_and_apply_asks_for_what_is_required() {
    let fields = vec![
        ParamField::new("url", "URL", ParamKind::Text).required(),
        ParamField::new(
            "headers",
            "Headers",
            ParamKind::List(vec![
                ParamField::new("name", "Name", ParamKind::Text),
                ParamField::new("value", "Value", ParamKind::Text).default("1"),
            ]),
        ),
    ];
    let values = State::new(ParamValues::new());
    let mut ui = ui(ParameterForm::new(fields, values.binding()));
    press(&mut ui, "Add");
    press(&mut ui, "Add");
    assert!(has(&mut ui, "Remove row 2"), "two rows");
    press(&mut ui, "Apply");
    assert!(has(&mut ui, "URL is required"), "nothing is given back with a required field empty");
    assert!(values.get().0.is_empty());
    type_into(&mut ui, "URL", "https://x");
    press(&mut ui, "Apply");
    assert_eq!(values.get().get("url").cloned().unwrap_or(Value::Null), json!("https://x"));
    assert_eq!(
        values.get().get("headers").cloned().unwrap_or(Value::Null),
        json!([{ "value": "1" }, { "value": "1" }]),
        "rows with their defaults"
    );
    press(&mut ui, "Remove row 1");
    press(&mut ui, "Apply");
    assert_eq!(values.get().get("headers").cloned().unwrap_or(Value::Null).as_array().map(Vec::len), Some(1));
}

#[test]
fn options_that_depend_on_a_field_are_asked_again_and_a_choice_they_lost_is_cleared() {
    let calls: Rc<RefCell<Vec<(String, Value)>>> = Rc::default();
    let options = State::new(ParamOptions::Loading);
    let values = State::new(map(json!({ "file": "x", "sheet": "b" })));
    let fields = vec![
        ParamField::new("file", "File", ParamKind::Text),
        ParamField::new("sheet", "Sheet", ParamKind::Loaded).depends_on(["file"]),
    ];
    let (heard, shown, bound) = (calls.clone(), options.clone(), values.clone());
    let mut ui = ui(view_fn(move || {
        let heard = heard.clone();
        ParameterForm::new(fields.clone(), bound.binding()).options("sheet", shown.get()).on_load(
            move |field, now| {
                heard.borrow_mut().push((field.to_owned(), now.get("file").cloned().unwrap_or(Value::Null)))
            },
        )
    }));
    assert_eq!(*calls.borrow(), [("sheet".to_owned(), json!("x"))], "asked the first time it shows");
    assert!(has(&mut ui, "Loading the options"));

    options.set(ParamOptions::Ready(vec![("a".into(), "A".into()), ("b".into(), "B".into())]));
    settle(&mut ui);
    type_into(&mut ui, "File", "y");
    assert_eq!(
        calls.borrow().last(),
        Some(&("sheet".to_owned(), json!("xy"))),
        "asked again with the new file"
    );
    options.set(ParamOptions::Ready(vec![("c".into(), "C".into())]));
    settle(&mut ui);
    press(&mut ui, "Apply");
    assert_eq!(
        values.get().get("sheet").cloned().unwrap_or(Value::Null),
        Value::Null,
        "the sheet the new file lacks is cleared"
    );

    options.set(ParamOptions::Failed("offline".into()));
    settle(&mut ui);
    assert!(has(&mut ui, "The options could not be loaded"), "the failure says so");
    assert!(has(&mut ui, "File"), "and the rest of the form stays");
    let before = calls.borrow().len();
    press(&mut ui, "Try again");
    assert_eq!(calls.borrow().len(), before + 1, "Try again asks once more");
}

#[test]
fn an_expression_is_kept_as_one_and_discard_forgets_everything_edited() {
    let values = State::new(map(json!({ "condition": "paid", "mode": "fast" })));
    let fields = vec![
        ParamField::new("condition", "Condition", ParamKind::Text).expressions(),
        ParamField::new(
            "mode",
            "Mode",
            ParamKind::Choice(vec![("fast".into(), "Fast".into()), ("safe".into(), "Safe".into())]),
        ),
        ParamField::new("retries", "Retries", ParamKind::Number).default(3).shown_when("mode", "safe"),
    ];
    let discarded = State::new(false);
    let d = discarded.clone();
    let mut ui = ui(ParameterForm::new(fields, values.binding()).on_discard(move || d.set(true)));
    assert!(!has(&mut ui, "Retries"), "shown only in the safe mode");
    press(&mut ui, "Expression");
    type_into(&mut ui, "Condition", " = true");
    press(&mut ui, "Discard");
    assert!(discarded.get());
    assert_eq!(
        values.get().get("condition").cloned().unwrap_or(Value::Null),
        json!("paid"),
        "nothing edited reached the values"
    );
    assert!(!has(&mut ui, "paid = true"), "and the copy went back");

    press(&mut ui, "Expression");
    type_into(&mut ui, "Condition", " = true");
    press(&mut ui, "Apply");
    assert_eq!(
        values.get().get("condition").cloned().unwrap_or(Value::Null),
        json!({ EXPRESSION_KEY: "paid = true" }),
        "kept as an expression"
    );
    assert_eq!(
        values.get().get("retries").cloned().unwrap_or(Value::Null),
        json!(3),
        "a default fills what the values lacked"
    );
}
