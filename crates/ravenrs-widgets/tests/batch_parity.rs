//! Behavior of the parity batch: dates, toggles, pagination, context menus,
//! hover cards, sheets, toasts, command palette, combobox, spinner, carousel,
//! splitter, OTP input and form fields.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(900.0, 700.0), 1.0);
    ui.update();
    ui
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn key(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
    ui.update();
}

fn typ(ui: &mut Ui, text: &str) {
    for c in text.chars() {
        key(ui, Key::Character(c.to_string()), Modifiers::default());
    }
}

fn has_panel(ui: &Ui) -> bool {
    quads(ui).iter().any(|q| q.shadow.is_some_and(|s| s.blur >= 40.0))
}

#[test]
fn calendar_picks_a_day_and_changes_month() {
    let picked = State::new(None);
    let october = Date::new(2026, 10, 1).unwrap();
    let mut ui = ui(VStack::new((Calendar::new(picked.binding(), october),)));
    // Oct 2026 starts on a Thursday: the first grid row is Sep 27..Oct 3.
    // Header 32px + gap 10 + weekday row 28 + gap 2; cells are 36 + 2.
    let row0 = 32.0 + 10.0 + 28.0 + 2.0 + 18.0;
    click(&mut ui, Point::new(4.0 * 38.0 + 18.0, row0));
    assert_eq!(picked.get(), Date::new(2026, 10, 1));
    // Next month, then pick the first cell (Nov 1, 2026 is a Sunday).
    click(&mut ui, Point::new(7.0 * 38.0 - 4.0, 16.0));
    click(&mut ui, Point::new(18.0, row0));
    assert_eq!(picked.get(), Date::new(2026, 11, 1));
}

#[test]
fn date_picker_opens_picks_and_closes() {
    let value = State::new(None);
    let mut ui =
        ui(VStack::new((DatePicker::new(value.binding(), Date::new(2026, 10, 1).unwrap()),)).padding(10.0));
    click(&mut ui, Point::new(100.0, 30.0));
    assert!(has_panel(&ui), "calendar popover opened");
    let panel = quads(&ui).into_iter().find(|q| q.shadow.is_some_and(|s| s.blur >= 40.0)).unwrap().rect;
    click(
        &mut ui,
        Point::new(panel.x() + 12.0 + 4.0 * 38.0 + 18.0, panel.y() + 12.0 + 32.0 + 10.0 + 28.0 + 2.0 + 18.0),
    );
    assert_eq!(value.get(), Date::new(2026, 10, 1));
    assert!(!has_panel(&ui), "picking closes the popover");
}

#[test]
fn toggles_and_toggle_groups() {
    let bold = State::new(false);
    let align = State::new(0_usize);
    let styles = State::new(BTreeSet::new());
    let mut ui = ui(VStack::new((
        ToggleButton::new(bold.binding(), "Bold"),
        ToggleGroup::single(align.binding(), [(Some(IconName::Menu), ""), (Some(IconName::List), "")]),
        ToggleGroup::multiple(styles.binding(), [(None, "A"), (None, "B")]),
    )));
    click(&mut ui, Point::new(10.0, 17.0));
    assert!(bold.get());
    click(&mut ui, Point::new(34.0 + 4.0 + 17.0, 34.0 + 16.0 + 17.0));
    assert_eq!(align.get(), 1);
    click(&mut ui, Point::new(10.0, 2.0 * 50.0 + 17.0));
    assert_eq!(styles.get(), BTreeSet::from([0]));
}

#[test]
fn pagination_moves_between_pages() {
    let page = State::new(0_usize);
    let mut ui = ui(VStack::new((Pagination::new(page.binding(), 20),)));
    // Find the "Next" button: the right-most button-sized quad.
    let next = quads(&ui)
        .into_iter()
        .filter(|q| q.rect.height() == 32.0)
        .max_by(|a, b| a.rect.x().total_cmp(&b.rect.x()));
    // Ghost buttons have transparent fills, but they still paint a quad.
    click(&mut ui, next.expect("next button").rect.center());
    assert_eq!(page.get(), 1);
}

#[test]
fn context_menu_opens_at_the_pointer_and_runs_an_action() {
    let hit = Rc::new(Cell::new(false));
    let h = hit.clone();
    let mut ui = ui(VStack::new((ContextMenu::new(Text::new("Right-click me").padding(40.0))
        .item(MenuItem::new("Rename"))
        .item(MenuItem::new("Delete").on_select(move || h.set(true))),)));
    ui.handle_event(&Event::PointerDown {
        position: Point::new(60.0, 50.0),
        button: PointerButton::Secondary,
    });
    ui.update();
    let menu = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("menu").rect;
    assert_eq!((menu.x(), menu.y()), (60.0, 50.0), "menu opens where the pointer is");
    click(&mut ui, Point::new(menu.x() + 30.0, menu.y() + 6.0 + 34.0 + 17.0));
    assert!(hit.get());
}

#[test]
fn hover_card_stays_open_over_the_card() {
    let mut ui =
        ui(VStack::new((HoverCard::new(Text::new("@raven"), Text::new("Profile details").padding(20.0)),))
            .padding(20.0));
    ui.handle_event(&Event::PointerMove { position: Point::new(30.0, 30.0) });
    ui.update();
    let card = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("hover card").rect;
    ui.handle_event(&Event::PointerMove { position: card.center() });
    ui.update();
    assert!(quads(&ui).iter().any(|q| q.shadow.is_some()), "moving onto the card keeps it open");
    ui.handle_event(&Event::PointerMove { position: Point::new(800.0, 600.0) });
    ui.update();
    assert!(quads(&ui).iter().all(|q| q.shadow.is_none()));
}

#[test]
fn sheet_slides_from_the_right_and_closes_on_backdrop() {
    let open = State::new(true);
    let mut ui = ui(VStack::new((Sheet::new(open.binding(), "Filters").width(300.0),)));
    let panel = quads(&ui).into_iter().find(|q| q.shadow.is_some()).expect("sheet").rect;
    assert_eq!((panel.x(), panel.width(), panel.height()), (600.0, 300.0, 700.0));
    click(&mut ui, Point::new(700.0, 300.0));
    assert!(open.get(), "clicks inside the sheet keep it open");
    click(&mut ui, Point::new(100.0, 300.0));
    assert!(!open.get());
}

#[test]
fn toasts_expire_on_the_clock() {
    let toasts = Toasts::new();
    let mut ui = ui(VStack::new((Toaster::new(toasts.clone()),)));
    ui.set_time(10.0);
    toasts.show("Saved", "Your changes are live.", Tone::Success);
    ui.update();
    assert!(has_panel(&ui));
    assert_eq!(ui.next_wake(), Some(14.0), "wakes when the toast expires");
    ui.set_time(13.9);
    assert!(!ui.needs_redraw());
    ui.set_time(14.0);
    assert!(ui.needs_redraw());
    ui.update();
    ui.update();
    assert!(!has_panel(&ui));
    assert!(toasts.is_empty());
}

#[test]
fn command_palette_opens_with_cmd_k_filters_and_runs() {
    let open = State::new(false);
    let ran = Rc::new(Cell::new(""));
    let (a, b) = (ran.clone(), ran.clone());
    let mut ui = ui(VStack::new((CommandPalette::new(open.binding())
        .command(Command::new("Open settings", move || a.set("settings")).icon(IconName::Settings))
        .command(Command::new("Toggle theme", move || b.set("theme")).icon(IconName::Moon)),)));
    key(&mut ui, Key::Character("k".into()), Modifiers { meta: true, ..Default::default() });
    assert!(open.get());
    typ(&mut ui, "tgth");
    key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(ran.get(), "theme");
    assert!(!open.get());
}

#[test]
fn combobox_filters_and_picks_with_keyboard() {
    let sel = State::new(None);
    let mut ui =
        ui(VStack::new((Combobox::new(sel.binding(), ["Brazil", "Portugal", "Peru", "Chile"]),))
            .padding(10.0));
    click(&mut ui, Point::new(100.0, 30.0));
    typ(&mut ui, "pe");
    key(&mut ui, Key::ArrowDown, Modifiers::default());
    key(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(sel.get(), Some(2), "Portugal and Peru match; the second is Peru");
}

#[test]
fn spinner_and_carousel_schedule_frames() {
    let mut ui = ui(VStack::new((
        Spinner::new(),
        Carousel::new(vec![Box::new(Text::new("One")), Box::new(Text::new("Two"))]).autoplay(3.0),
    )));
    assert!(ui.next_wake().is_some_and(|w| w <= 1.0 / 30.0 + 1e-9), "the spinner animates");
    let runs = |ui: &Ui| {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| if let Primitive::Glyphs(g) = p { Some(g.glyphs.len()) } else { None })
            .collect::<Vec<_>>()
    };
    assert!(runs(&ui).contains(&3));
    ui.set_time(3.1);
    ui.update();
    ui.update();
    assert_eq!(
        runs(&ui).iter().filter(|&&n| n == 3).count(),
        1,
        "autoplay moved to slide two ('Two' has 3 glyphs too, 'One' is gone)"
    );
}

#[test]
fn splitter_drag_changes_the_ratio() {
    let mut ui =
        ui(VStack::new((Splitter::horizontal(Text::new("Left"), Text::new("Right")).height(200.0),)));
    // 900px wide, handle 10px at the middle of the remaining 890px.
    let handle = Point::new(445.0 + 5.0, 100.0);
    ui.handle_event(&Event::PointerMove { position: handle });
    assert_eq!(ui.cursor(), ravenrs_core::CursorIcon::ColResize);
    ui.handle_event(&Event::PointerDown { position: handle, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: Point::new(200.0, 100.0) });
    ui.handle_event(&Event::PointerUp { position: Point::new(200.0, 100.0), button: PointerButton::Primary });
    ui.update();
    ui.handle_event(&Event::PointerMove { position: Point::new(200.0, 100.0) });
    assert_eq!(ui.cursor(), ravenrs_core::CursorIcon::ColResize, "the handle followed the pointer");
}

#[test]
fn otp_accepts_digits_and_completes() {
    let code = State::new(String::new());
    let done = Rc::new(Cell::new(false));
    let d = done.clone();
    let mut ui = ui(VStack::new((InputOtp::new(code.binding(), 4).on_complete(move |_| d.set(true)),)));
    click(&mut ui, Point::new(20.0, 20.0));
    typ(&mut ui, "1a2");
    key(&mut ui, Key::Backspace, Modifiers::default());
    typ(&mut ui, "345");
    assert_eq!(code.get(), "1345");
    assert!(done.get());
}

#[test]
fn form_field_shows_validation_errors() {
    let email = State::new(String::new());
    let field = || {
        FormField::new("Email", Text::new("(field)")).required().validate(email.binding(), validate::email)
    };
    assert_eq!(field().error().as_deref(), Some("This field is required."));
    email.set("ana@raven".into());
    assert_eq!(field().error().as_deref(), Some("Enter an email like name@example.com."));
    email.set("ana@raven.rs".into());
    assert_eq!(field().error(), None);
}
