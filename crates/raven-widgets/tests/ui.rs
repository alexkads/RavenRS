//! Drives the UI runtime the way a platform layer does, using only public API.

use std::cell::Cell;
use std::rc::Rc;

use raven_core::{
    Color, CursorIcon, Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Rect, Size,
};
use raven_reactive::State;
use raven_style::{Semantic, Theme};
use raven_widgets::{
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
        let style = raven_layout::LayoutStyle { padding: 6.0.into(), ..raven_layout::LayoutStyle::leaf() };
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
