//! The layout and input pieces of the last batch: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_core::{
    AccessNode, AccessTree, Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Rect, Role, Size,
};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn key(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
    ui.update();
}

fn typed(ui: &mut Ui, text: &str) {
    key(ui, Key::Character(text.into()));
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn double_click(ui: &mut Ui, at: Point) {
    click(ui, at);
    ui.handle_event(&Event::DoubleClick { position: at });
    ui.update();
    ui.update();
}

fn named<'a>(tree: &'a AccessTree, role: Role, name: &str) -> &'a AccessNode {
    tree.find(role, name).unwrap_or_else(|| {
        let all: Vec<_> = tree.nodes.iter().map(|n| format!("{:?} {:?}", n.role, n.name)).collect();
        panic!("no {role:?} named {name:?} in {all:#?}")
    })
}

fn center_of(ui: &mut Ui, role: Role, name: &str) -> Point {
    let tree = ui.access_tree();
    named(&tree, role, name).bounds.center()
}

fn has(ui: &mut Ui, role: Role, name: &str) -> bool {
    ui.access_tree().find(role, name).is_some()
}

fn has_role(ui: &mut Ui, role: Role) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.role == role)
}

// ValueSlider

fn opacity_slider(value: &State<f32>) -> ValueSlider {
    ValueSlider::new(value.binding()).range(0.0, 100.0).step(5.0).unit("%").label("Opacity")
}

#[test]
fn value_slider_shows_the_value_and_opens_a_slider_only_when_asked() {
    let value = State::new(50.0_f32);
    let mut ui = ui(opacity_slider(&value));
    assert!(has(&mut ui, Role::Button, "Opacity: 50%"));
    assert!(!has_role(&mut ui, Role::Slider));
    let at = center_of(&mut ui, Role::Button, "Opacity: 50%");
    click(&mut ui, at);
    assert!(has(&mut ui, Role::Slider, "Opacity"));
    assert_eq!(ui.access_tree().find(Role::Button, "Opacity: 50%").unwrap().expanded, Some(true));
}

#[test]
fn value_slider_works_from_the_keyboard_alone() {
    let value = State::new(50.0_f32);
    let mut ui = ui(opacity_slider(&value));
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Enter);
    assert!(has_role(&mut ui, Role::Slider));
    // The keyboard is already in the slider.
    key(&mut ui, Key::ArrowRight);
    assert_eq!(value.get(), 55.0);
    key(&mut ui, Key::Home);
    assert_eq!(value.get(), 0.0);
    assert!(has(&mut ui, Role::Button, "Opacity: 0%"));
    key(&mut ui, Key::Escape);
    assert!(!has_role(&mut ui, Role::Slider));
    // And it is back on the button: Space opens again, and the keyboard goes in again.
    key(&mut ui, Key::Space);
    assert!(has_role(&mut ui, Role::Slider));
    key(&mut ui, Key::End);
    assert_eq!(value.get(), 100.0);
}

#[test]
fn value_slider_names_itself_from_labels_writes_in_their_language_and_can_format() {
    let value = State::new(0.5_f32);
    let mut pt = ui(Localized::new(Labels::pt_br(), ValueSlider::new(value.binding()).step(0.1)));
    assert!(has(&mut pt, Role::Button, "Ajustar o valor: 0,5"));
    let mut formatted = ui(ValueSlider::new(value.binding())
        .label("Mix")
        .value_format(|v| format!("{:.0} of 100", v * 100.0)));
    assert!(has(&mut formatted, Role::Button, "Mix: 50 of 100"));
}

#[test]
fn value_slider_can_show_its_label_and_can_be_turned_off() {
    let value = State::new(50.0_f32);
    let mut off = ui(opacity_slider(&value).disabled(true));
    let at = center_of(&mut off, Role::Button, "Opacity: 50%");
    click(&mut off, at);
    assert!(!has_role(&mut off, Role::Slider));
    assert!(off.access_tree().find(Role::Button, "Opacity: 50%").unwrap().disabled);

    let mut shown = ui(opacity_slider(&value).show_label());
    let plain = ui(opacity_slider(&value)).element_count();
    assert!(shown.element_count() > plain, "the label is there");
    assert!(has(&mut shown, Role::Button, "Opacity: 50%"));
}

#[test]
fn value_slider_field_takes_typed_numbers_up_to_the_limits_and_brings_bad_text_back() {
    let value = State::new(50.0_f32);
    let mut ui = ui(opacity_slider(&value).field());
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::End);
    key(&mut ui, Key::Backspace);
    key(&mut ui, Key::Backspace);
    typed(&mut ui, "20");
    assert_eq!(value.get(), 50.0, "only Enter or leaving takes it");
    key(&mut ui, Key::Enter);
    assert_eq!(value.get(), 20.0);

    typed(&mut ui, "000");
    key(&mut ui, Key::Enter);
    assert_eq!(value.get(), 100.0, "up to the limit");

    typed(&mut ui, "x");
    key(&mut ui, Key::Enter);
    assert_eq!(value.get(), 100.0);
    assert_eq!(ui.access_tree().find(Role::TextInput, "Opacity").unwrap().value.as_deref(), Some("100"));

    typed(&mut ui, "9");
    key(&mut ui, Key::Escape);
    assert_eq!(ui.access_tree().find(Role::TextInput, "Opacity").unwrap().value.as_deref(), Some("100"));

    key(&mut ui, Key::ArrowDown);
    assert_eq!(value.get(), 95.0, "the arrows take a step");
}

#[test]
fn value_slider_field_opens_the_slider_by_its_arrow() {
    let value = State::new(50.0_f32);
    let mut ui = ui(opacity_slider(&value).field());
    assert!(!has_role(&mut ui, Role::Slider));
    let at = center_of(&mut ui, Role::Button, "Opacity");
    click(&mut ui, at);
    assert!(has(&mut ui, Role::Slider, "Opacity"));
    key(&mut ui, Key::ArrowRight);
    assert_eq!(value.get(), 55.0);
    assert_eq!(ui.access_tree().find(Role::TextInput, "Opacity").unwrap().value.as_deref(), Some("55"));
}

// InlineRename

struct Renames {
    name: State<String>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl Renames {
    fn new() -> Self {
        Self { name: State::new(String::from("Warm")), calls: Rc::default() }
    }

    fn view(&self) -> InlineRename {
        let calls = self.calls.clone();
        InlineRename::new(self.name.binding())
            .label("Preset")
            .on_rename(move |name| calls.borrow_mut().push(name))
    }
}

#[test]
fn rename_starts_with_a_double_click_with_the_text_selected_and_enter_confirms() {
    let r = Renames::new();
    let mut ui = ui(r.view());
    assert!(!has_role(&mut ui, Role::TextInput));
    let at = center_of(&mut ui, Role::Button, "Warm");
    click(&mut ui, at);
    assert!(!has_role(&mut ui, Role::TextInput), "one click does not start");
    ui.handle_event(&Event::DoubleClick { position: at });
    ui.update();
    ui.update();
    assert!(has_role(&mut ui, Role::TextInput));
    // Typing replaces: the whole name was selected, and the field has the keyboard.
    typed(&mut ui, "Cold");
    assert_eq!(r.name.get(), "Warm", "the name waits for the confirmation");
    key(&mut ui, Key::Enter);
    assert!(!has_role(&mut ui, Role::TextInput));
    assert_eq!(r.name.get(), "Cold");
    assert_eq!(*r.calls.borrow(), ["Cold"]);
    // The keyboard is back on the name.
    key(&mut ui, Key::Other("F2".into()));
    assert!(has_role(&mut ui, Role::TextInput));
}

#[test]
fn rename_starts_with_f2_and_enter_and_escape_cancels_and_forgets_the_draft() {
    let r = Renames::new();
    let mut ui = ui(r.view());
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Other("Named(F2)".into()));
    assert!(has_role(&mut ui, Role::TextInput));
    typed(&mut ui, "Discarded");
    key(&mut ui, Key::Escape);
    assert!(!has_role(&mut ui, Role::TextInput));
    assert_eq!(r.name.get(), "Warm");
    assert!(r.calls.borrow().is_empty());
    assert!(has(&mut ui, Role::Button, "Warm"));
    // Focus came back, and the next edit starts from the name, not from the draft.
    key(&mut ui, Key::Enter);
    assert_eq!(ui.access_tree().find(Role::TextInput, "Preset").unwrap().value.as_deref(), Some("Warm"));
}

#[test]
fn rename_confirms_when_the_focus_goes_elsewhere() {
    let r = Renames::new();
    let other = Rc::new(Cell::new(0));
    let hits = other.clone();
    let view = VStack::new((r.view(), Button::new("Other").on_click(move || hits.set(hits.get() + 1))));
    let mut ui = ui(view);
    let at = center_of(&mut ui, Role::Button, "Warm");
    double_click(&mut ui, at);
    typed(&mut ui, "Elsewhere");
    let button = center_of(&mut ui, Role::Button, "Other");
    click(&mut ui, button);
    assert_eq!(r.name.get(), "Elsewhere");
    assert_eq!(*r.calls.borrow(), ["Elsewhere"], "once");
    assert!(!has_role(&mut ui, Role::TextInput));
    assert_eq!(other.get(), 1, "the click still reached the button");
    let tree = ui.access_tree();
    assert_eq!(tree.focus, Some(named(&tree, Role::Button, "Other").id), "the keyboard stayed there");
}

#[test]
fn rename_ignores_empty_and_equal_names_and_trims() {
    let r = Renames::new();
    let mut ui = ui(r.view());
    let start = |ui: &mut Ui| key(ui, Key::Other("F2".into()));
    key(&mut ui, Key::Tab);
    start(&mut ui);
    key(&mut ui, Key::Backspace);
    key(&mut ui, Key::Enter);
    assert_eq!(r.name.get(), "Warm", "empty");
    start(&mut ui);
    key(&mut ui, Key::Enter);
    assert!(r.calls.borrow().is_empty(), "the same");
    start(&mut ui);
    typed(&mut ui, "  Hot ");
    key(&mut ui, Key::Enter);
    assert_eq!(r.name.get(), "Hot");
}

#[test]
fn rename_can_be_started_from_outside_passes_clicks_on_and_can_be_off() {
    let r = Renames::new();
    let editing = State::new(false);
    let clicks = Rc::new(Cell::new(0));
    let hits = clicks.clone();
    let mut driven = ui(r.view().editing(editing.binding()).on_click(move || hits.set(hits.get() + 1)));
    let at = center_of(&mut driven, Role::Button, "Warm");
    click(&mut driven, at);
    assert_eq!(clicks.get(), 1);
    editing.set(true);
    driven.update();
    driven.update();
    assert!(has_role(&mut driven, Role::TextInput));
    key(&mut driven, Key::Escape);
    assert!(!editing.get());

    let mut off = ui(r.view().disabled(true));
    assert!(has(&mut off, Role::Label, "Warm"));
    key(&mut off, Key::Tab);
    key(&mut off, Key::Other("F2".into()));
    let at = center_of(&mut off, Role::Label, "Warm");
    double_click(&mut off, at);
    assert!(!has_role(&mut off, Role::TextInput));
}

#[test]
fn rename_explains_itself_to_screen_readers() {
    let r = Renames::new();
    let mut ui = ui(Localized::new(Labels::pt_br(), r.view()));
    let tree = ui.access_tree();
    assert_eq!(
        named(&tree, Role::Button, "Warm").description.as_deref(),
        Some("Clique duas vezes ou aperte F2 para renomear")
    );
}

// FloatingToolbar

/// The content stretches across the window.
const STAGE: Size = Size::new(800.0, 300.0);

fn stage() -> impl View {
    Spacer::new().frame(None, Some(STAGE.height))
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q) => Some(*q),
            _ => None,
        })
        .collect()
}

fn bounds(ui: &mut Ui, role: Role, name: &str) -> Rect {
    let tree = ui.access_tree();
    named(&tree, role, name).bounds
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: to });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn zoom_bar() -> FloatingToolbar<impl View> {
    FloatingToolbar::new(stage()).label("Zoom").action(ToolbarAction::new("Fit"))
}

#[test]
fn floating_toolbar_sits_over_its_content_where_it_is_told_without_changing_its_size() {
    let mut center = ui(zoom_bar());
    let bar = bounds(&mut center, Role::Toolbar, "Zoom");
    assert!(((bar.center().x) - STAGE.width / 2.0).abs() < 1.0, "centered: {bar:?}");
    assert!((bar.max_y() - (STAGE.height - 12.0)).abs() < 1.0, "over the bottom: {bar:?}");

    let mut corner = ui(zoom_bar().place(Align::End, Align::Start).margin(20.0));
    let bar = bounds(&mut corner, Role::Toolbar, "Zoom");
    assert!((bar.max_x() - (STAGE.width - 20.0)).abs() < 1.0, "{bar:?}");
    assert!((bar.y() - 20.0).abs() < 1.0, "{bar:?}");

    // What comes after the content does not make room for the bar.
    let mut after = ui(VStack::new((zoom_bar(), Button::new("After"))).spacing(0.0));
    assert!((bounds(&mut after, Role::Button, "After").y() - STAGE.height).abs() < 1.0);
}

#[test]
fn floating_toolbar_is_translucent_and_its_commands_work() {
    let hits = Rc::new(Cell::new(0));
    let counter = hits.clone();
    let view = FloatingToolbar::new(stage())
        .label("Zoom")
        .action(ToolbarAction::new("Fit").on_click(move || counter.set(counter.get() + 1)))
        .action(ToolbarAction::new("Fill").disabled(true))
        .separator()
        .view(Button::new("Free"));
    let mut fresh = ui(view);
    key(&mut fresh, Key::Tab);
    key(&mut fresh, Key::Enter);
    assert_eq!(hits.get(), 1, "Tab reaches it");
    let solid = Theme::raven_dark().color(Semantic::SurfaceElevated);
    let bar = bounds(&mut fresh, Role::Toolbar, "Zoom");
    let surface = quads(&fresh).into_iter().find(|q| q.rect == bar).expect("the bar paints");
    assert!(matches!(surface.fill, ravenrs_core::Brush::Solid(c) if c.a < solid.a), "{:?}", surface.fill);
    assert!(surface.shadow.is_some());

    let at = center_of(&mut fresh, Role::Button, "Fit");
    click(&mut fresh, at);
    assert_eq!(hits.get(), 2);
    assert!(has(&mut fresh, Role::Button, "Free"));
}

#[test]
fn floating_toolbar_keeps_clicks_from_reaching_the_content() {
    let hits = Rc::new(Cell::new(0));
    let counter = hits.clone();
    let under = Button::new("Under").full_width().on_click(move || counter.set(counter.get() + 1));
    let view = FloatingToolbar::new(under.frame(None, Some(STAGE.height)))
        .label("Zoom")
        .action(ToolbarAction::new("Fit"))
        .place(Align::Start, Align::Start);
    let mut ui = ui(view);
    let bar = bounds(&mut ui, Role::Toolbar, "Zoom");
    // Its own padding, beside the button.
    click(&mut ui, Point::new(bar.x() + 1.5, bar.center().y));
    assert_eq!(hits.get(), 0);
    click(&mut ui, Point::new(bar.max_x() + 40.0, bar.center().y));
    assert_eq!(hits.get(), 1);
}

#[test]
fn floating_toolbar_moves_by_its_handle_stays_inside_and_goes_home() {
    let place = State::new(None::<Point>);
    let mut ui = ui(zoom_bar().movable(place.binding()));
    let home = bounds(&mut ui, Role::Toolbar, "Zoom");
    let handle = center_of(&mut ui, Role::Button, "Move: Zoom");
    drag(&mut ui, handle, Point::new(handle.x - 50.0, handle.y - 100.0));
    let moved = bounds(&mut ui, Role::Toolbar, "Zoom");
    assert!((moved.x() - (home.x() - 50.0)).abs() < 1.0, "{moved:?} from {home:?}");
    assert!((moved.y() - (home.y() - 100.0)).abs() < 1.0, "{moved:?} from {home:?}");
    assert_eq!(place.get(), Some(moved.origin), "written when it ends");

    let handle = center_of(&mut ui, Role::Button, "Move: Zoom");
    drag(&mut ui, handle, Point::new(5000.0, 5000.0));
    let moved = bounds(&mut ui, Role::Toolbar, "Zoom");
    assert!(moved.max_x() <= STAGE.width && moved.max_y() <= STAGE.height, "inside: {moved:?}");

    // The arrows move it too, from the handle that has the keyboard.
    key(&mut ui, Key::ArrowUp);
    assert!(bounds(&mut ui, Role::Toolbar, "Zoom").y() < moved.y());

    let handle = center_of(&mut ui, Role::Button, "Move: Zoom");
    ui.handle_event(&Event::DoubleClick { position: handle });
    ui.update();
    ui.update();
    assert_eq!(place.get(), None);
    assert_eq!(bounds(&mut ui, Role::Toolbar, "Zoom"), home);
}

#[test]
fn floating_toolbar_has_no_handle_unless_asked_and_copes_with_no_room() {
    let mut fixed = ui(zoom_bar());
    assert!(!has(&mut fixed, Role::Button, "Move: Zoom"));
    let mut tiny = ui(FloatingToolbar::new(Spacer::new().frame(Some(4.0), Some(4.0)))
        .label("Zoom")
        .action(ToolbarAction::new("Fit")));
    let bar = bounds(&mut tiny, Role::Toolbar, "Zoom");
    assert!(bar.x() >= 0.0 && bar.y() >= 0.0, "{bar:?}");
}

// TitleBar

#[derive(Default)]
struct WindowEvents {
    minimize: Cell<u32>,
    maximize: Cell<u32>,
    close: Cell<u32>,
}

fn title_bar(events: &Rc<WindowEvents>) -> TitleBar {
    let (a, b, c) = (events.clone(), events.clone(), events.clone());
    TitleBar::new("Report.pdf")
        .on_minimize(move || a.minimize.set(a.minimize.get() + 1))
        .on_maximize(move || b.maximize.set(b.maximize.get() + 1))
        .on_close(move || c.close.set(c.close.get() + 1))
}

#[test]
fn title_bar_buttons_call_back_with_pointer_and_keyboard() {
    let events = Rc::new(WindowEvents::default());
    let mut ui = ui(title_bar(&events).action(Button::new("Share")));
    assert!(has(&mut ui, Role::Button, "Share"));
    for name in ["Minimize", "Maximize", "Close"] {
        let at = center_of(&mut ui, Role::Button, name);
        click(&mut ui, at);
    }
    assert_eq!((events.minimize.get(), events.maximize.get(), events.close.get()), (1, 1, 1));
    key(&mut ui, Key::Enter);
    assert_eq!(events.close.get(), 2, "the keyboard works on what was clicked last");
}

#[test]
fn title_bar_shows_only_the_buttons_it_can_answer_and_says_restore_when_maximized() {
    let mut bare = ui(TitleBar::new("Tool").on_close(|| {}));
    assert!(has(&mut bare, Role::Button, "Close"));
    assert!(!has(&mut bare, Role::Button, "Minimize"));
    assert!(!has(&mut bare, Role::Button, "Maximize"));

    let events = Rc::new(WindowEvents::default());
    let mut big = ui(Localized::new(Labels::pt_br(), title_bar(&events).maximized(true)));
    assert!(has(&mut big, Role::Button, "Restaurar"));
    assert!(has(&mut big, Role::Button, "Minimizar"));
    assert!(has(&mut big, Role::Button, "Fechar"));
    assert!(!has(&mut big, Role::Button, "Maximizar"));
}

#[test]
fn title_bar_double_click_maximizes_except_on_its_buttons_and_right_click_asks_for_a_menu() {
    let events = Rc::new(WindowEvents::default());
    let menu = Rc::new(Cell::new(None));
    let seen = menu.clone();
    let mut ui = ui(title_bar(&events).on_context_menu(move |at| seen.set(Some(at))));
    let bar = bounds(&mut ui, Role::Group, "Report.pdf");
    let middle = bar.center();
    ui.handle_event(&Event::DoubleClick { position: middle });
    assert_eq!(events.maximize.get(), 1);
    let close = center_of(&mut ui, Role::Button, "Close");
    ui.handle_event(&Event::DoubleClick { position: close });
    assert_eq!(events.maximize.get(), 1, "a double click on a button is about the button");
    ui.handle_event(&Event::PointerDown { position: middle, button: PointerButton::Secondary });
    assert_eq!(menu.get(), Some(middle));
}

#[test]
fn title_bar_has_a_long_title_cut_rather_than_push_the_buttons_out() {
    let events = Rc::new(WindowEvents::default());
    let long = "A very long name ".repeat(40);
    let mut ui = ui(TitleBar::new(long.clone())
        .on_close(move || events.close.set(1))
        .leading(Icon::new(IconName::File)));
    let close = bounds(&mut ui, Role::Button, "Close");
    assert!(close.max_x() <= 800.0, "{close:?}");
    assert!(close.x() > 700.0, "{close:?}");
}

#[test]
fn title_bar_goes_quiet_when_the_window_has_no_keyboard() {
    let theme = Theme::raven_dark();
    let mut on = ui(TitleBar::new("Report"));
    let mut off = ui(TitleBar::new("Report").active(false));
    let fill_of = |ui: &mut Ui| {
        let bar = bounds(ui, Role::Group, "Report");
        quads(ui).into_iter().find(|q| q.rect == bar).map(|q| q.fill)
    };
    assert_eq!(fill_of(&mut on), Some(ravenrs_core::Brush::Solid(theme.color(Semantic::Surface))));
    assert_eq!(fill_of(&mut off), Some(ravenrs_core::Brush::Solid(theme.color(Semantic::Canvas))));
}

// ScrollEdgeArrow

fn tile(n: u32) -> impl View {
    Button::new(format!("Item {n}")).frame(Some(100.0), Some(40.0))
}

/// 30 tiles of 100 with gaps of 8: 3132 of content in 800 of window.
fn strip() -> ScrollEdgeArrow<Vec<AnyView>> {
    let tiles: Vec<AnyView> = (0..30).map(|n| -> AnyView { Box::new(tile(n)) }).collect();
    ScrollEdgeArrow::new(tiles)
}

const BACK: &str = "Scroll back";
const FORWARD: &str = "Scroll forward";

fn wheel(ui: &mut Ui, at: Point, delta: Point) {
    ui.handle_event(&Event::Scroll { position: at, delta });
    ui.update();
    ui.update();
}

#[test]
fn edge_arrows_show_only_where_there_is_more_to_scroll() {
    let mut ui = ui(strip());
    assert!(has(&mut ui, Role::Button, FORWARD), "more to the right from the first frame");
    assert!(!has(&mut ui, Role::Button, BACK), "nothing to the left");
    let at = bounds(&mut ui, Role::Button, "Item 1").center();
    wheel(&mut ui, at, Point::new(-300.0, 0.0));
    assert!(has(&mut ui, Role::Button, BACK), "the wheel moved it");
    assert!(has(&mut ui, Role::Button, FORWARD));
    wheel(&mut ui, at, Point::new(-90000.0, 0.0));
    assert!(has(&mut ui, Role::Button, BACK));
    assert!(!has(&mut ui, Role::Button, FORWARD), "the end");
}

#[test]
fn edge_arrows_are_not_there_when_everything_fits() {
    let tiles: Vec<AnyView> = (0..3).map(|n| -> AnyView { Box::new(tile(n)) }).collect();
    let mut ui = ui(ScrollEdgeArrow::new(tiles));
    assert!(!has(&mut ui, Role::Button, FORWARD));
    assert!(!has(&mut ui, Role::Button, BACK));
}

#[test]
fn edge_arrows_notice_when_the_content_or_the_window_changes() {
    let many = State::new(false);
    let shown = many.clone();
    let view = view_fn(move || {
        let count = if shown.get() { 30 } else { 3 };
        let tiles: Vec<AnyView> = (0..count).map(|n| -> AnyView { Box::new(tile(n)) }).collect();
        ScrollEdgeArrow::new(tiles)
    });
    let mut ui = ui(view);
    assert!(!has(&mut ui, Role::Button, FORWARD));
    many.set(true);
    ui.update();
    assert!(has(&mut ui, Role::Button, FORWARD), "new content, in the same update");
    ui.set_viewport(Size::new(4000.0, 600.0), 1.0);
    ui.update();
    assert!(!has(&mut ui, Role::Button, FORWARD), "a window where everything fits");
}

#[test]
fn edge_arrow_scrolls_most_of_a_screen_at_a_time_and_lies_over_the_end_of_the_strip() {
    let mut ui = ui(strip());
    let before = bounds(&mut ui, Role::Button, "Item 0");
    let arrow = bounds(&mut ui, Role::Button, FORWARD);
    assert_eq!(arrow.max_x(), 800.0);
    assert_eq!(arrow.height(), 40.0, "as tall as the strip");
    click(&mut ui, arrow.center());
    let after = bounds(&mut ui, Role::Button, "Item 0");
    assert_eq!(after.x() - before.x(), -640.0, "80% of the 800 of the window");
    let back = bounds(&mut ui, Role::Button, BACK);
    assert_eq!(back.x(), 0.0);
    click(&mut ui, back.center());
    assert_eq!(bounds(&mut ui, Role::Button, "Item 0").x(), before.x());
    assert!(!has(&mut ui, Role::Button, BACK));
}

#[test]
fn edge_arrow_fades_over_what_is_behind_it() {
    let mut ui = ui(strip().fade(Semantic::Surface));
    let arrow = bounds(&mut ui, Role::Button, FORWARD);
    let quad = quads(&ui).into_iter().find(|q| q.rect == arrow).expect("the arrow paints");
    let ravenrs_core::Brush::LinearGradient(gradient) = quad.fill else { panic!("{:?}", quad.fill) };
    assert_eq!(gradient.from, Theme::raven_dark().color(Semantic::Surface));
    assert_eq!(gradient.to.a, 0.0);
    assert!(gradient.start.x > gradient.end.x, "solid at the edge of the strip");
}

#[test]
fn edge_arrows_work_from_the_keyboard_and_the_keyboard_is_not_lost_at_the_end() {
    let tiles: Vec<AnyView> = (0..12).map(|n| -> AnyView { Box::new(tile(n)) }).collect();
    let mut ui = ui(ScrollEdgeArrow::new(tiles));
    let arrow = bounds(&mut ui, Role::Button, FORWARD);
    click(&mut ui, arrow.center());
    assert!(!has(&mut ui, Role::Button, FORWARD), "12 tiles end within one step");
    let tree = ui.access_tree();
    assert_eq!(tree.focus, Some(named(&tree, Role::Button, BACK).id), "on the arrow that is left");
    key(&mut ui, Key::Space);
    assert!(!has(&mut ui, Role::Button, BACK), "and it answers the keyboard");
    let tree = ui.access_tree();
    assert_eq!(tree.focus, Some(named(&tree, Role::Button, FORWARD).id), "and back again");
    key(&mut ui, Key::Enter);
    assert!(has(&mut ui, Role::Button, BACK));
}

#[test]
fn edge_arrows_can_go_up_and_down_and_speak_the_language() {
    let tiles: Vec<AnyView> = (0..30).map(|n| -> AnyView { Box::new(tile(n)) }).collect();
    let view = ScrollEdgeArrow::new(tiles).vertical().height(300.0);
    let mut ui = ui(Localized::new(Labels::pt_br(), view));
    assert!(!has(&mut ui, Role::Button, "Rolar para trás"));
    let arrow = bounds(&mut ui, Role::Button, "Rolar para a frente");
    assert_eq!(arrow.max_y(), 300.0);
    assert_eq!(arrow.width(), 800.0);
    let before = bounds(&mut ui, Role::Button, "Item 0");
    click(&mut ui, arrow.center());
    assert_eq!(bounds(&mut ui, Role::Button, "Item 0").y() - before.y(), -240.0);
    assert!(has(&mut ui, Role::Button, "Rolar para trás"));
}
