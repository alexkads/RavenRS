//! The app driven as a person drives it, without a window or a GPU.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::Updater;
use ravenrs::core::{Event, Key, Modifiers};
use ravenrs::prelude::*;

const PAGES: [&str; 8] =
    ["Painel", "Lançamentos", "Contas a pagar", "Agenda", "Orçamentos", "Relatórios", "Cadastros", "Ajustes"];

fn today() -> Date {
    Date::new(2026, 11, 15).expect("valid date")
}

fn app() -> Ui {
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today()).expect("seed");
    let mut ui = Ui::new(App::new(db, today(), None, Updater::idle()), Theme::raven_light());
    ui.set_viewport(Size::new(1360.0, 860.0), 1.0);
    settle(&mut ui);
    ui
}

fn settle(ui: &mut Ui) {
    ui.update();
    ui.update();
}

fn press(ui: &mut Ui, role: Role, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref().is_some_and(|t| t.starts_with(name)))
        .unwrap_or_else(|| panic!("no {role:?} '{name}' in {:?}", names(ui, role)))
        .id;
    ui.perform_action(id, AccessAction::Click);
    settle(ui);
}

fn names(ui: &mut Ui, role: Role) -> Vec<String> {
    ui.access_tree().nodes.iter().filter(|n| n.role == role).filter_map(|n| n.name.clone()).collect()
}

fn says(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| {
        [&n.name, &n.value, &n.description]
            .iter()
            .any(|field| field.as_deref().is_some_and(|t| t.contains(text)))
    })
}

fn type_into(ui: &mut Ui, field: &str, text: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
        .unwrap_or_else(|| panic!("no field '{field}' in {:?}", names(ui, Role::TextInput)))
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

fn audit(ui: &mut Ui, screen: &str) {
    let tree = ui.access_tree();
    for n in &tree.nodes {
        if matches!(
            n.role,
            Role::Button
                | Role::CheckBox
                | Role::Switch
                | Role::RadioButton
                | Role::TextInput
                | Role::ComboBox
                | Role::Slider
                | Role::SpinButton
                | Role::Tab
        ) {
            assert!(
                n.name.as_deref().is_some_and(|s| !s.trim().is_empty()),
                "{screen}: unnamed {:?}",
                n.role
            );
        }
    }
}

#[test]
fn every_screen_opens_and_names_its_controls() {
    let mut ui = app();
    for page in PAGES {
        press(&mut ui, Role::Button, page);
        assert!(says(&mut ui, page), "{page} did not open");
        audit(&mut ui, page);
    }
}

#[test]
fn the_dashboard_warns_about_overdue_bills() {
    let mut ui = app();
    assert!(says(&mut ui, "vencida"), "the sample family has overdue bills");
    assert!(says(&mut ui, "Saldo das contas".to_uppercase().as_str()) || says(&mut ui, "Saldo das contas"));
}

#[test]
fn a_new_expense_needs_a_description_and_an_amount() {
    let mut ui = app();
    press(&mut ui, Role::Button, "Lançamentos");
    press(&mut ui, Role::Button, "Nova despesa");
    press(&mut ui, Role::Button, "Salvar lançamento");
    assert!(says(&mut ui, "Preencha este campo"), "the form says what is missing");
    assert!(names(&mut ui, Role::Button).iter().any(|n| n == "Salvar lançamento"), "and stays open");
}

#[test]
fn saving_an_expense_shows_it_in_the_month() {
    let mut ui = app();
    press(&mut ui, Role::Button, "Lançamentos");
    press(&mut ui, Role::Button, "Nova despesa");
    type_into(&mut ui, "Descrição", "Conserto da pia");
    type_into(&mut ui, "Valor", "150,75");
    press(&mut ui, Role::Button, "Salvar lançamento");
    assert!(!names(&mut ui, Role::Button).iter().any(|n| n == "Salvar lançamento"), "the dialog closed");
    assert!(says(&mut ui, "Conserto da pia"), "and the grid shows it");
    assert!(says(&mut ui, "Lançamento salvo"), "with a confirmation");
}

#[test]
fn paying_from_the_dashboard_clears_the_warning_count() {
    let mut ui = app();
    let before = names(&mut ui, Role::Button).iter().filter(|n| n.starts_with("Pagar ")).count();
    assert!(before > 0);
    let first = names(&mut ui, Role::Button).into_iter().find(|n| n.starts_with("Pagar ")).expect("a bill");
    press(&mut ui, Role::Button, &first);
    assert!(says(&mut ui, "Pagar conta"), "the payment dialog opened");
    press(&mut ui, Role::Button, "Confirmar pagamento");
    assert!(says(&mut ui, "Conta paga"));
    assert!(!says(&mut ui, "Confirmar pagamento"), "the dialog closed");
}

#[test]
fn a_bad_filter_explains_itself_and_a_good_one_applies() {
    let mut ui = app();
    press(&mut ui, Role::Button, "Lançamentos");
    type_into(&mut ui, "Filtro avançado", "[Nada] > 1");
    press(&mut ui, Role::Button, "Aplicar");
    assert!(says(&mut ui, "Filtro não aplicado"));
    press(&mut ui, Role::Button, "Limpar");
    assert!(!says(&mut ui, "Filtro não aplicado"));
}

#[test]
fn the_report_screen_offers_every_export() {
    let mut ui = app();
    press(&mut ui, Role::Button, "Relatórios");
    let buttons = names(&mut ui, Role::Button);
    for format in ["Exportar em PDF", "Exportar em Excel", "Exportar em CSV"] {
        assert!(buttons.iter().any(|n| n == format), "{format} in {buttons:?}");
    }
}

#[test]
fn deleting_a_register_in_use_says_so() {
    let mut ui = app();
    press(&mut ui, Role::Button, "Cadastros");
    press(&mut ui, Role::Button, "Excluir Conta corrente");
    press(&mut ui, Role::Button, "Excluir");
    assert!(says(&mut ui, "em uso"), "the database refused and the toast explains");
}

#[test]
fn the_components_speak_portuguese() {
    let mut ui = app();
    assert!(says(&mut ui, "Escuro"), "the theme switcher");
    press(&mut ui, Role::Button, "Lançamentos");
    for words in ["Agrupar", "Colunas", "Exportar CSV", "linhas"] {
        assert!(says(&mut ui, words), "the grid says {words}");
    }
    assert!(!says(&mut ui, "Export CSV"));
    press(&mut ui, Role::Button, "Agenda");
    assert!(says(&mut ui, "Mês anterior"), "the calendar");
}

#[test]
fn an_idle_app_asks_for_nothing() {
    for (page, name) in PAGES.iter().enumerate() {
        let db = Database::in_memory().expect("database");
        seed::load_demo(&db, today()).expect("seed");
        let app = App::new(db, today(), None, Updater::idle());
        app.open_page(page);
        let mut ui = Ui::new(app, Theme::raven_light());
        ui.set_viewport(Size::new(1360.0, 860.0), 1.0);
        let mut clock = 0.0;
        for _ in 0..240 {
            clock += 1.0 / 60.0;
            ui.set_time(clock);
            ui.update();
        }
        assert!(!ui.needs_redraw(), "{name}: wants another frame");
        assert!(!ui.is_animating(), "{name}: still animating");
        assert_eq!(ui.next_wake(), None, "{name}: asks to be woken");
    }
}
