//! `DataInspector`: the same items as a table, a tree, JSON and a schema, the field chosen kept
//! from one to the next, absent told from null, big whole numbers exact and long lists cut.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessAction, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;
use serde_json::json;

fn ui(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 900.0), 1.0);
    for _ in 0..3 {
        ui.update();
    }
    ui
}

fn texts(ui: &mut Ui) -> Vec<String> {
    ui.access_tree().nodes.iter().flat_map(|n| [n.name.clone(), n.value.clone()]).flatten().collect()
}

fn says(ui: &mut Ui, text: &str) -> bool {
    texts(ui).iter().any(|t| t.contains(text))
}

fn press(ui: &mut Ui, name: &str) {
    let id = ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).id;
    ui.perform_action(id, AccessAction::Click);
    for _ in 0..3 {
        ui.update();
    }
}

const ORDERS: &str = r#"[
    { "cliente": { "nome": "Ana" }, "total": 1234567890123456789, "desconto": null },
    { "cliente": { "nome": "Bia" }, "total": 10 }
]"#;

#[test]
fn the_same_items_show_four_ways_and_the_field_chosen_stays() {
    let mode = State::new(0_usize);
    let chosen = State::new(None::<String>);
    let mut ui = ui(DataInspector::new(ORDERS).mode(mode.binding()).selected(chosen.binding()));
    assert!(
        says(&mut ui, "1234567890123456789"),
        "a 19-digit whole number stays exact: {:?}",
        texts(&mut ui)
    );
    assert!(says(&mut ui, "null"), "the null discount");
    assert!(says(&mut ui, "—"), "and the absent one, told apart");
    assert!(says(&mut ui, "{1}"), "an object says its size");

    press(&mut ui, "Tree");
    assert!(says(&mut ui, "Item 1 of 2"));
    chosen.set(Some("cliente.nome".into()));
    press(&mut ui, "Next item");
    assert!(says(&mut ui, "Item 2 of 2"));
    assert!(says(&mut ui, "total: 10"), "the second item's tree, its branches closed");

    press(&mut ui, "Schema");
    assert!(says(&mut ui, "▸ cliente.nome"), "the field chosen stays chosen in the schema");
    assert!(says(&mut ui, "Field chosen: cliente.nome"));
    assert!(says(&mut ui, "desconto"), "a field one item lacks");
    assert!(says(&mut ui, "1 of 2"), "is in one of two");

    press(&mut ui, "JSON");
    assert!(says(&mut ui, "\"Bia\""), "the JSON of the item shown: {:?}", texts(&mut ui));
    assert_eq!(mode.get(), 2);
    assert_eq!(chosen.get().as_deref(), Some("cliente.nome"), "and the field chosen did not change");
}

#[test]
fn a_hundred_thousand_items_open_cut_with_a_notice() {
    let many: Vec<serde_json::Value> = (0..100_000).map(|i| json!({ "n": i })).collect();
    let started = std::time::Instant::now();
    let mut ui = ui(DataInspector::of(many).limit(50));
    assert!(says(&mut ui, "Showing 50 of 100000 items"), "{:?}", &texts(&mut ui)[..5]);
    assert!(started.elapsed().as_secs() < 5, "and it does not hang");
}

#[test]
fn a_pick_in_the_tree_is_heard_as_a_path_with_quotes_where_needed() {
    let heard = State::new(String::new());
    let h = heard.clone();
    let mut ui = ui(DataInspector::new(r#"{ "nome completo": "Ana", "itens": [ { "preço": 3 } ] }"#)
        .mode(State::new(1_usize).binding())
        .on_pick(move |path| h.set(path.to_owned())));
    press(&mut ui, "nome completo: Ana");
    assert_eq!(heard.get(), r#"["nome completo"]"#);
    assert_eq!(
        field_path(&[PathStep::Key("itens".into()), PathStep::Row(0), PathStep::Key("preço".into())]),
        "itens[0].preço"
    );
}
