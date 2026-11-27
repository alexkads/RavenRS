//! `PatchPanel` (#578): plug a cable from an output into an input of its signal, refuse the
//! wrong one, take a plug out to plug it elsewhere or let it go to unplug, all of it with the
//! keys too, pairs of cords, the front and the back, and no frames once nothing moves.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessNode, Event, Key, Modifiers, Point, PointerButton, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

type Log = Rc<RefCell<Vec<String>>>;

fn jacks() -> Vec<Jack> {
    vec![
        Jack::output("a", "Saída A").signal("áudio").at(60.0, 60.0),
        Jack::output("c", "Controle C").signal("controle").at(200.0, 60.0),
        Jack::input("x", "Entrada X").signal("áudio").at(60.0, 220.0),
        Jack::input("y", "Entrada Y").signal("áudio").at(200.0, 220.0),
        Jack::input("z", "Entrada Z").signal("controle").at(340.0, 220.0),
    ]
}

struct Rack {
    links: State<Vec<PatchLink>>,
    log: Log,
    front: bool,
}

impl View for Rack {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (plugged, unplugged) = (self.log.clone(), self.log.clone());
        let mut panel = PatchPanel::new(jacks(), self.links.binding())
            .size(420.0, 300.0)
            .on_connect(move |l| plugged.borrow_mut().push(format!("+{}>{}", l.from, l.to)))
            .on_disconnect(move |l| unplugged.borrow_mut().push(format!("-{}>{}", l.from, l.to)));
        if self.front {
            panel = panel.front(Text::new("Frente do amplificador"));
        }
        cx.child(&panel)
    }
}

fn rack(links: Vec<PatchLink>, front: bool) -> (Ui, State<Vec<PatchLink>>, Log) {
    let (links, log) = (State::new(links), Rc::new(RefCell::new(Vec::new())));
    // The panel speaks the language of the app; these tests read it in Portuguese.
    let rack = Rack { links: links.clone(), log: log.clone(), front };
    let mut ui = Ui::new(Localized::new(Labels::pt_br(), rack), Theme::raven_dark());
    ui.set_viewport(Size::new(600.0, 500.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui.update();
    (ui, links, log)
}

fn node(ui: &mut Ui, name: &str, role: Role) -> AccessNode {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name) && n.role == role)
        .cloned()
        .unwrap_or_else(|| panic!("{name}"))
}

fn jack(ui: &mut Ui, name: &str) -> Point {
    let n = node(ui, name, Role::Button);
    Point::new(n.bounds.x() + n.bounds.width() / 2.0, n.bounds.y() + n.bounds.height() / 2.0)
}

fn run(ui: &mut Ui, from: f64, seconds: f64) -> f64 {
    let mut t = from;
    while t < from + seconds {
        t += 1.0 / 60.0;
        ui.set_time(t);
        ui.update();
    }
    t
}

fn drag(ui: &mut Ui, from: Point, to: Point, t: f64) -> f64 {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.update();
    let mut t = t;
    for i in 1..=8 {
        let k = i as f32 / 8.0;
        ui.handle_event(&Event::PointerMove {
            position: Point::new(from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k),
        });
        t = run(ui, t, 0.03);
    }
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    run(ui, t, 0.05)
}

fn key(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
    ui.update();
}

fn focused(ui: &mut Ui) -> Option<String> {
    ui.access_tree().focused().and_then(|n| n.name.clone())
}

fn link(from: &str, to: &str) -> PatchLink {
    PatchLink::new(from, to)
}

#[test]
fn a_cable_from_an_output_to_an_input_of_its_signal_plugs_and_is_told() {
    let (mut ui, links, log) = rack(Vec::new(), false);
    let (a, x) = (jack(&mut ui, "Saída A"), jack(&mut ui, "Entrada X"));
    let t = drag(&mut ui, a, x, 0.0);
    assert_eq!(links.get(), vec![link("a", "x")]);
    assert_eq!(*log.borrow(), vec!["+a>x".to_owned()]);
    assert!(ui.next_wake().is_some() || ui.needs_redraw(), "the new cable swings");
    run(&mut ui, t, 3.0);
    assert_eq!(ui.cables().ids().len(), 1, "one cable, plugged");
    assert!(ui.next_wake().is_none() && !ui.needs_redraw(), "settled: no frames");
    let x = node(&mut ui, "Entrada X", Role::Button);
    assert_eq!(x.description.as_deref(), Some("entrada de áudio, ligada a Saída A"));
}

#[test]
fn another_signal_or_a_taken_input_refuses_and_the_cable_falls_away() {
    let (mut ui, links, log) = rack(vec![link("a", "x")], false);
    let (a, c, x, y) = (
        jack(&mut ui, "Saída A"),
        jack(&mut ui, "Controle C"),
        jack(&mut ui, "Entrada X"),
        jack(&mut ui, "Entrada Y"),
    );
    let t = drag(&mut ui, c, y, 0.0);
    assert_eq!(links.get(), vec![link("a", "x")], "control does not go into audio");
    let t = drag(&mut ui, a, x, t);
    assert_eq!(links.get(), vec![link("a", "x")], "one cable an input");
    assert!(log.borrow().is_empty());
    run(&mut ui, t, 3.0);
    assert_eq!(ui.cables().ids().len(), 1, "the refused cables fell and went");
}

#[test]
fn the_plug_of_an_input_goes_to_another_and_let_go_on_the_panel_unplugs() {
    let (mut ui, links, log) = rack(vec![link("a", "x")], false);
    let (x, y) = (jack(&mut ui, "Entrada X"), jack(&mut ui, "Entrada Y"));
    let t = drag(&mut ui, x, y, 0.0);
    assert_eq!(links.get(), vec![link("a", "y")]);
    assert_eq!(*log.borrow(), vec!["-a>x".to_owned(), "+a>y".to_owned()]);
    let empty = Point::new((x.x + y.x) / 2.0, x.y - 70.0);
    let t = drag(&mut ui, y, empty, t);
    assert!(links.get().is_empty(), "let go on the panel: unplugged");
    assert_eq!(log.borrow().last().map(String::as_str), Some("-a>y"));
    run(&mut ui, t, 3.0);
    assert!(ui.cables().ids().is_empty());
}

#[test]
fn the_keys_plug_unplug_and_undo() {
    let (mut ui, links, _) = rack(Vec::new(), false);
    key(&mut ui, Key::Tab, Modifiers::default());
    assert_eq!(focused(&mut ui).as_deref(), Some("Saída A"));
    key(&mut ui, Key::Enter, Modifiers::default());
    key(&mut ui, Key::ArrowDown, Modifiers::default());
    assert_eq!(focused(&mut ui).as_deref(), Some("Entrada X"), "the arrow goes to the jack that takes it");
    key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(links.get(), vec![link("a", "x")]);
    key(&mut ui, Key::Delete, Modifiers::default());
    assert!(links.get().is_empty(), "Delete unplugs the focused jack");
    key(&mut ui, Key::Character("z".into()), Modifiers { ctrl: true, ..Modifiers::default() });
    assert_eq!(links.get(), vec![link("a", "x")], "Ctrl+Z brings it back");
    key(&mut ui, Key::Character("z".into()), Modifiers { ctrl: true, shift: true, ..Modifiers::default() });
    assert!(links.get().is_empty(), "Ctrl+Shift+Z does it again");
}

#[test]
fn an_arrow_skips_the_jacks_that_refuse() {
    let (mut ui, _, _) = rack(Vec::new(), false);
    key(&mut ui, Key::Tab, Modifiers::default());
    key(&mut ui, Key::Tab, Modifiers::default());
    assert_eq!(focused(&mut ui).as_deref(), Some("Controle C"));
    key(&mut ui, Key::Enter, Modifiers::default());
    key(&mut ui, Key::ArrowDown, Modifiers::default());
    assert_eq!(focused(&mut ui).as_deref(), Some("Entrada Z"), "control only goes into control");
    key(&mut ui, Key::Escape, Modifiers::default());
}

#[test]
fn with_nothing_moving_no_frame_is_asked() {
    let (mut ui, _, _) = rack(vec![link("a", "x"), link("c", "z")], false);
    run(&mut ui, 0.0, 0.5);
    assert_eq!(ui.cables().ids().len(), 2);
    assert!(ui.next_wake().is_none() && !ui.needs_redraw(), "links already there show at rest");
}

#[test]
fn the_front_turns_around_and_the_cables_live_on_the_back() {
    let (mut ui, _, _) = rack(vec![link("a", "x")], true);
    run(&mut ui, 0.0, 0.2);
    assert_eq!(ui.cables().ids().len(), 1);
    let turn = node(&mut ui, "Virar", Role::Button);
    let at = Point::new(turn.bounds.x() + 4.0, turn.bounds.y() + 4.0);
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    let t = run(&mut ui, 0.2, 2.0);
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Frente do amplificador")));
    assert!(ui.cables().ids().is_empty(), "no cables on the front");
    let back = node(&mut ui, "Ver a traseira", Role::Button);
    let at = Point::new(back.bounds.x() + 4.0, back.bounds.y() + 4.0);
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    run(&mut ui, t, 0.2);
    assert_eq!(ui.cables().ids().len(), 1, "back again, with its cable");
}

/// A switchboard: two cords that answer and call, and the extensions they join.
struct Board {
    links: State<Vec<PatchLink>>,
    joined: Log,
}

impl View for Board {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let joined = self.joined.clone();
        let jacks = vec![
            Jack::input("21", "Ramal 21").at(60.0, 40.0),
            Jack::input("25", "Ramal 25").at(300.0, 40.0),
            Jack::output("atender", "Atender").at(140.0, 240.0),
            Jack::output("chamar", "Chamar").at(220.0, 240.0),
        ];
        cx.child(
            &PatchPanel::new(jacks, self.links.binding())
                .size(360.0, 300.0)
                .retract(true)
                .pair("atender", "chamar")
                .on_join(move |a, b| joined.borrow_mut().push(format!("{a}+{b}"))),
        )
    }
}

#[test]
fn a_pair_of_cords_joins_two_extensions_and_a_cord_let_go_goes_home() {
    let (links, joined) = (State::new(Vec::new()), Rc::new(RefCell::new(Vec::new())));
    let board = Board { links: links.clone(), joined: joined.clone() };
    let mut ui = Ui::new(Localized::new(Labels::pt_br(), board), Theme::raven_dark());
    ui.set_viewport(Size::new(500.0, 400.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui.update();
    let (answer, call) = (jack(&mut ui, "Atender"), jack(&mut ui, "Chamar"));
    let (r21, r25) = (jack(&mut ui, "Ramal 21"), jack(&mut ui, "Ramal 25"));
    let t = drag(&mut ui, answer, r21, 0.0);
    assert!(joined.borrow().is_empty(), "one cord is not a call yet");
    let t = drag(&mut ui, call, r25, t);
    assert_eq!(*joined.borrow(), vec!["21+25".to_owned()]);
    // Taken out of 25 and let go on the board: it goes back to its hole.
    let t = drag(&mut ui, r25, Point::new(r25.x - 60.0, r25.y + 80.0), t);
    assert_eq!(links.get(), vec![link("atender", "21")]);
    run(&mut ui, t, 3.0);
    assert_eq!(ui.cables().ids().len(), 1, "the cord went home and is gone from sight");
}
