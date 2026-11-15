//! The chat automation builder through its interface: blocks dragged in, rules on what joins
//! what, and the conversation saved as a file.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs::core::{AccessNode, Event, PointerButton};
use ravenrs::prelude::*;

fn opened() -> Ui {
    let mut ui = Ui::new(flow_chat_bot::app(), Theme::raven_dark());
    ui.set_viewport(Size::new(1280.0, 800.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn named(ui: &mut Ui, name: &str) -> Option<AccessNode> {
    ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).cloned()
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.update();
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        ui.handle_event(&Event::PointerMove {
            position: Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t),
        });
        ui.update();
    }
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

#[test]
fn the_builder_shows_blocks_conversation_and_properties() {
    let mut ui = opened();
    assert!(named(&mut ui, "Blocos").is_some(), "the blocks");
    assert!(named(&mut ui, "Conversa").is_some(), "the conversation");
    assert!(named(&mut ui, "Boas-vindas").is_some(), "a message");
    assert!(named(&mut ui, "Export").is_some(), "saving it");
}

#[test]
fn a_block_dragged_in_joins_the_conversation() {
    let mut ui = opened();
    let tile = named(&mut ui, "Novo contato").expect("a trigger to drag").bounds.center();
    let canvas = named(&mut ui, "Conversa").expect("the conversation").bounds;
    let before = ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some("Novo contato")).count();
    drag(&mut ui, tile, Point::new(canvas.x() + 120.0, canvas.y() + 120.0));
    let after = ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some("Novo contato")).count();
    assert_eq!(after, before + 1, "the block is on the canvas, besides its tile");
}

#[test]
fn nothing_goes_into_a_trigger_and_an_action_ends_the_way() {
    let graph = flow_chat_bot::sample();
    let end = |node: &str, port: &str| FlowEndpoint::new(node, port);
    assert!(flow_chat_bot::validate(&graph, &end("menu", "pedidos"), &end("start", "in")).is_err());
    assert!(flow_chat_bot::validate(&graph, &end("human", "out"), &end("hours", "in")).is_err());
    assert!(flow_chat_bot::validate(&graph, &end("menu", "pedidos"), &end("hours", "in")).is_ok());
    let action = graph.find_node("human").expect("an action");
    assert!(flow_chat_bot::next(action, PortSide::Right).is_none());
    let message = graph.find_node("hours").expect("a message");
    assert_eq!(flow_chat_bot::next(message, PortSide::Right).map(|n| n.kind), Some("message".into()));
}

#[test]
fn every_answer_of_a_question_leads_its_own_way_and_is_saved_with_its_name() {
    let graph = flow_chat_bot::sample();
    let menu = graph.find_node("menu").expect("the welcome");
    let outs = menu.ports.iter().filter(|p| p.direction == PortDirection::Output).count();
    assert_eq!(outs, 3, "one way out per answer");
    let picture = graph.picture(None, FlowColors::paper()).expect("a picture");
    assert!(picture.texts.iter().any(|t| t.text == "Atendente"), "the answers' names are drawn");
    let file = ravenrs_flow_export::export(&picture, FlowExportFormat::Png, "Conversa").expect("a png");
    assert_eq!(file.name, "Conversa.png");
}

#[test]
fn a_test_run_marks_the_way_taken_a_step_at_a_time() {
    let graph = flow_chat_bot::sample();
    let path = flow_chat_bot::run_path(&graph);
    let ids: Vec<&str> = path.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["start", "menu", "orders"], "the first way out each time");
    let at_menu = flow_chat_bot::marked(&graph, &path, Some(1));
    let status = |g: &FlowGraph, id: &str| g.find_node(id).map(|n| n.status.clone());
    assert_eq!(status(&at_menu, "start"), Some(FlowNodeStatus::Success));
    assert_eq!(status(&at_menu, "menu"), Some(FlowNodeStatus::Running));
    assert_eq!(status(&at_menu, "orders"), Some(FlowNodeStatus::Normal));
    assert!(at_menu.find_edge("e1").is_some_and(|e| e.animated), "the way taken moves");
    let back = flow_chat_bot::marked(&at_menu, &path, None);
    assert_eq!(back, graph, "stopping puts everything back");
}

#[test]
fn the_test_buttons_run_and_stop() {
    let mut ui = opened();
    let press = |ui: &mut Ui, name: &str| {
        let id = ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).id;
        ui.perform_action(id, ravenrs::core::AccessAction::Click);
        ui.update();
        ui.update();
    };
    press(&mut ui, "Testar conversa");
    press(&mut ui, "Próximo passo");
    let tree = ui.access_tree();
    let menu = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Boas-vindas")).expect("the welcome");
    assert!(menu.description.as_deref().is_some_and(|d| d.contains("Running")), "{:?}", menu.description);
    press(&mut ui, "Parar");
    let tree = ui.access_tree();
    let menu = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Boas-vindas")).expect("the welcome");
    assert!(!menu.description.as_deref().unwrap_or_default().contains("Running"));
}
