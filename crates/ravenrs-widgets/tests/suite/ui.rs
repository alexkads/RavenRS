//! Drives the UI runtime the way a platform layer does, using only public API.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Rect, Size,
};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::{
    BuildCx, Button, Canvas, Card, Component, NodeId, Paint, QuadPaint, Text, Ui, VStack, View, view_fn,
};

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

fn glyph_runs(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
}

fn quad_with_fill(ui: &Ui, color: Color) -> Option<Quad> {
    quads(ui).into_iter().find(|q| q.fill == color.into())
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
}

struct Counter {
    count: State<i32>,
}

impl Component for Counter {
    fn body(&self) -> impl View {
        let count = self.count.clone();
        VStack::new((
            Text::new("Welcome to RavenRS"),
            Card::new(Text::bound(&self.count, |c| format!("Clicks: {c}"))),
            Button::new("Get Started").on_click(move || count.update(|c| *c += 1)),
        ))
        .padding(32.0)
        .spacing(24.0)
    }
}

fn counter_ui() -> (Ui, State<i32>) {
    let count = State::new(0);
    let mut ui = Ui::new(Counter { count: count.clone() }, Theme::raven_dark());
    ui.set_viewport(Size::new(600.0, 400.0), 2.0);
    ui.update();
    (ui, count)
}

#[test]
fn first_frame_has_card_button_and_text() {
    let (ui, _) = counter_ui();
    let theme = Theme::raven_dark();
    assert_eq!(ui.scene().clear_color(), theme.color(Semantic::Canvas));
    let card = quad_with_fill(&ui, theme.color(Semantic::SurfaceElevated)).expect("card quad");
    assert_eq!(card.corner_radii, [24.0; 4]);
    assert!(card.shadow.is_some());
    let button = quad_with_fill(&ui, theme.color(Semantic::Accent)).expect("button quad");
    assert!(button.rect.height() >= 40.0, "button meets the minimum target size");
    assert!(button.rect.y() > card.rect.max_y(), "button is below the card");
    assert_eq!(glyph_runs(&ui), 3);
}

#[test]
fn clicking_the_button_updates_bound_text() {
    let (mut ui, count) = counter_ui();
    let button = quad_with_fill(&ui, Theme::raven_dark().color(Semantic::Accent)).expect("button").rect;
    click(&mut ui, button.center());
    assert_eq!(count.get(), 1);
    assert!(ui.needs_redraw());
    ui.update();
    assert!(!ui.needs_redraw());
    click(&mut ui, button.center());
    assert_eq!(count.get(), 2);
}

#[test]
fn press_outside_then_release_on_button_does_not_click() {
    let (mut ui, count) = counter_ui();
    let button = quad_with_fill(&ui, Theme::raven_dark().color(Semantic::Accent)).expect("button").rect;
    ui.handle_event(&Event::PointerDown { position: Point::new(1.0, 1.0), button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: button.center(), button: PointerButton::Primary });
    assert_eq!(count.get(), 0);
}

#[test]
fn hover_changes_fill_and_cursor() {
    let (mut ui, _) = counter_ui();
    let theme = Theme::raven_dark();
    let button = quad_with_fill(&ui, theme.color(Semantic::Accent)).expect("button").rect;
    ui.handle_event(&Event::PointerMove { position: button.center() });
    ui.update();
    assert_eq!(ui.cursor(), CursorIcon::Pointer);
    assert!(quad_with_fill(&ui, theme.color(Semantic::AccentHover)).is_some());
    ui.handle_event(&Event::PointerMove { position: Point::new(1.0, 1.0) });
    ui.update();
    assert_eq!(ui.cursor(), CursorIcon::Default);
    assert!(quad_with_fill(&ui, theme.color(Semantic::Accent)).is_some());
}

#[test]
fn keyboard_focus_shows_ring_and_activates() {
    let (mut ui, count) = counter_ui();
    let ring = Theme::raven_dark().color(Semantic::FocusRing);
    let has_ring = |ui: &Ui| quads(ui).iter().any(|q| q.border.is_some_and(|b| b.color == ring));
    assert!(!has_ring(&ui));
    key(&mut ui, Key::Tab);
    ui.update();
    assert!(has_ring(&ui), "Tab shows the focus ring");
    key(&mut ui, Key::Enter);
    key(&mut ui, Key::Space);
    assert_eq!(count.get(), 2);
    key(&mut ui, Key::Escape);
    ui.update();
    assert!(!has_ring(&ui));
}

#[test]
fn unrelated_state_does_not_invalidate() {
    let unrelated = State::new(0);
    let (mut ui, count) = counter_ui();
    let wakes = Rc::new(Cell::new(0));
    let w = wakes.clone();
    ui.set_waker(move || w.set(w.get() + 1));
    unrelated.set(1);
    assert!(!ui.needs_redraw());
    assert_eq!(wakes.get(), 0);
    count.set(10);
    assert!(ui.needs_redraw());
    assert_eq!(wakes.get(), 1, "the platform is asked for exactly one frame");
}

/// A component written against the low-level API, like the built-in widgets.
struct Badge {
    label: &'static str,
    clicks: Rc<Cell<u32>>,
}

impl View for Badge {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let fill =
            if cx.is_hovered() { theme.color(Semantic::Success) } else { theme.color(Semantic::Warning) };
        let text = cx.child(&Text::new(self.label).foreground(Semantic::Ink));
        let style =
            ravenrs_layout::LayoutStyle { padding: 6.0.into(), ..ravenrs_layout::LayoutStyle::leaf() };
        let node = cx.container(
            style,
            &[text],
            Paint::Quad(QuadPaint { corner_radius: 9999.0, ..QuadPaint::new(fill) }),
        );
        let clicks = self.clicks.clone();
        cx.on_click(node, Rc::new(move || clicks.set(clicks.get() + 1)));
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

#[test]
fn custom_low_level_component_paints_and_handles_input() {
    let clicks = Rc::new(Cell::new(0));
    let mut ui =
        Ui::new(VStack::new((Badge { label: "Beta", clicks: clicks.clone() },)), Theme::raven_dark());
    ui.update();
    let theme = Theme::raven_dark();
    let badge = quad_with_fill(&ui, theme.color(Semantic::Warning)).expect("badge").rect;
    click(&mut ui, badge.center());
    ui.update();
    assert_eq!(clicks.get(), 1);
    assert!(
        quad_with_fill(&ui, theme.color(Semantic::Success)).is_some(),
        "hover state read through BuildCx"
    );
}

#[test]
fn view_fn_and_option_switch_subtrees() {
    let show = State::new(false);
    let s = show.clone();
    let mut ui = Ui::new(
        view_fn(move || VStack::new((Text::new("always"), s.get().then(|| Text::new("sometimes"))))),
        Theme::raven_light(),
    );
    ui.update();
    assert_eq!(glyph_runs(&ui), 1);
    show.set(true);
    ui.update();
    assert_eq!(glyph_runs(&ui), 2);
}

#[test]
fn canvas_draws_in_its_frame() {
    let mut ui = Ui::new(
        VStack::new((Canvas::new(|frame, scene| scene.push_quad(Quad::new(frame, Color::hex(0x123456))))
            .size(50.0, 20.0),))
        .padding(10.0),
        Theme::raven_dark(),
    );
    ui.update();
    let q = quad_with_fill(&ui, Color::hex(0x123456)).expect("canvas quad");
    assert_eq!(q.rect, Rect::new(10.0, 10.0, 50.0, 20.0));
}

#[test]
fn theme_switch_repaints_with_new_tokens() {
    let (mut ui, _) = counter_ui();
    ui.set_theme(Theme::raven_light());
    ui.update();
    assert_eq!(ui.scene().clear_color(), Theme::raven_light().color(Semantic::Canvas));
}

/// An 80 by 20 box at (100, 100), turned a quarter, with a 20 by 20 box at its right end.
struct Turned {
    hits: Rc<Cell<(u32, u32)>>,
}

impl View for Turned {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        use ravenrs_layout::LayoutStyle;
        let at = |x: f32, y: f32, w: f32, h: f32| LayoutStyle {
            absolute: Some((x, y)),
            width: Some(w),
            height: Some(h),
            ..LayoutStyle::leaf()
        };
        let inner =
            cx.container(at(60.0, 0.0, 20.0, 20.0), &[], Paint::Quad(QuadPaint::new(Color::hex(0x00ff00))));
        let hits = self.hits.clone();
        cx.on_click(inner, Rc::new(move || hits.set((hits.get().0, hits.get().1 + 1))));
        cx.semantics(inner, ravenrs_widgets::Semantics::new(ravenrs_core::Role::Button).name("inner"));
        let outer = cx.container(
            at(100.0, 100.0, 80.0, 20.0),
            &[inner],
            Paint::Quad(QuadPaint::new(Color::hex(0xff0000))),
        );
        let hits = self.hits.clone();
        cx.on_click(outer, Rc::new(move || hits.set((hits.get().0 + 1, hits.get().1))));
        cx.semantics(outer, ravenrs_widgets::Semantics::new(ravenrs_core::Role::Group).name("outer"));
        cx.rotate(outer, std::f32::consts::FRAC_PI_2);
        cx.container(
            LayoutStyle { width: Some(400.0), height: Some(300.0), ..LayoutStyle::leaf() },
            &[outer],
            Paint::None,
        )
    }
}

#[test]
fn a_turned_node_paints_turned_and_the_pointer_finds_it_where_it_shows() {
    let hits = Rc::new(Cell::new((0, 0)));
    let mut ui = Ui::new(Turned { hits: hits.clone() }, Theme::raven_dark());
    ui.set_viewport(Size::new(400.0, 300.0), 1.0);
    ui.update();
    let red = quad_with_fill(&ui, Color::hex(0xff0000)).expect("the turned box");
    assert!((red.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    // Standing up around its center (140, 110): 130..150 across, 70..150 down; the small box
    // went to its bottom end.
    click(&mut ui, Point::new(140.0, 80.0));
    assert_eq!(hits.get(), (1, 0), "the top of the standing box");
    click(&mut ui, Point::new(140.0, 140.0));
    assert_eq!(hits.get(), (1, 1), "the small box, where it turned to");
    click(&mut ui, Point::new(170.0, 110.0));
    assert_eq!(hits.get(), (1, 1), "where the lying box was, nothing now");
    let tree = ui.access_tree();
    let bounds = |name: &str| {
        tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).map(|n| n.bounds).expect("a node")
    };
    let near = |r: Rect, x: f32, y: f32, w: f32, h: f32| {
        (r.x() - x).abs() < 0.01
            && (r.y() - y).abs() < 0.01
            && (r.width() - w).abs() < 0.01
            && (r.height() - h).abs() < 0.01
    };
    assert!(near(bounds("outer"), 130.0, 70.0, 20.0, 80.0), "{:?}", bounds("outer"));
    assert!(near(bounds("inner"), 130.0, 130.0, 20.0, 20.0), "{:?}", bounds("inner"));
}

/// A box of a fixed size that counts its clicks.
struct Block {
    color: Color,
    hits: Rc<Cell<u32>>,
}

impl View for Block {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        use ravenrs_layout::LayoutStyle;
        let style = LayoutStyle { width: Some(80.0), height: Some(20.0), ..LayoutStyle::leaf() };
        let node = cx.container(style, &[], Paint::Quad(QuadPaint::new(self.color)));
        let hits = self.hits.clone();
        cx.on_click(node, Rc::new(move || hits.set(hits.get() + 1)));
        cx.semantics(node, ravenrs_widgets::Semantics::new(ravenrs_core::Role::Button).name("block"));
        node
    }
}

/// A quarter turn with a marker after it in a row, to show how much room the turn takes.
struct TurnedRow {
    turn: ravenrs_widgets::Turn,
    hits: Rc<Cell<u32>>,
}

impl View for TurnedRow {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        use ravenrs_layout::{Axis, LayoutStyle};
        let block = Block { color: Color::hex(0x0000ff), hits: self.hits.clone() };
        let turned = cx.child(&ravenrs_widgets::QuarterTurn::new(block, self.turn));
        let marker = cx.container(
            LayoutStyle { width: Some(10.0), height: Some(10.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint::new(Color::hex(0x00ff00))),
        );
        cx.container(LayoutStyle::stack(Axis::Horizontal), &[turned, marker], Paint::None)
    }
}

fn turned_ui(turn: ravenrs_widgets::Turn, hits: &Rc<Cell<u32>>) -> Ui {
    let mut ui = Ui::new(TurnedRow { turn, hits: hits.clone() }, Theme::raven_dark());
    ui.set_viewport(Size::new(400.0, 300.0), 1.0);
    // The first layout measures what it holds, the next gives the box that size.
    ui.update();
    ui.update();
    ui
}

#[test]
fn a_quarter_turn_stands_its_content_up_in_a_box_turned_too() {
    let hits = Rc::new(Cell::new(0));
    let mut ui = turned_ui(ravenrs_widgets::Turn::Clockwise, &hits);
    let marker = quad_with_fill(&ui, Color::hex(0x00ff00)).expect("the marker");
    assert_eq!(marker.rect.x(), 20.0, "the turned box is as wide as the block is tall");
    let block = quad_with_fill(&ui, Color::hex(0x0000ff)).expect("the block");
    assert!((block.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    let tree = ui.access_tree();
    let bounds = tree.nodes.iter().find(|n| n.name.as_deref() == Some("block")).expect("the block").bounds;
    let near = |a: f32, b: f32| (a - b).abs() < 0.01;
    assert!(
        near(bounds.x(), 0.0)
            && near(bounds.y(), 0.0)
            && near(bounds.width(), 20.0)
            && near(bounds.height(), 80.0),
        "standing in its box: {bounds:?}"
    );
    click(&mut ui, Point::new(10.0, 70.0));
    assert_eq!(hits.get(), 1, "the pointer finds it where it stands");
    click(&mut ui, Point::new(60.0, 10.0));
    assert_eq!(hits.get(), 1, "and not where it would lie");
}

#[test]
fn a_quarter_turn_goes_the_other_way_when_asked() {
    let hits = Rc::new(Cell::new(0));
    let ui = turned_ui(ravenrs_widgets::Turn::CounterClockwise, &hits);
    let block = quad_with_fill(&ui, Color::hex(0x0000ff)).expect("the block");
    assert!((block.rotation + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert_eq!(quad_with_fill(&ui, Color::hex(0x00ff00)).expect("the marker").rect.x(), 20.0);
}

#[test]
fn turned_text_reads_the_same_and_its_glyphs_turn() {
    let view = ravenrs_widgets::QuarterTurn::new(Text::new("Files"), ravenrs_widgets::Turn::Clockwise);
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(400.0, 300.0), 1.0);
    ui.update();
    ui.update();
    let run = ui
        .scene()
        .primitives()
        .iter()
        .find_map(|p| if let Primitive::Glyphs(run) = p { Some(run.clone()) } else { None })
        .expect("the text");
    assert!((run.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5, "{}", run.rotation);
    let tree = ui.access_tree();
    let files = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Files")).expect("still read");
    assert!(files.bounds.height() > files.bounds.width(), "taller than wide: {:?}", files.bounds);
}
