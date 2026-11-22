//! The editor through its interface: only library components, joined.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs::core::AccessAction;
use ravenrs::prelude::*;

fn opened() -> Ui {
    let mut ui = Ui::new(bpmn_editor::app(), Theme::raven_light());
    ui.set_viewport(Size::new(1440.0, 900.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn find(ui: &mut Ui, name: &str) -> Option<ravenrs::core::AccessNode> {
    ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).cloned()
}

fn press(ui: &mut Ui, name: &str) {
    let id = find(ui, name).unwrap_or_else(|| panic!("no {name}")).id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
}

#[test]
fn the_editor_is_made_of_the_library_and_checks_the_process() {
    let mut ui = opened();
    for part in ["Processos", "Elementos BPMN", "Processo", "Verificar"] {
        assert!(find(&mut ui, part).is_some(), "{part}");
    }
    press(&mut ui, "Verificar");
    assert!(find(&mut ui, "Nenhum problema").is_some(), "the example process is well formed");
}

#[test]
fn without_a_model_the_assistant_says_how_to_get_one() {
    let mut ui = opened();
    press(&mut ui, bpmn_editor::REVIEW);
    let said = ui.access_tree().nodes.iter().any(|n| {
        n.name.as_deref().is_some_and(|t| t.contains("Escolha um modelo"))
            || n.description.as_deref().is_some_and(|t| t.contains("Escolha um modelo"))
            || n.value.as_deref().is_some_and(|t| t.contains("Escolha um modelo"))
    });
    assert!(said);
}

#[test]
fn the_editor_starts_with_two_processes_one_of_them_between_participants() {
    let mut ui = opened();
    let starts = |ui: &mut Ui, prefix: &str| {
        ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|t| t.starts_with(prefix)))
    };
    assert!(starts(&mut ui, "Compra aprovada"), "the open process has its name");
    assert!(!starts(&mut ui, "Sem nome"));
    press(&mut ui, "Pedido entre cliente e loja 8 nós");
    assert!(find(&mut ui, "Loja").is_some() && find(&mut ui, "Cliente").is_some(), "the participants show");
}
