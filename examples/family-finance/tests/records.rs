//! The registers, generated from what they are, driven as a person drives them.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::Updater;
use ravenrs::core::{Event, Key, Modifiers};
use ravenrs::prelude::*;

fn settle(ui: &mut Ui) {
    ui.update();
    ui.update();
}

fn app() -> Ui {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let mut ui = Ui::new(App::new(db, today, None, Updater::idle()), Theme::raven_light());
    ui.set_viewport(Size::new(1360.0, 860.0), 1.0);
    settle(&mut ui);
    press(&mut ui, "Cadastros");
    ui
}

fn press(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| {
            matches!(n.role, Role::Button | Role::Tab)
                && n.name.as_deref().is_some_and(|t| t.starts_with(name))
        })
        .unwrap_or_else(|| panic!("no button '{name}'"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    settle(ui);
}

fn says(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| {
        [&n.name, &n.value, &n.description]
            .iter()
            .any(|field| field.as_deref().is_some_and(|t| t.contains(text)))
    })
}

fn fill(ui: &mut Ui, field: &str, text: &str) {
    let tree = ui.access_tree();
    let node = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
        .unwrap_or_else(|| panic!("no field '{field}'"));
    let (id, held) = (node.id, node.value.clone().unwrap_or_default());
    ui.perform_action(id, AccessAction::Focus);
    settle(ui);
    let key = |ui: &mut Ui, key: Key| {
        ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        settle(ui);
    };
    key(ui, Key::End);
    for _ in held.chars() {
        key(ui, Key::Backspace);
    }
    for character in text.chars() {
        let pressed = if character == ' ' { Key::Space } else { Key::Character(character.to_string()) };
        key(ui, pressed);
    }
}

#[test]
fn an_account_is_added_changed_and_removed_with_its_balance() {
    let mut ui = app();
    fill(&mut ui, "Nome", "Banco do bairro");
    fill(&mut ui, "Saldo inicial", "100");
    press(&mut ui, "Adicionar conta");
    assert!(says(&mut ui, "Banco do bairro"), "in the list");
    assert!(says(&mut ui, "R$ 100,00"), "its balance, worked out by the database");
    assert!(says(&mut ui, "Conta salva"), "the app's own message");
    press(&mut ui, "Alterar Banco do bairro");
    assert!(says(&mut ui, "Alterar conta"), "the form says so");
    fill(&mut ui, "Nome", "Banco novo");
    press(&mut ui, "Salvar alterações");
    assert!(says(&mut ui, "Banco novo"));
    assert!(!says(&mut ui, "Banco do bairro"));
    press(&mut ui, "Excluir Banco novo");
    press(&mut ui, "Excluir");
    assert!(!says(&mut ui, "Banco novo"), "with no transactions, it can go");
    assert!(says(&mut ui, "Conta excluída"));
}

#[test]
fn a_name_is_required_and_cannot_repeat() {
    let mut ui = app();
    press(&mut ui, "Adicionar conta");
    assert!(says(&mut ui, "Nome é obrigatório"), "it says so, instead of doing nothing");
    fill(&mut ui, "Nome", "Poupança");
    assert!(!says(&mut ui, "Nome é obrigatório"), "it goes as it gets fixed");
    press(&mut ui, "Adicionar conta");
    assert!(says(&mut ui, "Já existe um registro com esse nome"));
    assert!(says(&mut ui, "Nova conta"), "the form stays with what was typed");
    fill(&mut ui, "Nome", "Outra");
    press(&mut ui, "Adicionar conta");
    assert!(says(&mut ui, "Outra"));
}

#[test]
fn canceling_goes_back_to_a_new_record_and_changes_nothing() {
    let mut ui = app();
    press(&mut ui, "Alterar Carteira");
    fill(&mut ui, "Nome", "Mudou");
    press(&mut ui, "Cancelar");
    assert!(says(&mut ui, "Carteira"));
    assert!(!says(&mut ui, "Mudou"));
    assert!(says(&mut ui, "Nova conta"));
}

#[test]
fn a_category_has_a_type_and_no_limit_is_empty() {
    let mut ui = app();
    press(&mut ui, "Categorias");
    assert!(says(&mut ui, "Limite mensal"));
    fill(&mut ui, "Nome", "Pets");
    press(&mut ui, "Adicionar categoria");
    assert!(says(&mut ui, "Pets"), "with no limit, kept as zero");
    press(&mut ui, "Alterar Pets");
    let empty = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Limite mensal") && n.role == Role::TextInput)
        .is_some_and(|n| n.value.as_deref().unwrap_or("").is_empty());
    assert!(empty, "no limit reads as an empty field");
    press(&mut ui, "Cancelar");
    press(&mut ui, "Excluir Pets");
    press(&mut ui, "Excluir");
    assert!(!says(&mut ui, "Pets"));
}

#[test]
fn a_person_can_go_and_what_they_did_stays() {
    let mut ui = app();
    press(&mut ui, "Família");
    fill(&mut ui, "Nome", "Maria Nova");
    press(&mut ui, "Adicionar pessoa");
    assert!(says(&mut ui, "Maria Nova"));
    assert!(says(&mut ui, "Pessoa salva"));
    press(&mut ui, "Remover Maria Nova");
    assert!(says(&mut ui, "Os lançamentos dela continuam"));
    press(&mut ui, "Remover");
    assert!(!says(&mut ui, "Maria Nova"));
}

#[test]
fn what_other_screens_do_shows_here() {
    let mut ui = app();
    press(&mut ui, "Ajustes");
    press(&mut ui, "Cadastros");
    assert!(says(&mut ui, "R$ 48.945,83"), "the balance from the transactions");
}

#[test]
fn an_account_named_after_its_kind_has_nothing_more_to_say() {
    let mut ui = app();
    assert!(!says(&mut ui, "Poupança Poupança"));
    fill(&mut ui, "Nome", "Corretora");
    press(&mut ui, "Adicionar conta");
    let cells: Vec<String> = ui
        .access_tree()
        .nodes
        .iter()
        .filter(|n| n.role == Role::Cell)
        .filter_map(|n| n.name.clone().or_else(|| n.value.clone()))
        .collect();
    assert!(cells.iter().any(|c| c == "Conta corrente"), "the kind of one named otherwise: {cells:?}");
}
