//! The design board through its interface: frames holding their pieces, a piece dragged in,
//! and the board saved as a drawing.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs::core::{AccessNode, Event, PointerButton};
use ravenrs::prelude::*;

fn opened() -> Ui {
    let mut ui = Ui::new(flow_design_board::app(), Theme::raven_dark());
    ui.set_viewport(Size::new(1280.0, 800.0), 1.0);
    ui.update();
    ui.update();
    ui
}

fn named(ui: &mut Ui, name: &str) -> Vec<AccessNode> {
    ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some(name)).cloned().collect()
}

#[test]
fn frames_hold_their_pieces_and_a_piece_turns() {
    let graph = flow_design_board::sample();
    let inside: Vec<&str> = graph.children("login").map(|n| n.id.as_str()).collect();
    assert!(inside.contains(&"enter") && inside.contains(&"logo"));
    assert!(graph.find_node("contrast").is_some_and(|n| n.parent.is_none()), "the note sits outside");
    assert!(graph.find_node("badge").is_some_and(|n| n.rotation != 0.0));
}

#[test]
fn a_piece_dragged_in_lands_on_the_board() {
    let mut ui = opened();
    let before = named(&mut ui, "Elipse").len();
    let tile = named(&mut ui, "Elipse").first().expect("the tile").bounds.center();
    let board = named(&mut ui, "Quadro").first().expect("the board").bounds;
    let to = Point::new(board.x() + 80.0, board.max_y() - 120.0);
    ui.handle_event(&Event::PointerMove { position: tile });
    ui.handle_event(&Event::PointerDown { position: tile, button: PointerButton::Primary });
    ui.update();
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        let at = Point::new(tile.x + (to.x - tile.x) * t, tile.y + (to.y - tile.y) * t);
        ui.handle_event(&Event::PointerMove { position: at });
        ui.update();
    }
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    ui.update();
    assert_eq!(named(&mut ui, "Elipse").len(), before + 2, "the new piece, and its row in the layers");
}

#[test]
fn the_board_is_saved_as_a_drawing() {
    let picture = flow_design_board::sample().picture(None, FlowColors::paper()).expect("a picture");
    let file = ravenrs_flow_export::export(&picture, FlowExportFormat::Svg, "Quadro").expect("an svg");
    let svg = String::from_utf8(file.bytes).expect("text");
    assert!(svg.contains("Cabeçalho") && svg.contains("stroke-dasharray"), "pieces and frames");
}

#[test]
fn layers_pick_hide_and_reorder_the_pieces() {
    let mut ui = opened();
    assert!(!named(&mut ui, "Camadas").is_empty());
    assert!(!named(&mut ui, "Logo").is_empty());
    let press = |ui: &mut Ui, name: &str| {
        let id = named(ui, name).first().unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, ravenrs::core::AccessAction::Click);
        ui.update();
        ui.update();
    };
    let logos = named(&mut ui, "Logo").len();
    press(&mut ui, "Ocultar Logo");
    assert!(!named(&mut ui, "Mostrar Logo").is_empty(), "the row offers it back");
    assert_eq!(named(&mut ui, "Logo").len(), logos - 1, "the piece left the board, its row stays");
    press(&mut ui, "Mostrar Logo");
    assert_eq!(named(&mut ui, "Logo").len(), logos);
    press(&mut ui, "Trazer Logo para frente");
    assert!(!named(&mut ui, "Enviar Logo para trás").is_empty());
}
