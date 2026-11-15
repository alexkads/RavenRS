//! `CablePlug`: cables between widgets, in the hand, clicked, dropped and at rest.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Event, Point, PointerButton, Primitive, Size};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::cable::CableTexture;
use ravenrs_widgets::*;

/// Two boxes and, when `tied`, a cable between them; the second box sits `gap` lower.
struct Board {
    tied: State<bool>,
    gap: State<f32>,
    loose: bool,
    clicked: Rc<Cell<u32>>,
    dropped: Rc<Cell<Option<Point>>>,
}

impl View for Board {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let boxed = |cx: &mut BuildCx<'_>, x: f32, y: f32| {
            let style = LayoutStyle {
                absolute: Some((x, y)),
                width: Some(80.0),
                height: Some(30.0),
                ..LayoutStyle::leaf()
            };
            cx.container(style, &[], Paint::None)
        };
        let a = boxed(cx, 40.0, 40.0);
        let b = boxed(cx, 420.0, 40.0 + self.gap.get());
        if self.tied.get() {
            let clicked = self.clicked.clone();
            let mut plug = CablePlug::between(a, b)
                .sides(CableSide::Right, CableSide::Left)
                .texture(CableTexture::Braided)
                .plugs(true)
                .key(1)
                .on_click(move || clicked.set(clicked.get() + 1));
            if self.loose {
                let dropped = self.dropped.clone();
                plug = plug.loose().on_drop(move |p| dropped.set(Some(p)));
            }
            plug.build(cx);
        }
        let style =
            LayoutStyle { width: Some(900.0), height: Some(600.0), ..LayoutStyle::stack(Axis::Vertical) };
        cx.container(style, &[a, b], Paint::None)
    }
}

/// The window, whether it is tied, the gap, the clicks and where a cable was let go.
type Rigged = (Ui, State<bool>, State<f32>, Rc<Cell<u32>>, Rc<Cell<Option<Point>>>);

fn board(tied: bool, loose: bool) -> Rigged {
    let (tied, gap) = (State::new(tied), State::new(0.0));
    let (clicked, dropped) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(None)));
    let view = Board {
        tied: tied.clone(),
        gap: gap.clone(),
        loose,
        clicked: clicked.clone(),
        dropped: dropped.clone(),
    };
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 600.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    (ui, tied, gap, clicked, dropped)
}

/// Runs the clock for `seconds` at 60 frames a second.
fn run(ui: &mut Ui, from: f64, seconds: f64) -> f64 {
    let mut t = from;
    let end = from + seconds;
    while t < end {
        t += 1.0 / 60.0;
        ui.set_time(t);
        ui.update();
    }
    t
}

fn plugs(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Quad(_))).count()
}

#[test]
fn a_cable_between_two_widgets_drops_settles_and_then_asks_for_no_frames() {
    let (mut ui, _, _, _, _) = board(true, false);
    assert!(ui.next_wake().is_some(), "a new cable drops, so the window paints");
    assert!(
        !ui.scene().primitives().iter().any(|p| matches!(p, Primitive::Cable(_))),
        "drawn as shapes on the CPU"
    );
    let shape = ui.cables().shape(cable_id(&ui)).expect("placed");
    assert_eq!(shape.len(), 16);
    let t = run(&mut ui, 0.0, 1.5);
    assert_eq!(ui.next_wake(), None, "settled, it sleeps");
    assert!(!ui.needs_redraw());
    let rest = ui.cables().shape(cable_id(&ui)).unwrap();
    let lowest = rest.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(lowest > rest[0].y + 20.0, "it hangs");
    let _ = t;
}

fn cable_id(ui: &Ui) -> u64 {
    ui.cables().ids().first().copied().expect("one cable")
}

#[test]
fn moving_what_it_ties_makes_it_swing_and_settle_again() {
    let (mut ui, _, gap, _, _) = board(true, false);
    let t = run(&mut ui, 0.0, 1.5);
    assert_eq!(ui.next_wake(), None);
    gap.set(120.0);
    ui.update();
    let t = run(&mut ui, t, 0.1);
    assert!(ui.next_wake().is_some() || ui.needs_redraw(), "it swings");
    run(&mut ui, t, 2.0);
    assert_eq!(ui.next_wake(), None, "and settles");
}

#[test]
fn reduced_motion_shows_it_at_rest_without_frames() {
    let (mut ui, tied, _, _, _) = board(false, false);
    ui.set_reduced_motion(true);
    tied.set(true);
    ui.update();
    assert_eq!(ui.next_wake(), None);
    assert!(ui.cables().shape(cable_id(&ui)).is_some());
}

#[test]
fn the_plugs_show_and_a_click_on_the_cable_calls_its_handler() {
    let (mut ui, _, _, clicked, _) = board(true, false);
    run(&mut ui, 0.0, 1.5);
    assert_eq!(plugs(&ui), 4, "a body and a tip at each end");
    let shape = ui.cables().shape(cable_id(&ui)).unwrap();
    let middle = shape[shape.len() / 2];
    ui.handle_event(&Event::PointerDown { position: middle, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: middle, button: PointerButton::Primary });
    assert_eq!(clicked.get(), 1);
}

#[test]
fn a_cable_in_the_hand_follows_the_pointer_and_tells_where_it_was_let_go() {
    let (mut ui, tied, _, _, dropped) = board(true, true);
    ui.handle_event(&Event::PointerMove { position: Point::new(500.0, 400.0) });
    let t = run(&mut ui, 0.0, 0.5);
    let shape = ui.cables().shape(cable_id(&ui)).unwrap();
    let end = shape[shape.len() - 1];
    assert_eq!(end, Point::new(500.0, 400.0), "its end is in the hand");
    ui.handle_event(&Event::PointerUp { position: Point::new(500.0, 400.0), button: PointerButton::Primary });
    assert_eq!(dropped.get(), Some(Point::new(500.0, 400.0)));
    tied.set(false);
    ui.update();
    let t = run(&mut ui, t, 0.2);
    assert!(ui.next_wake().is_some(), "let go, it falls");
    run(&mut ui, t, 2.0);
    assert_eq!(ui.next_wake(), None, "and is gone");
    assert!(ui.cables().ids().is_empty());
}
