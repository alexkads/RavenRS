//! The viewer driven as a user drives it, through its accessibility tree.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
use ravenrs_reactive::State;
use ravenrs_report::{Band, Control, Document, ParameterKind, Report, ReportEngine};
use ravenrs_report_viewer::{DocumentViewer, ParameterPanel, ParameterValues, ViewerLabels, ViewerState};
use ravenrs_style::Theme;
use ravenrs_widgets::{AnyView, Date, Ui, VStack};
use serde_json::json;

fn document(rows: usize) -> Rc<Document> {
    let report =
        Report::new("T").detail(Band::new(200.0).control(Control::field("[N]").at(0.0, 0.0, 300.0, 20.0)));
    let data: Vec<_> = (0..rows).map(|i| json!({ "N": format!("Região {i}") })).collect();
    Rc::new(ReportEngine::new().render(&report, &json!(data)))
}

fn viewer_ui(doc: Rc<Document>, state: &ViewerState) -> Ui {
    let state = state.clone();
    let mut ui = Ui::new(
        ravenrs_widgets::view_fn(move || {
            DocumentViewer::new(doc.clone(), &state).labels(ViewerLabels::pt_br())
        }),
        Theme::raven_light(),
    );
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.update();
    ui
}

fn press(ui: &mut Ui, role: Role, name: &str) {
    let id = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref().is_some_and(|t| t.starts_with(name)))
        .unwrap_or_else(|| panic!("no {role:?} '{name}'"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
}

fn has(ui: &mut Ui, role: Role, name: &str) -> bool {
    ui.access_tree()
        .nodes
        .iter()
        .any(|n| n.role == role && n.name.as_deref().is_some_and(|t| t.starts_with(name)))
}

#[test]
fn pages_turn_with_buttons_and_keys_and_stop_at_the_ends() {
    let state = ViewerState::new();
    let mut ui = viewer_ui(document(20), &state);
    press(&mut ui, Role::Button, "Próxima");
    assert_eq!(state.page().get(), 1);
    press(&mut ui, Role::Button, "Anterior");
    press(&mut ui, Role::Button, "Anterior");
    assert_eq!(state.page().get(), 0, "stops at the first page");
    ui.handle_event(&Event::KeyDown { key: Key::PageDown, modifiers: Modifiers::default() });
    ui.update();
    assert_eq!(state.page().get(), 1);
    ui.handle_event(&Event::KeyDown { key: Key::PageUp, modifiers: Modifiers::default() });
    ui.update();
    assert_eq!(state.page().get(), 0);
}

#[test]
fn zoom_stays_in_its_limits_and_fits_the_width() {
    let state = ViewerState::new();
    let mut ui = viewer_ui(document(1), &state);
    for _ in 0..20 {
        press(&mut ui, Role::Button, "Aproximar");
    }
    assert_eq!(state.zoom().get(), 3.0);
    for _ in 0..20 {
        press(&mut ui, Role::Button, "Afastar");
    }
    assert_eq!(state.zoom().get(), 0.5);
    press(&mut ui, Role::Button, "Ajustar");
    let zoom = state.zoom().get();
    assert!(zoom > 1.5 && zoom < 2.1, "{zoom}");
}

#[test]
fn the_page_text_reaches_screen_readers() {
    let state = ViewerState::new();
    let mut ui = viewer_ui(document(1), &state);
    assert!(has(&mut ui, Role::Image, "Região 0"));
}

#[test]
fn thumbnails_show_around_the_page_and_go_to_a_page() {
    let state = ViewerState::new();
    let mut ui = viewer_ui(document(120), &state);
    let thumbs = |ui: &mut Ui| -> Vec<String> {
        ui.access_tree()
            .nodes
            .iter()
            .filter(|n| n.role == Role::Button)
            .filter_map(|n| n.name.clone())
            .filter(|t| t.starts_with("Miniatura da página "))
            .collect()
    };
    assert!(thumbs(&mut ui).is_empty(), "hidden until asked");
    press(&mut ui, Role::Button, "Miniaturas");
    assert!(thumbs(&mut ui).contains(&"Miniatura da página 1".to_owned()));
    press(&mut ui, Role::Button, "Miniatura da página 3");
    assert_eq!(state.page().get(), 2);

    state.page().set(30);
    ui.update();
    let shown = thumbs(&mut ui);
    assert!(shown.contains(&"Miniatura da página 31".to_owned()), "the current page is in view: {shown:?}");
    assert!(!shown.contains(&"Miniatura da página 1".to_owned()), "far pages are not built");
    assert!(shown.len() <= 12, "{}", shown.len());
}

#[test]
fn search_jumps_to_results_and_steps_around() {
    let state = ViewerState::new();
    let doc = document(20);
    let pages = doc.pages.len();
    assert!(pages >= 4);
    let mut ui = viewer_ui(doc, &state);
    state.query().set("regiao 19".to_owned());
    ui.update();
    assert!(
        has(&mut ui, Role::Label, "1 de 1")
            || ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("1 de 1"))
    );

    state.query().set("regiao".to_owned());
    ui.update();
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("1 de 20")));
    press(&mut ui, Role::Button, "Próximo resultado");
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("2 de 20")));
    press(&mut ui, Role::Button, "Resultado anterior");
    press(&mut ui, Role::Button, "Resultado anterior");
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("20 de 20")), "goes around");
    assert_eq!(state.page().get(), pages - 1, "the page follows the result");

    state.query().set("xyz".to_owned());
    ui.update();
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Não encontrado")));
    assert!(!has(&mut ui, Role::Button, "Próximo resultado"));
}

#[test]
fn a_new_document_replaces_the_old_results_and_pages() {
    let state = ViewerState::new();
    let current = State::new(document(3));
    let (read, st) = (current.clone(), state.clone());
    let mut ui =
        Ui::new(ravenrs_widgets::view_fn(move || DocumentViewer::new(read.get(), &st)), Theme::raven_light());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.update();
    state.query().set("Região 2".to_owned());
    ui.update();
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("1 of 1")));
    current.set(document(1));
    ui.update();
    assert!(
        ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Not found")),
        "the search ran again"
    );
}

#[test]
fn a_page_past_the_end_shows_the_last_one() {
    let state = ViewerState::new();
    state.page().set(99);
    let mut ui = viewer_ui(document(1), &state);
    assert!(has(&mut ui, Role::Image, "Região 0"));
}

#[test]
fn an_empty_document_does_not_break_the_viewer() {
    let state = ViewerState::new();
    state.query().set("x".to_owned());
    let mut ui = viewer_ui(Rc::new(Document::default()), &state);
    press(&mut ui, Role::Button, "Miniaturas");
    press(&mut ui, Role::Button, "Ajustar");
}

#[test]
fn report_problems_are_shown() {
    let report =
        Report::new("T").detail(Band::new(20.0).control(Control::field("Nope(").at(0.0, 0.0, 100.0, 20.0)));
    let doc = Rc::new(ReportEngine::new().render(&report, &json!([{}])));
    assert!(!doc.errors.is_empty());
    let mut ui = viewer_ui(doc, &ViewerState::new());
    assert!(
        ui.access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref().is_some_and(|t| t.contains("Erro no relatório")))
    );
}

fn panel(values: &State<ParameterValues>) -> Ui {
    let report = Report::new("T")
        .parameter("Region", ParameterKind::Text, json!("Sul"))
        .parameter("Min", ParameterKind::Number, json!(10))
        .parameter("Paid", ParameterKind::Boolean, json!(false))
        .parameter("From", ParameterKind::Date, json!("2026-03-05"));
    let parameters = report.parameters;
    let today = Date::new(2026, 11, 1).unwrap();
    let values = values.clone();
    let mut ui = Ui::new(
        ravenrs_widgets::view_fn(move || {
            VStack::new(vec![
                Box::new(ParameterPanel::new(parameters.clone(), values.clone(), today)) as AnyView
            ])
        }),
        Theme::raven_light(),
    );
    ui.set_viewport(Size::new(1200.0, 400.0), 1.0);
    ui.update();
    ui
}

#[test]
fn values_apply_together_and_reset_clears_them() {
    let values: State<ParameterValues> = State::new(HashMap::new());
    let mut ui = panel(&values);
    press(&mut ui, Role::CheckBox, "Paid");
    assert!(values.get().is_empty(), "nothing is applied until asked");
    press(&mut ui, Role::Button, "Apply");
    assert_eq!(values.get().get("Paid"), Some(&json!(true)));
    press(&mut ui, Role::Button, "Reset");
    assert!(values.get().is_empty());
}

#[test]
fn each_kind_gets_its_control() {
    let values: State<ParameterValues> = State::new(HashMap::new());
    let mut ui = panel(&values);
    let tree = ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.role == Role::CheckBox), "boolean");
    assert!(tree.nodes.iter().filter(|n| n.role == Role::TextInput).count() >= 2, "text and number");
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("From")), "date");
}
