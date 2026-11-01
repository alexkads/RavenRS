//! The runtime's motion guarantees: what animates, what it costs, and that it stops.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_animation::Transition;
use ravenrs_core::{Brush, Color, Effect, Event, Point, PointerButton, Primitive, Quad, Size};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

const FRAME: f64 = 1.0 / 60.0;

fn running(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

/// Advances `n` frames from time `t`.
fn frames(ui: &mut Ui, t: &mut f64, n: u32) {
    for _ in 0..n {
        *t += FRAME;
        ui.set_time(*t);
        ui.update();
    }
}

fn settle(ui: &mut Ui, t: &mut f64) {
    for _ in 0..600 {
        ui.update();
        if !ui.is_animating() {
            return;
        }
        frames(ui, t, 1);
    }
    panic!("animations never finished");
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

fn fill(q: &Quad) -> Color {
    match q.fill {
        Brush::Solid(c) => c,
        _ => Color::TRANSPARENT,
    }
}

fn hover(ui: &mut Ui, p: Point) {
    ui.handle_event(&Event::PointerMove { position: p });
}

fn click(ui: &mut Ui, p: Point) {
    ui.handle_event(&Event::PointerMove { position: p });
    ui.handle_event(&Event::PointerDown { position: p, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: p, button: PointerButton::Primary });
}

/// Counts how many times it is built.
struct Counting<V>(V, Rc<Cell<u32>>);

impl<V: View> View for Counting<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.1.set(self.1.get() + 1);
        self.0.build(cx)
    }
}

#[test]
fn an_idle_app_asks_for_no_frames() {
    let mut ui = running(Button::new("Save"));
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    assert!(!ui.needs_redraw());
    assert_eq!(ui.next_wake(), None);
    t += 100.0;
    ui.set_time(t);
    assert!(!ui.needs_redraw(), "time passing alone changes nothing");
}

#[test]
fn hover_eases_the_color_and_ends_exactly_on_the_token() {
    let theme = Theme::raven_dark();
    let mut ui = running(Button::new("Save"));
    let mut t = 0.0;
    let rest = theme.color(Semantic::Accent);
    let hovered = theme.color(Semantic::AccentHover);
    let button = quads(&ui).into_iter().find(|q| fill(q) == rest).expect("button").rect;
    hover(&mut ui, button.center());
    ui.update();
    assert!(ui.is_animating());
    let start = fill(&quads(&ui)[0]);
    assert!((start.r - rest.r).abs() + (start.g - rest.g).abs() < 0.01, "starts from where it was");
    frames(&mut ui, &mut t, 3);
    let mid = fill(&quads(&ui)[0]);
    assert_ne!(mid, rest);
    assert_ne!(mid, hovered);
    settle(&mut ui, &mut t);
    assert_eq!(fill(&quads(&ui)[0]), hovered, "no rounding error at rest");
    assert!(!ui.needs_redraw());
}

#[test]
fn animation_frames_do_not_rebuild_the_tree() {
    let builds = Rc::new(Cell::new(0));
    let mut ui = running(Counting(Button::new("Save"), builds.clone()));
    let mut t = 0.0;
    let button = quads(&ui)[0].rect;
    hover(&mut ui, button.center());
    ui.update();
    let after_hover = builds.get();
    frames(&mut ui, &mut t, 4);
    assert!(ui.is_animating(), "still animating");
    assert_eq!(builds.get(), after_hover, "only the paint phase ran");
}

#[test]
fn input_uses_the_final_layout_while_things_move() {
    let on = State::new(false);
    let mut ui = running(VStack::new((Toggle::new(on.binding()),)));
    let frame_before = quads(&ui)[0].rect;
    let thumb = |ui: &Ui| quads(ui).iter().find(|q| fill(q) == Color::WHITE).expect("thumb").rect;
    let start = thumb(&ui);
    let mut t = 0.0;
    click(&mut ui, frame_before.center());
    assert!(on.get());
    ui.update();
    frames(&mut ui, &mut t, 3);
    let mid = thumb(&ui);
    assert!(mid.x() > start.x(), "moving right");
    settle(&mut ui, &mut t);
    let end = thumb(&ui);
    assert!(mid.x() < end.x(), "still short of the end midway");
    assert!((end.max_x() - (frame_before.max_x() - 3.0)).abs() < 0.01);
    assert_eq!(end.width(), 18.0);
    // Clicking mid-flight works and turns it around.
    click(&mut ui, frame_before.center());
    assert!(!on.get());
}

#[test]
fn the_selection_marker_of_tabs_slides_between_tabs() {
    let selected = State::new(0_usize);
    let mut ui =
        running(Tabs::new(selected.binding()).tab("One", Text::new("a")).tab("A second", Text::new("b")));
    let accent = Theme::raven_dark().color(Semantic::Accent);
    let marker = |ui: &Ui| quads(ui).into_iter().find(|q| fill(q) == accent).expect("marker").rect;
    let first = marker(&ui);
    let mut t = 0.0;
    selected.set(1);
    ui.update();
    frames(&mut ui, &mut t, 3);
    let mid = quads(&ui).iter().find(|q| q.rect.height() == 2.0 && fill(q).a > 0.5).expect("marker").rect;
    assert!(mid.x() > first.x());
    settle(&mut ui, &mut t);
    let last = marker(&ui);
    assert!(mid.x() < last.x(), "{mid:?} {last:?}");
    assert!(last.width() != first.width(), "takes the size of the new tab");
    assert!(!ui.needs_redraw());
}

#[test]
fn a_closed_dialog_keeps_fading_but_takes_no_input() {
    let open = State::new(true);
    let clicks = State::new(0);
    let counter = clicks.clone();
    let mut ui = running(VStack::new((
        Button::new("Behind").on_click(move || counter.update(|c| *c += 1)),
        Dialog::new(open.binding(), "Hello"),
    )));
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    let count = quads(&ui).len();
    let button = quads(&ui)[0].rect;
    open.set(false);
    ui.update();
    frames(&mut ui, &mut t, 2);
    assert!(ui.is_animating());
    assert!(quads(&ui).len() >= count - 1, "the dialog is still on screen");
    assert!(
        !ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Hello")),
        "screen readers no longer see it"
    );
    click(&mut ui, button.center());
    assert_eq!(clicks.get(), 1, "the app underneath already works");
    settle(&mut ui, &mut t);
    assert_eq!(quads(&ui).len(), 1, "only the button remains");
    assert!(!ui.needs_redraw());
}

#[test]
fn reopening_during_the_exit_continues_from_where_it_was() {
    let open = State::new(true);
    let mut ui = running(VStack::new((Dialog::new(open.binding(), "Hello"),)));
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    open.set(false);
    ui.update();
    frames(&mut ui, &mut t, 3);
    let alpha = |ui: &Ui| quads(ui).first().map(|q| fill(q).a);
    let fading = alpha(&ui).expect("still visible");
    let full = Theme::raven_dark().color(Semantic::Scrim).a;
    assert!(fading < full);
    open.set(true);
    ui.update();
    let revived = alpha(&ui).expect("visible");
    assert!((revived - fading).abs() < 0.05, "{revived} vs {fading}");
    assert_eq!(quads(&ui).iter().filter(|q| q.rect.width() >= 800.0).count(), 1, "no second copy");
    settle(&mut ui, &mut t);
    assert_eq!(alpha(&ui), Some(full));
}

#[test]
fn popups_grow_from_their_anchor() {
    let open = State::new(false);
    let mut ui = running(VStack::new((Popover::new(open.binding(), Button::new("Open"), Text::new("Hi")),)));
    let anchor = quads(&ui)[0].rect;
    let mut t = 0.0;
    open.set(true);
    ui.update();
    frames(&mut ui, &mut t, 2);
    let growing = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("panel").rect;
    settle(&mut ui, &mut t);
    let done = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("panel").rect;
    assert!(growing.width() < done.width());
    assert!((growing.y() - done.y()).abs() < 0.01, "the edge next to the anchor stays put");
    assert!(done.y() >= anchor.max_y());
}

#[test]
fn stalls_do_not_make_animations_jump() {
    let on = State::new(false);
    let mut ui = running(VStack::new((Toggle::new(on.binding()),)));
    let track = quads(&ui)[0].rect;
    let thumb = |ui: &Ui| quads(ui).iter().find(|q| fill(q) == Color::WHITE).expect("thumb").rect;
    let start = thumb(&ui);
    let mut t = 0.0;
    click(&mut ui, track.center());
    ui.update();
    frames(&mut ui, &mut t, 1);
    t += 30.0; // The window was hidden for half a minute.
    ui.set_time(t);
    ui.update();
    let after = thumb(&ui);
    assert!(ui.is_animating());
    assert!(after.x() - start.x() < 10.0, "continues instead of finishing unseen");
    // A machine this slow still gets through it, rather than almost freezing.
    for _ in 0..40 {
        t += 0.5;
        ui.set_time(t);
        ui.update();
    }
    assert!(!ui.is_animating());
}

#[test]
fn scrolling_does_not_count_as_layout_motion() {
    struct Row;
    impl View for Row {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let t = cx.theme().motion.snappy;
            let node = cx.container(
                LayoutStyle { height: Some(100.0), ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint::new(Color::WHITE)),
            );
            cx.motion(node, Motion::new().layout(t));
            node
        }
    }
    struct Rows;
    impl View for Rows {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let rows: Vec<NodeId> = (0..30).map(|i| cx.keyed_child(i, &Row)).collect();
            cx.container(LayoutStyle::stack(Axis::Vertical), &rows, Paint::None)
        }
    }
    let mut ui = running(ScrollView::new((Rows,)).height(300.0));
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    let y = quads(&ui)[0].rect.y();
    ui.handle_event(&Event::Scroll { position: Point::new(10.0, 10.0), delta: Point::new(0.0, -50.0) });
    ui.update();
    assert!(!ui.is_animating());
    assert_eq!(quads(&ui)[0].rect.y(), y - 50.0, "follows the finger exactly");
}

#[test]
fn reduced_motion_stops_travel_but_keeps_fades() {
    let on = State::new(false);
    let open = State::new(false);
    let mut ui = running(VStack::new((Toggle::new(on.binding()), Dialog::new(open.binding(), "Hi"))));
    ui.set_reduced_motion(true);
    ui.update();
    let track = quads(&ui)[0].rect;
    let thumb = |ui: &Ui| quads(ui).iter().find(|q| fill(q) == Color::WHITE).expect("thumb").rect;
    let mut t = 0.0;
    click(&mut ui, track.center());
    ui.update();
    let end = thumb(&ui);
    assert!((end.max_x() - (track.max_x() - 3.0)).abs() < 0.01, "lands at once");
    settle(&mut ui, &mut t);
    open.set(true);
    ui.update();
    let scrim = quads(&ui).into_iter().find(|q| q.rect.width() >= 800.0).expect("backdrop");
    let full = Theme::raven_dark().color(Semantic::Scrim).a;
    assert!(fill(&scrim).a < full * 0.5, "the fade still plays");
    let card = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("card");
    settle(&mut ui, &mut t);
    let done = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("card");
    assert_eq!(card.rect, done.rect, "but nothing moves or scales");
}

#[test]
fn live_drawing_paints_without_rebuilding_and_is_capped() {
    let builds = Rc::new(Cell::new(0));
    let mut ui = running(Counting(Spinner::new(), builds.clone()));
    let built = builds.get();
    let first = ui.scene().primitives().to_vec();
    assert!(!ui.needs_redraw(), "waits for the next tick");
    let wake = ui.next_wake().expect("scheduled");
    assert!(wake <= FRAME + 1e-9);
    ui.set_time(wake);
    assert!(ui.needs_redraw());
    ui.update();
    assert_eq!(builds.get(), built);
    assert_ne!(ui.scene().primitives(), &first[..], "it moved");
}

#[test]
fn live_drawing_stops_under_reduced_motion() {
    let mut ui = running(Skeleton::line(20.0));
    ui.set_reduced_motion(true);
    ui.update();
    assert_eq!(ui.next_wake(), None);
    assert!(!ui.needs_redraw());
}

#[test]
fn a_theme_change_crossfades_the_whole_picture_and_lands_on_the_new_tokens() {
    let mut ui = running(Button::new("Save"));
    let mut t = 0.0;
    settle(&mut ui, &mut t);
    let light = Theme::raven_light();
    let old_canvas = Theme::raven_dark().color(Semantic::Canvas);
    ui.set_theme(light);
    ui.update();
    assert!(ui.is_animating());
    let cover = quads(&ui).into_iter().find(|q| q.rect.width() >= 800.0).expect("old picture");
    assert_eq!(fill(&cover).r, old_canvas.r, "starts showing the old theme");
    settle(&mut ui, &mut t);
    assert_eq!(ui.scene().clear_color(), light.color(Semantic::Canvas));
    assert_eq!(fill(&quads(&ui)[0]), light.color(Semantic::Accent));
    assert_eq!(quads(&ui).len(), 1);
}

#[test]
fn without_a_clock_everything_lands_at_once() {
    let open = State::new(false);
    let mut ui = Ui::new(VStack::new((Dialog::new(open.binding(), "Hi"),)), Theme::raven_dark());
    ui.update();
    open.set(true);
    ui.update();
    assert!(!ui.is_animating());
    open.set(false);
    ui.update();
    assert!(quads(&ui).is_empty());
    ui.update(); // Focus goes back to where it was.
    assert!(!ui.needs_redraw());
}

#[test]
fn collapse_animates_height_and_unbuilds_when_closed() {
    let open = State::new(false);
    let o = open.clone();
    struct Page(State<bool>);
    impl View for Page {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let body = VStack::new((Text::new("line"), Text::new("line"), Text::new("line")));
            let collapse = cx.child(&Collapse::new(self.0.get(), body));
            let below = cx.container(
                LayoutStyle { height: Some(10.0), width: Some(10.0), ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint::new(Color::WHITE)),
            );
            cx.container(LayoutStyle::stack(Axis::Vertical), &[collapse, below], Paint::None)
        }
    }
    let mut ui = running(Page(o));
    let y = |ui: &Ui| quads(ui).first().expect("marker below").rect.y();
    let closed = y(&ui);
    let mut t = 0.0;
    open.set(true);
    ui.update();
    frames(&mut ui, &mut t, 6);
    let mid = y(&ui);
    assert!(mid > closed, "pushes what is below");
    settle(&mut ui, &mut t);
    let full = y(&ui);
    assert!(mid < full);
    assert!(!ui.needs_redraw());
    open.set(false);
    ui.update();
    settle(&mut ui, &mut t);
    assert_eq!(y(&ui), closed);
    let glyphs = ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count();
    assert_eq!(glyphs, 0);
}

#[test]
fn effects_and_exits_are_available_to_third_party_views() {
    struct Mine(State<bool>);
    impl View for Mine {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let node = cx.container(
                LayoutStyle { width: Some(10.0), height: Some(10.0), ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint::new(Color::WHITE)),
            );
            let e = if self.0.get() { Effect::moved(100.0, 0.0) } else { Effect::IDENTITY };
            cx.motion(node, Motion::new().effect(e, Transition::base()));
            node
        }
    }
    let on = State::new(false);
    let o = on.clone();
    let mut ui = running(Mine(o));
    let x = |ui: &Ui| quads(ui)[0].rect.x();
    let start = x(&ui);
    let mut t = 0.0;
    on.set(true);
    ui.update();
    frames(&mut ui, &mut t, 6);
    assert!(x(&ui) > start && x(&ui) < start + 100.0);
    settle(&mut ui, &mut t);
    assert_eq!(x(&ui), start + 100.0);
}
