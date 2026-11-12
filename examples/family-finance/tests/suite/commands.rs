//! Transactions and bills, whose lists and forms come from what they are, driven as a person
//! drives them.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::Updater;
use ravenrs::core::{Event, Key, Modifiers, PointerButton};
use ravenrs::prelude::*;

fn today() -> Date {
    Date::new(2026, 11, 15).expect("valid date")
}

fn settle(ui: &mut Ui) {
    ui.update();
    ui.update();
    ui.update();
}

fn app(page: &str) -> Ui {
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today()).expect("seed");
    let mut ui = Ui::new(App::new(db, today(), None, Updater::idle()), Theme::raven_light());
    ui.set_viewport(Size::new(1360.0, 860.0), 1.0);
    settle(&mut ui);
    press(&mut ui, Role::Button, page);
    ui
}

fn press(ui: &mut Ui, role: Role, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref().is_some_and(|t| t.starts_with(name)))
        .unwrap_or_else(|| panic!("no {role:?} '{name}'"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    settle(ui);
}

fn button(ui: &mut Ui, name: &str) -> Option<bool> {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
        .map(|n| !n.disabled)
}

fn says(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| {
        [&n.name, &n.value, &n.description]
            .iter()
            .any(|field| field.as_deref().is_some_and(|t| t.contains(text)))
    })
}

fn select_row(ui: &mut Ui, text: &str) {
    let tree = ui.access_tree();
    let spot = tree
        .nodes
        .iter()
        .find(|n| {
            n.role == Role::Cell && (n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
        })
        .unwrap_or_else(|| panic!("no row with {text}"))
        .bounds
        .center();
    ui.handle_event(&Event::PointerMove { position: spot });
    ui.handle_event(&Event::PointerDown { position: spot, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: spot, button: PointerButton::Primary });
    settle(ui);
}

fn type_into(ui: &mut Ui, field: &str, text: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
        .unwrap_or_else(|| panic!("no field '{field}'"))
        .id;
    ui.perform_action(id, AccessAction::Focus);
    settle(ui);
    for character in text.chars() {
        ui.handle_event(&Event::KeyDown {
            key: Key::Character(character.to_string()),
            modifiers: Modifiers::default(),
        });
    }
    settle(ui);
}

#[test]
fn what_acts_on_a_row_waits_for_one_to_be_selected() {
    let mut ui = app("Lançamentos");
    assert_eq!(button(&mut ui, "Alterar"), Some(false));
    assert_eq!(button(&mut ui, "Excluir"), Some(false));
    select_row(&mut ui, "Cinema");
    assert_eq!(button(&mut ui, "Alterar"), Some(true));
    assert_eq!(button(&mut ui, "Excluir"), Some(true));
}

#[test]
fn changing_a_transaction_opens_it_as_it_is_and_keeps_the_change() {
    let mut ui = app("Lançamentos");
    select_row(&mut ui, "Cinema");
    press(&mut ui, Role::Button, "Alterar");
    assert!(says(&mut ui, "Alterar lançamento"));
    assert!(says(&mut ui, "Lazer"), "its own category");
    type_into(&mut ui, "Descrição", " 3D");
    press(&mut ui, Role::Button, "Salvar lançamento");
    assert!(says(&mut ui, "Lançamento alterado"));
    assert!(says(&mut ui, "Cinema 3D"));
}

#[test]
fn deleting_a_transaction_asks_in_its_own_words() {
    let mut ui = app("Lançamentos");
    select_row(&mut ui, "Cinema");
    press(&mut ui, Role::Button, "Excluir");
    assert!(says(&mut ui, "Se ele pagou uma conta"));
    press(&mut ui, Role::Button, "Excluir lançamento");
    assert!(says(&mut ui, "Lançamento excluído"));
    assert!(!says(&mut ui, "Cinema"));
    assert_eq!(button(&mut ui, "Alterar"), Some(false), "what went away is not selected");
}

#[test]
fn changing_the_type_of_a_transaction_changes_its_categories() {
    let mut ui = app("Lançamentos");
    press(&mut ui, Role::Button, "Nova despesa");
    press(&mut ui, Role::RadioButton, "Receita");
    type_into(&mut ui, "Descrição", "Bico");
    type_into(&mut ui, "Valor", "50");
    press(&mut ui, Role::Button, "Salvar lançamento");
    assert!(says(&mut ui, "Lançamento salvo"));
    assert!(says(&mut ui, "+R$ 50,00"), "it came in, under a category of what comes in");
}

#[test]
fn an_amount_of_zero_is_refused_on_the_field() {
    let mut ui = app("Lançamentos");
    press(&mut ui, Role::Button, "Nova despesa");
    type_into(&mut ui, "Descrição", "Nada");
    type_into(&mut ui, "Valor", "0");
    press(&mut ui, Role::Button, "Salvar lançamento");
    assert!(says(&mut ui, "O valor precisa ser maior que zero."));
}

#[test]
fn the_kinds_and_expressions_narrow_the_month_and_its_totals() {
    let mut ui = app("Lançamentos");
    assert!(says(&mut ui, "Salário Bruno"));
    press(&mut ui, Role::RadioButton, "Despesas");
    assert!(!says(&mut ui, "Salário Bruno"));
    assert!(says(&mut ui, "Entrou R$ 0,00"));
    press(&mut ui, Role::RadioButton, "Todos");
    press(&mut ui, Role::Button, "Filtrar");
    press(&mut ui, Role::MenuItem, "Na carteira");
    assert!(says(&mut ui, "Estacionamento"));
    assert!(!says(&mut ui, "Cinema"), "only what came out of the wallet");
}

#[test]
fn paying_a_bill_and_undoing_it_are_commands_that_follow_the_row() {
    let mut ui = app("Contas a pagar");
    assert_eq!(button(&mut ui, "Pagar"), Some(false));
    assert_eq!(button(&mut ui, "Desfazer pagamento"), None, "nothing paid is selected");
    select_row(&mut ui, "Material escolar");
    press(&mut ui, Role::Button, "Pagar");
    assert!(says(&mut ui, "Pagar conta"));
    press(&mut ui, Role::Button, "Confirmar pagamento");
    assert!(says(&mut ui, "Conta paga"));
    assert!(!says(&mut ui, "Material escolar"), "no longer open");

    press(&mut ui, Role::RadioButton, "Pagas");
    select_row(&mut ui, "Material escolar");
    assert_eq!(button(&mut ui, "Pagar"), Some(false), "what was paid has nothing to pay");
    assert_eq!(button(&mut ui, "Alterar"), Some(false));
    assert_eq!(button(&mut ui, "Excluir"), Some(false));
    press(&mut ui, Role::Button, "Desfazer pagamento");
    assert!(says(&mut ui, "A conta volta a ficar em aberto"));
    let tree = ui.access_tree();
    let answer = tree
        .nodes
        .iter()
        .rfind(|n| n.role == Role::Button && n.name.as_deref() == Some("Desfazer pagamento"))
        .expect("the answer")
        .id;
    ui.perform_action(answer, AccessAction::Click);
    settle(&mut ui);
    assert!(says(&mut ui, "Pagamento desfeito"));
    assert!(!says(&mut ui, "Material escolar"), "no longer among the paid ones");
}

#[test]
fn the_choices_at_the_top_show_each_kind_of_bill() {
    let mut ui = app("Contas a pagar");
    assert!(says(&mut ui, "Água e esgoto"));
    press(&mut ui, Role::RadioButton, "Vencidas");
    assert!(says(&mut ui, "Plano de saúde"));
    assert!(!says(&mut ui, "Água e esgoto"));
}

#[test]
fn splitting_a_bill_changes_what_its_form_asks() {
    let mut ui = app("Contas a pagar");
    press(&mut ui, Role::Button, "Nova conta");
    assert!(says(&mut ui, "Repetição"));
    assert!(says(&mut ui, "Parcelas: 1, à vista"));
    type_into(&mut ui, "Descrição", "Sofá");
    type_into(&mut ui, "Valor", "300");
    press(&mut ui, Role::Button, "Aumentar");
    press(&mut ui, Role::Button, "Aumentar");
    assert!(says(&mut ui, "Valor total"), "the amount is of all the parts");
    assert!(says(&mut ui, "Primeiro vencimento"));
    assert!(says(&mut ui, "3x de R$ 100,00"));
    assert!(!says(&mut ui, "Repetição"), "what is split does not repeat too");
    press(&mut ui, Role::Button, "Salvar conta");
    assert!(says(&mut ui, "Parcelas criadas"));
    assert!(says(&mut ui, "Sofá (1/3)"));
}
