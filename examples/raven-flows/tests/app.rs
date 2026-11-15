//! Raven Flows through its screens and its folder, with no window: the flows kept as files,
//! a run drawn on the diagram, an approval that lets a run go on, a run repeated and the news.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::path::PathBuf;

use raven_flows::{Casa, Worker, casa, marcas};
use ravenrs::core::AccessAction;
use ravenrs::prelude::*;
use ravenrs_flows::{Catalog, FlowDefinition, NodeDef};
use ravenrs_flows_runtime::RunStatus;
use serde_json::json;

/// A folder of its own for each test, empty when it starts.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("raven-flows-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn opened(dir: &PathBuf) -> Ui {
    let app = raven_flows::app_in(dir, Worker::Inline).expect("the app opens");
    let mut ui = Ui::new(app, Theme::raven_dark());
    ui.set_viewport(Size::new(1440.0, 900.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn has(ui: &mut Ui, name: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(name))
}

fn press(ui: &mut Ui, name: &str) {
    let id = ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
    ui.update();
}

#[test]
fn the_first_opening_lists_the_sample_flow_kept_as_a_file() {
    let dir = folder("first");
    let mut ui = opened(&dir);
    assert!(has(&mut ui, "Conferir pedidos"), "the sample is in the list");
    assert!(dir.join("fluxos").join("conferir-pedidos.json").exists(), "kept as a file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn running_the_sample_marks_each_node_and_counts_the_items_on_each_connection() {
    let dir = folder("marks");
    let casa = Casa::open(&dir, Worker::Inline).expect("opens");
    let flow = casa.flow("conferir-pedidos").expect("the sample");
    let run = casa.run(&flow).expect("asked for");
    assert_eq!(casa.run_record(&run).map(|r| r.status), Some(RunStatus::Waiting), "it stops at the approval");

    let graph = marcas::marked(&flow.to_graph(&Catalog::builtin()), &casa.parts(&run));
    let status = |id: &str| graph.find_node(id).map(|n| n.status.clone());
    assert_eq!(status("inicio"), Some(FlowNodeStatus::Success));
    assert_eq!(status("grandes"), Some(FlowNodeStatus::Success));
    assert_eq!(status("aprovar"), Some(FlowNodeStatus::Warning("Aguardando".into())));
    assert_eq!(status("aprovado"), Some(FlowNodeStatus::Normal), "not reached yet");
    let label = |from: &str, port: &str| {
        graph.edges.iter().find(|e| e.from.node == from && e.from.port == port).and_then(|e| e.label.clone())
    };
    assert_eq!(label("inicio", "main").as_deref(), Some("4 itens"));
    assert_eq!(label("pago", "true").as_deref(), Some("3 itens"));
    assert_eq!(label("pago", "false").as_deref(), Some("1 item"));
    assert_eq!(label("grandes", "main").as_deref(), Some("2 itens"), "Bia and Davi; Ana is small");
    assert_eq!(label("aprovar", "main"), None, "nothing went on yet");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_no_item_takes_keeps_its_plain_look() {
    let dir = folder("branch");
    let casa = Casa::open(&dir, Worker::Inline).expect("opens");
    let mut flow = FlowDefinition::new("todos-pagos", "Todos pagos")
        .node(NodeDef::new("inicio", "manual").parameter("items", json!([{ "pago": true }])))
        .node(NodeDef::new("pago", "if").parameter("condition", json!("[pago] = true")))
        .node(NodeDef::new("cobrar", "set").parameter("fields", json!({ "x": "1" })))
        .connect("inicio", "main", "pago", "main")
        .connect("pago", "false", "cobrar", "main");
    casa.save(&mut flow).expect("saved");
    let run = casa.run(&flow).expect("ran");
    let graph = marcas::marked(&flow.to_graph(&Catalog::builtin()), &casa.parts(&run));
    assert_eq!(graph.find_node("cobrar").map(|n| n.status.clone()), Some(FlowNodeStatus::Normal));
    assert_eq!(graph.edges[1].label.as_deref(), Some("0 itens"), "the empty side says so");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn approving_a_run_that_waits_lets_it_finish_and_repeating_makes_another() {
    let dir = folder("approve");
    let casa = Casa::open(&dir, Worker::Inline).expect("opens");
    let flow = casa.flow("conferir-pedidos").expect("the sample");
    let run = casa.run(&flow).expect("asked for");
    assert_eq!(casa.waiting_for(&run).as_deref(), Some("aprovacao"));
    casa.approve(&run).expect("approved");
    assert_eq!(casa.run_record(&run).map(|r| r.status), Some(RunStatus::Finished));
    let parts = casa.parts(&run);
    let approved = &parts["aprovado"].outputs["main"];
    assert_eq!(approved.len(), 2, "both big orders are approved");
    assert_eq!(approved[0].json["situacao"], json!("aprovado"));
    assert_eq!(approved[0].json["aprovacao"]["aprovado_por"], json!("Raven Flows"), "with who approved");
    assert!(casa.approve(&run).is_err(), "nothing waits any more");

    let again = casa.repeat(&run).expect("repeated");
    assert_ne!(again, run);
    assert_eq!(casa.runs().len(), 2);
    assert_eq!(casa.run_record(&again).and_then(|r| r.origin).as_deref(), Some(run.as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parameters_that_are_not_a_json_object_are_refused_and_good_ones_are_kept() {
    let flow = casa::sample();
    let mut graph = flow.to_graph(&Catalog::builtin());
    let before = graph.clone();
    let refused = raven_flows::apply_parameters(&mut graph, "pago", "{ condition: ").expect_err("not JSON");
    assert!(refused.starts_with("JSON inválido"), "{refused}");
    assert!(raven_flows::apply_parameters(&mut graph, "pago", "[1]").is_err(), "not an object");
    assert_eq!(graph, before, "a refusal changes nothing");

    raven_flows::apply_parameters(&mut graph, "pago", r#"{ "condition": "[valor] > 50" }"#).expect("kept");
    let read = FlowDefinition::from_graph(&graph, &Catalog::builtin());
    let pago = read.nodes.iter().find(|n| n.id == "pago").expect("the node");
    assert_eq!(pago.parameters["condition"], json!("[valor] > 50"));
}

#[test]
fn flows_and_runs_are_there_when_the_app_opens_again() {
    let dir = folder("again");
    {
        let casa = Casa::open(&dir, Worker::Inline).expect("opens");
        let mut flow = casa.create("Relatório do mês").expect("made");
        assert_eq!(flow.id, "relatorio-do-mes");
        casa.save(&mut flow).expect("saved again");
        casa.run(&flow).expect("ran");
    }
    let casa = Casa::open(&dir, Worker::Inline).expect("opens again");
    let names: Vec<String> = casa.flows().into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["Conferir pedidos", "Relatório do mês"]);
    assert_eq!(casa.flow("relatorio-do-mes").map(|f| f.revision), Some(2));
    assert_eq!(casa.runs().len(), 1);
    assert_eq!(casa.create("Relatório do mês").map(|f| f.id).as_deref(), Ok("relatorio-do-mes-2"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_editor_runs_the_flow_and_runs_approve_it_from_the_screens() {
    let dir = folder("screens");
    let mut ui = opened(&dir);
    press(&mut ui, "Conferir pedidos");
    press(&mut ui, "Abrir");
    assert!(has(&mut ui, "Área do fluxo"), "the editor is open");
    assert!(has(&mut ui, "Nós"), "with the toolbox");
    press(&mut ui, "Executar fluxo");
    assert!(has(&mut ui, "Última execução: Aguardando"), "the run stopped at the approval");

    press(&mut ui, "Execuções");
    assert!(has(&mut ui, "Aguardando"), "the run is in the list");
    press(&mut ui, "Aguardando");
    press(&mut ui, "Aprovar e continuar");
    assert!(has(&mut ui, "Concluída"), "it went on to the end");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_news_of_this_version_say_who_made_it_and_which_issues_it_closes() {
    let news: serde_json::Value = serde_json::from_str(raven_flows::NEWS).expect("JSON");
    let history: Vec<serde_json::Value> = serde_json::from_str(raven_flows::HISTORY).expect("JSON");
    assert_eq!(news["versao"], json!(raven_flows::VERSION), "the news are of this version");
    assert_eq!(history[0]["versao"], news["versao"], "and on top of the history");
    assert!(news["contribuidores"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(news["issues"].as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(raven_flows::release_notes().len(), history.len());
}
