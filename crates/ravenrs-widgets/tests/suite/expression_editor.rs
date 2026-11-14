//! The expression editor: the check while typing, the groups, the search and inserting
//! at the caret.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn editor(text: &str) -> (Ui, State<String>) {
    let value = State::new(text.to_owned());
    let view = Localized::new(
        Labels::pt_br(),
        ExpressionEditor::new(value.binding())
            .field(FilterField::new("Quantity", "Quantidade", FilterFieldKind::Number))
            .field(FilterField::new("Customer", "Cliente", FilterFieldKind::Text))
            .parameter(FilterField::new("MinTotal", "Total mínimo", FilterFieldKind::Number)),
    );
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 700.0), 1.0);
    ui.update();
    (ui, value)
}

fn says(ui: &mut Ui, part: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|s| s.contains(part)))
}

fn click(ui: &mut Ui, role: Role, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref().is_some_and(|s| s.starts_with(name)))
        .unwrap_or_else(|| panic!("no {role:?} '{name}'"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
}

#[test]
fn it_checks_the_expression_as_it_changes() {
    let (mut ui, value) = editor("[Quantity] * 2");
    assert!(says(&mut ui, "A expressão está correta."));
    value.set("[Quantity] *\n  (2 +".into());
    ui.update();
    assert!(says(&mut ui, "Linha 2, coluna"), "a syntax error says where it is");
    value.set("[Quantidade] > ?MinTotal And Upperr([Customer]) = 'A'".into());
    ui.update();
    assert!(says(&mut ui, "Desconhecido"));
    assert!(says(&mut ui, "[Quantidade], Upperr()"), "the unknown field and function, in order");
}

#[test]
fn a_second_click_on_a_function_puts_it_at_the_caret() {
    let (mut ui, value) = editor("[Customer] = ");
    click(&mut ui, Role::Button, "Texto");
    click(&mut ui, Role::Button, "Upper");
    assert!(says(&mut ui, "Upper(text)"), "the chosen function's signature");
    assert_eq!(value.get(), "[Customer] = ", "one click only shows it");
    click(&mut ui, Role::Button, "Upper");
    assert_eq!(value.get(), "[Customer] = Upper()");
    click(&mut ui, Role::Button, "Campos");
    click(&mut ui, Role::Button, "Cliente");
    click(&mut ui, Role::Button, "Inserir");
    assert_eq!(value.get(), "[Customer] = Upper([Customer])", "inside the parentheses, where the caret went");
}

#[test]
fn groups_show_one_at_a_time_and_the_search_looks_in_all() {
    let (mut ui, _) = editor("");
    click(&mut ui, Role::Button, "Parâmetros");
    assert!(says(&mut ui, "Total mínimo"));
    click(&mut ui, Role::Button, "Data e hora");
    assert!(says(&mut ui, "AddDays"));
    assert!(!says(&mut ui, "Total mínimo"), "one group at a time");
    let search = ui.access_tree().nodes.iter().rfind(|n| n.role == Role::TextInput).expect("search").id;
    ui.perform_action(search, AccessAction::Focus);
    for c in "quant".chars() {
        ui.handle_event(&Event::KeyDown {
            key: Key::Character(c.to_string()),
            modifiers: Modifiers::default(),
        });
        ui.update();
    }
    ui.update();
    assert!(says(&mut ui, "Quantidade"), "a field found from the date group");
    assert!(!says(&mut ui, "AddDays"));
}

#[test]
fn syntax_errors_and_function_help_are_in_the_labels_language() {
    let (mut ui, value) = editor("[Quantity] *\n  (2 +");
    assert!(says(&mut ui, "a expressão terminou antes da hora"), "the parser's error, translated");
    value.set("Total * 2".into());
    ui.update();
    assert!(says(&mut ui, "nome desconhecido 'Total' (campos vão entre colchetes: [Total])"));
    value.set("[Customer] = ".into());
    ui.update();
    click(&mut ui, Role::Button, "Texto");
    click(&mut ui, Role::Button, "Upper");
    assert!(says(&mut ui, "Em maiúsculas."), "the chosen function's description, translated");
}

#[test]
fn every_catalog_function_and_syntax_error_has_brazilian_portuguese() {
    let labels = Labels::pt_br();
    let english = Labels::default();
    for f in ravenrs_criteria::functions::catalog() {
        let key = if f.category == ravenrs_criteria::Category::Aggregate {
            format!("Aggregate.{}", f.name)
        } else {
            f.name.to_owned()
        };
        let pt = labels.expression_function(&key).unwrap_or_else(|| panic!("no pt-BR text for {key}"));
        assert_ne!(Some(pt), english.expression_function(&key), "{key} is still in English");
    }
    for (code, message) in ravenrs_criteria::ParseError::CODES {
        let pt =
            labels.expression_error(code).unwrap_or_else(|| panic!("no pt-BR text for the error {code}"));
        assert_ne!(pt, *message, "the error {code} is still in English");
        assert_eq!(english.expression_error(code), Some(*message), "English follows the parser");
        for i in 0..4 {
            let slot = format!("{{{i}}}");
            assert_eq!(pt.contains(&slot), message.contains(&slot), "{code} keeps {slot}");
        }
    }
}
