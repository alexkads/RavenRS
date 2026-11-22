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
    assert!(says(&mut ui, "→ cliente.nome"), "the field chosen stays chosen in the schema");
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

fn center_of(ui: &mut Ui, name: &str, role: Option<ravenrs_core::Role>) -> ravenrs_core::Point {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| {
            n.name.as_deref() == Some(name)
                && role.is_none_or(|r| n.role == r)
                && !matches!(n.role, ravenrs_core::Role::Label | ravenrs_core::Role::Group)
        })
        .expect(name)
        .bounds
        .center()
}

fn carry(ui: &mut Ui, from: ravenrs_core::Point, to: ravenrs_core::Point) -> Option<String> {
    use ravenrs_core::{Event, Point, PointerButton};
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: Point::new(from.x + 30.0, from.y + 10.0) });
    ui.update();
    ui.handle_event(&Event::PointerMove { position: to });
    ui.update();
    let said = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == ravenrs_core::Role::Status)
        .and_then(|n| n.name.clone());
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    for _ in 0..3 {
        ui.update();
    }
    said
}

#[test]
fn a_field_carried_to_a_parameter_becomes_its_expression_and_a_number_refuses_it() {
    let chosen = State::new(Some("cliente.nome".to_owned()));
    let values = State::new(ParamValues::new());
    let form = ParameterForm::new(
        vec![
            ParamField::new("mensagem", "Message", ParamKind::Text),
            ParamField::new("qtd", "Quantity", ParamKind::Number),
        ],
        values.binding(),
    );
    let mut ui = ui(VStack::new((
        DataInspector::new(ORDERS).mode(State::new(1_usize).binding()).selected(chosen.binding()),
        form,
    )));
    let tag = center_of(&mut ui, "Drag cliente.nome to a parameter", None);
    let number = center_of(&mut ui, "Quantity", None);
    let refused = carry(&mut ui, tag, number);
    assert_eq!(
        refused.as_deref(),
        Some("This field takes no field carried to it"),
        "a number refuses it, saying why"
    );
    let text = center_of(&mut ui, "Message", Some(ravenrs_core::Role::TextInput));
    let tag = center_of(&mut ui, "Drag cliente.nome to a parameter", None);
    carry(&mut ui, tag, text);
    press(&mut ui, "Apply");
    assert_eq!(values.get().get("mensagem"), Some(&json!("{{ [cliente.nome] }}")));
    assert_eq!(values.get().get("qtd"), None, "the number took nothing");

    press(&mut ui, "Copy as an expression");
    assert_eq!(ui.clipboard().read().as_deref(), Some("[cliente.nome]"), "the same, without a pointer");
}

#[test]
fn a_field_carried_to_an_expression_goes_in_at_the_caret() {
    let chosen = State::new(Some("total".to_owned()));
    let text = State::new("Iif(, 1, 0)".to_owned());
    let mut ui = ui(VStack::new((
        DataInspector::new(ORDERS).mode(State::new(1_usize).binding()).selected(chosen.binding()),
        ExpressionEditor::new(text.binding()).rows(2),
    )));
    let tag = center_of(&mut ui, "Drag total to a parameter", None);
    let to = ui
        .access_tree()
        .nodes
        .iter()
        .rfind(|n| n.role == ravenrs_core::Role::TextInput)
        .expect("the expression's text")
        .bounds
        .center();
    carry(&mut ui, tag, to);
    assert!(text.get().contains("[total]"), "the field went in: {}", text.get());
}
