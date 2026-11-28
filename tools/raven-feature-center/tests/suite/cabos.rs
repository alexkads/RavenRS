//! The cables page through its interface: every use shows its cables, settles and sleeps, the
//! panels plug, unplug, retract and check like the real things, and the lab turns every knob.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use raven_feature_center::widgets::graficos::cabos::{CablesPage, USES};
use ravenrs::core::{AccessNode, Event, PointerButton};
use ravenrs::prelude::*;

fn opened() -> Ui {
    let mut ui = Ui::new(CablesPage, Theme::raven_dark());
    ui.set_viewport(Size::new(1100.0, 640.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui.update();
    ui
}

fn named(ui: &mut Ui, name: &str) -> AccessNode {
    ui.access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("{name}"))
}

/// A jack of a panel, not its label.
fn jack(ui: &mut Ui, name: &str) -> Point {
    let node = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name) && n.role == ravenrs::core::Role::Button)
        .cloned()
        .unwrap_or_else(|| panic!("jack {name}"));
    center(&node)
}

fn center(node: &AccessNode) -> Point {
    Point::new(node.bounds.x() + node.bounds.width() / 2.0, node.bounds.y() + node.bounds.height() / 2.0)
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn open_tab(ui: &mut Ui, name: &str) {
    let tab = named(ui, name);
    click(ui, center(&tab));
}

/// Runs the clock for `seconds` at 60 frames a second, from `from`.
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
        t = run(ui, t, 0.05);
    }
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    ui.update();
    t
}

#[test]
fn every_use_shows_its_cables_and_then_sleeps() {
    let mut ui = opened();
    let mut t = 0.0;
    for (name, cables) in USES.iter().zip([2, 3, 2, 1, 4, 3, 0, 1, 1]) {
        open_tab(&mut ui, name);
        t = run(&mut ui, t, 2.0);
        assert_eq!(ui.cables().ids().len(), cables, "{name}");
        assert_eq!(ui.next_wake(), None, "{name} settles and sleeps");
    }
}

#[test]
fn a_cable_pulled_out_of_a_jack_plugs_into_one_of_the_same_signal_and_a_click_unplugs_it() {
    let mut ui = opened();
    open_tab(&mut ui, "Rack");
    let t = run(&mut ui, 0.0, 1.0);
    assert_eq!(ui.cables().ids().len(), 2);
    let (from, to) = (jack(&mut ui, "Microfone"), jack(&mut ui, "Entrada 2"));
    let t = drag(&mut ui, from, to, t);
    let t = run(&mut ui, t, 1.5);
    assert_eq!(ui.cables().ids().len(), 3, "plugged in");
    // A click on the new cable unplugs it: it falls and is gone.
    let shapes: Vec<_> =
        ui.cables().ids().iter().map(|id| ui.cables().shape(*id).expect("a cable")).collect();
    let mine = shapes.iter().find(|s| (s[0].x - from.x).abs() < 2.0).expect("the microphone's cable");
    click(&mut ui, mine[mine.len() / 2]);
    let t = run(&mut ui, t, 0.1);
    assert!(ui.next_wake().is_some(), "it falls");
    run(&mut ui, t, 2.0);
    assert_eq!(ui.cables().ids().len(), 2, "unplugged");
}

#[test]
fn a_cable_of_another_signal_is_refused() {
    let mut ui = opened();
    open_tab(&mut ui, "Rack");
    let t = run(&mut ui, 0.0, 1.0);
    let (from, to) = (jack(&mut ui, "Clock"), jack(&mut ui, "Entrada 2"));
    let t = drag(&mut ui, from, to, t);
    run(&mut ui, t, 1.5);
    assert_eq!(ui.cables().ids().len(), 2, "a clock does not go into an audio input");
}

#[test]
fn the_switchboard_cord_goes_back_into_its_hole() {
    let mut ui = opened();
    open_tab(&mut ui, "Central");
    let t = run(&mut ui, 0.0, 1.0);
    assert_eq!(ui.cables().ids().len(), 1);
    let shape = ui.cables().shape(ui.cables().ids()[0]).expect("the cord");
    click(&mut ui, shape[shape.len() / 2]);
    let t = run(&mut ui, t, 0.1);
    assert!(ui.next_wake().is_some(), "it is pulled back");
    run(&mut ui, t, 2.0);
    assert!(ui.cables().ids().is_empty());
    assert_eq!(ui.next_wake(), None);
}

#[test]
fn a_click_on_a_network_cable_says_what_it_joins() {
    let mut ui = opened();
    open_tab(&mut ui, "Rede");
    run(&mut ui, 0.0, 1.5);
    let shapes: Vec<_> =
        ui.cables().ids().iter().map(|id| ui.cables().shape(*id).expect("a cable")).collect();
    let first = jack(&mut ui, "Switch 1");
    let cable = shapes.iter().find(|s| (s[0].x - first.x).abs() < 2.0).expect("switch 1's cable");
    click(&mut ui, cable[cable.len() / 2]);
    assert!(ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Switch 1 → Servidor A")));
}

#[test]
fn checking_the_columns_drops_the_wrong_answers() {
    let mut ui = opened();
    open_tab(&mut ui, "Colunas");
    let mut t = run(&mut ui, 0.0, 0.5);
    for (a, b) in [("Rio", "Brasil"), ("Lisboa", "Japão")] {
        let (from, to) = (jack(&mut ui, a), jack(&mut ui, b));
        t = drag(&mut ui, from, to, t);
    }
    t = run(&mut ui, t, 1.0);
    assert_eq!(ui.cables().ids().len(), 2);
    let check = center(&named(&mut ui, "Conferir"));
    click(&mut ui, check);
    run(&mut ui, t, 2.0);
    assert_eq!(ui.cables().ids().len(), 1, "Lisboa is not in Japan");
}

#[test]
fn the_tip_hangs_from_its_button_and_falls_when_hidden() {
    let mut ui = opened();
    open_tab(&mut ui, "Dica");
    let t = run(&mut ui, 0.0, 1.5);
    assert_eq!(ui.cables().ids().len(), 1);
    let toggle = center(&named(&mut ui, "Mostrar a dica"));
    click(&mut ui, toggle);
    let t = run(&mut ui, t, 0.1);
    assert!(ui.next_wake().is_some(), "it falls");
    run(&mut ui, t, 2.0);
    assert!(ui.cables().ids().is_empty());
}

/// Draws every use with the GPU world, as a window would, into PNGs in the folder that
/// `CABLE_SNAPSHOTS` names: `CABLE_SNAPSHOTS=/tmp/x cargo test --manifest-path tools/raven-feature-center/Cargo.toml --test cabos -- --ignored`.
#[test]
#[ignore = "writes pictures; run by hand"]
fn snapshots() {
    use ravenrs::render::{GpuCables, GpuContext, Renderer, render_with, wgpu};
    let Some(folder) = std::env::var_os("CABLE_SNAPSHOTS") else { return };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None)).expect("a GPU");
    let world = GpuCables::new(&gpu.device, &gpu.queue).expect("compute");
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer.set_cables(Some(world.drawer()));
    let mut ui = Ui::new(CablesPage, Theme::raven_dark());
    ui.set_cable_world(Box::new(world));
    ui.set_viewport(Size::new(1100.0, 640.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    let mut t = 0.0;
    let shoot = |ui: &mut Ui, renderer: &mut Renderer, name: &str| {
        let pixels = render_with(&gpu, renderer, ui.scene(), 1100, 640).expect("pixels");
        let path = std::path::Path::new(&folder).join(format!("{name}.png"));
        image::save_buffer(&path, &pixels, 1100, 640, image::ColorType::Rgba8).expect("saved");
    };
    for name in USES {
        open_tab(&mut ui, name);
        // Mid-drop, then settled.
        t = run(&mut ui, t, 0.12);
        shoot(&mut ui, &mut renderer, &format!("{name}-caindo"));
        t = run(&mut ui, t, 2.0);
        shoot(&mut ui, &mut renderer, name);
    }
}

#[test]
fn the_lab_turns_every_knob_and_a_cable_in_the_hand_stays_where_it_is_let_go() {
    let mut ui = opened();
    open_tab(&mut ui, "Laboratório");
    let mut t = run(&mut ui, 0.0, 0.5);
    assert_eq!(ui.cables().ids().len(), 1, "the cable of the knobs");
    for knob in
        ["Tracejado", "Mais pesado", "Preso no caminho", "Plugues coloridos", "Mais pontos", "Ponta na caixa"]
    {
        let at = center(&named(&mut ui, knob));
        click(&mut ui, at);
        t = run(&mut ui, t, 0.3);
        assert_eq!(ui.cables().ids().len(), 1, "{knob}");
    }
    t = run(&mut ui, t, 3.0);
    assert_eq!(ui.next_wake(), None, "with every knob on, it settles");
    // Let go with "Some ao sair": it is gone at once.
    let vanish = center(&named(&mut ui, "Some ao sair"));
    click(&mut ui, vanish);
    let off = center(&named(&mut ui, "Cabo de ajustes"));
    click(&mut ui, off);
    t = run(&mut ui, t, 0.05);
    assert!(ui.cables().ids().is_empty());
    // In the hand.
    let take = center(&named(&mut ui, "Pegar o cabo"));
    click(&mut ui, take);
    ui.handle_event(&Event::PointerMove { position: Point::new(500.0, 500.0) });
    t = run(&mut ui, t, 0.3);
    let id = ui.cables().ids()[0];
    let end = *ui.cables().shape(id).expect("in the hand").last().expect("an end");
    assert_eq!(end, Point::new(500.0, 500.0), "it follows the pointer");
    ui.handle_event(&Event::PointerUp { position: Point::new(500.0, 500.0), button: PointerButton::Primary });
    ui.update();
    ui.update();
    run(&mut ui, t, 0.3);
    let id = ui.cables().ids()[0];
    let end = *ui.cables().shape(id).expect("let go").last().expect("an end");
    assert_eq!(end, Point::new(500.0, 500.0), "and stays where it was let go");
}
