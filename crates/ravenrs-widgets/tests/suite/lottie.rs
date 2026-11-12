//! `Lottie`: plays on the UI clock, sleeps when it ends, and respects reduced motion.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{PathElement, PathPaint, Primitive, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

/// A second of a square that changes from nothing to all of its width.
const GROWING: &str = r#"{"v":"5.7.0","fr":10,"ip":0,"op":10,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":10,"st":0,"shapes":[{"ty":"rc","p":{"a":0,"k":[50,50]},"s":{"a":1,"k":[
{"t":0,"s":[0,100],"o":{"x":[0],"y":[0]},"i":{"x":[1],"y":[1]}},{"t":10,"s":[100,100]}]},"r":{"a":0,"k":0}},
{"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":100}}]}]}"#;

fn growing() -> LottieAnimation {
    LottieAnimation::from_json(GROWING.as_bytes()).expect("a Lottie")
}

fn running(root: impl View + 'static, reduced: bool) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_light());
    ui.set_reduced_motion(reduced);
    ui.set_viewport(Size::new(200.0, 200.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

/// How wide the square is on the 100-point comp, measured from the outline.
fn width_drawn(ui: &Ui) -> f32 {
    let mut xs = Vec::new();
    for primitive in ui.scene().primitives() {
        if let Primitive::Path(path) = primitive {
            assert!(matches!(path.paint, PathPaint::Solid(_)));
            for element in path.elements.iter() {
                match element {
                    PathElement::MoveTo(p) | PathElement::LineTo(p) | PathElement::CubicTo(_, _, p) => {
                        xs.push(p.x);
                    }
                    _ => {}
                }
            }
        }
    }
    let high = xs.iter().copied().fold(f32::MIN, f32::max);
    let low = xs.iter().copied().fold(f32::MAX, f32::min);
    if xs.is_empty() { 0.0 } else { high - low }
}

fn at(ui: &mut Ui, time: f64) {
    ui.set_time(time);
    ui.update();
    // The clock reaches the paint on the frame after the state it wrote.
    ui.update();
}

#[test]
fn it_plays_on_the_clock_and_sleeps_on_its_last_frame() {
    let mut ui = running(Lottie::new(growing()).size(100.0, 100.0).looping(false), false);
    assert!(ui.next_wake().is_some(), "it asks for its next frame");
    at(&mut ui, 0.5);
    assert!((width_drawn(&ui) - 50.0).abs() < 1.0, "half way: {}", width_drawn(&ui));
    at(&mut ui, 1.0);
    at(&mut ui, 5.0);
    // Its last frame is the ninth of ten, and it does not vanish past it.
    assert!((width_drawn(&ui) - 90.0).abs() < 1.0, "stays on the end: {}", width_drawn(&ui));
    assert_eq!(ui.next_wake(), None, "an idle window stays idle");
}

#[test]
fn looping_starts_over_and_keeps_asking_for_frames() {
    let mut ui = running(Lottie::new(growing()).size(100.0, 100.0), false);
    at(&mut ui, 1.25);
    assert!((width_drawn(&ui) - 25.0).abs() < 1.0, "a quarter into the second turn: {}", width_drawn(&ui));
    assert!(ui.next_wake().is_some());
}

#[test]
fn a_binding_pauses_it_where_it_is() {
    let playing = State::new(true);
    let mut ui = running(Lottie::new(growing()).size(100.0, 100.0).playing(playing.binding()), false);
    at(&mut ui, 0.5);
    playing.set(false);
    ui.update();
    at(&mut ui, 0.9);
    assert!((width_drawn(&ui) - 50.0).abs() < 1.0, "held: {}", width_drawn(&ui));
    assert_eq!(ui.next_wake(), None, "and asks for nothing while paused");
    playing.set(true);
    at(&mut ui, 0.9);
    at(&mut ui, 1.1);
    assert!((width_drawn(&ui) - 70.0).abs() < 1.0, "goes on from there: {}", width_drawn(&ui));
}

#[test]
fn where_motion_is_reduced_it_shows_its_last_frame_still() {
    let mut ui = running(Lottie::new(growing()).size(100.0, 100.0), true);
    ui.update();
    assert!((width_drawn(&ui) - 90.0).abs() < 1.0, "the last frame: {}", width_drawn(&ui));
    assert_eq!(ui.next_wake(), None);
}

#[test]
fn it_scales_inside_its_size_and_keeps_its_proportions() {
    let mut ui = running(Lottie::new(growing()).size(200.0, 50.0).speed(100.0).looping(false), false);
    at(&mut ui, 1.0);
    let transform = ui.scene().primitives().iter().find_map(|p| match p {
        Primitive::Path(path) => Some(path.transform),
        _ => None,
    });
    let transform = transform.expect("a shape");
    assert!((transform[0] - 0.5).abs() < 1e-3 && (transform[3] - 0.5).abs() < 1e-3, "{transform:?}");
    assert!((transform[4] - 75.0).abs() < 1.0, "centered: {transform:?}");
}

#[test]
fn screen_readers_skip_it_unless_it_has_a_name() {
    let mut ui = running(Lottie::new(growing()).size(100.0, 100.0), false);
    assert!(!ui.access_tree().nodes.iter().any(|n| n.role == Role::Image));
    let mut named = running(Lottie::new(growing()).size(100.0, 100.0).alt("A square"), false);
    let tree = named.access_tree();
    assert!(tree.nodes.iter().any(|n| n.role == Role::Image && n.name.as_deref() == Some("A square")));
}
