//! Text with expressions inside it, in a flow: a field of `set` written as `Olá {{ nome }}`, a
//! reference to a node that ran, a node the flow lacks marked where it is, and a preview that
//! tells a failure from an empty value.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::collections::BTreeMap;

use ravenrs_flows::expression::{self, Preview};
use ravenrs_flows::{Catalog, Executor, FlowDefinition, NodeDef, Plan};
use serde_json::{Map, json};

fn flow() -> FlowDefinition {
    FlowDefinition::new("f", "Saudação")
        .node(
            NodeDef::new("inicio", "manual").parameter("items", json!([{ "nome": "Ana", "loja": "Centro" }])),
        )
        .node(NodeDef::new("ola", "set").parameter(
            "fields",
            json!({ "mensagem": "Olá {{ nome }}, da loja {{ nodes.inicio.loja }}", "dobro": "2 * 2" }),
        ))
        .connect("inicio", "main", "ola", "main")
}

#[test]
fn a_field_of_set_written_as_text_with_expressions_reads_the_item_and_the_nodes_that_ran() {
    let catalog = Catalog::builtin();
    let flow = flow();
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).run();
    let out = &run.output("ola", "main")[0].json;
    assert_eq!(out["mensagem"], json!("Olá Ana, da loja Centro"));
    assert_eq!(out["dobro"], json!(4), "an expression alone still works out as before");
}

#[test]
fn a_reference_to_a_node_the_flow_lacks_is_marked_and_not_worked_out() {
    let flow = flow();
    let problems = expression::check("Total {{ nodes.fantasma.valor }} e {{ nodes.inicio.nome }}", &flow);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].at, 6..32);
    assert!(problems[0].message.contains("fantasma"));
    let record = json!({});
    assert!(matches!(
        expression::preview("{{ nodes.fantasma.x }}", &record, Some(&flow)),
        Preview::Problems(_)
    ));
}

#[test]
fn a_preview_works_out_the_text_and_never_shows_a_failure_as_empty() {
    let item = json!({ "nome": "Ana", "nada": null });
    let record = expression::record(&item, &BTreeMap::new(), &Map::new(), &expression::run_value(0));
    assert_eq!(expression::preview("Olá {{ item.nome }}", &record, None), Preview::Text("Olá Ana".into()));
    assert_eq!(
        expression::preview("[{{ nada }}]", &record, None),
        Preview::Text("[]".into()),
        "null is empty"
    );
    assert!(
        matches!(expression::preview("{{ Nope(1) }}", &record, None), Preview::Failed(_)),
        "a failure says so"
    );
    // Each piece fits, the two together do not: the preview stops between them and says why.
    let long =
        expression::preview("{{ PadLeft('a', 40000, 'b') }}{{ PadLeft('a', 40000, 'b') }}", &record, None);
    assert!(matches!(&long, Preview::Failed(why) if why.contains("limit")), "{long:?}");
}
