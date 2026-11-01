//! `Sidebar`: closing to icons or off the screen, its parts, the sheet in a
//! narrow window, and the keyboard.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{AccessNode, AccessTree, Event, Key, Modifiers, Point, PointerButton, Rect, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(root: impl View + 'static, width: f32) -> Ui {
    let mut theme = Theme::raven_dark();
    // Layout settles at once, so each test reads where things end up.
    theme.interaction.reduced_motion = true;
    let mut ui = Ui::new(root, theme);
    ui.set_viewport(Size::new(width, 600.0), 1.0);
    ui.update();
    ui
}

fn node<'a>(tree: &'a AccessTree, name: &str) -> Option<&'a AccessNode> {
    tree.nodes.iter().find(|n| n.name.as_deref() == Some(name))
}

fn bounds(ui: &mut Ui, name: &str) -> Rect {
    node(&ui.access_tree(), name).expect(name).bounds
}

fn exists(ui: &mut Ui, name: &str) -> bool {
    node(&ui.access_tree(), name).is_some()
}

fn press(ui: &mut Ui, key: Key, modifiers: Modifiers) {
    ui.handle_event(&Event::KeyDown { key, modifiers });
    ui.update();
    ui.update();
}

fn tab(ui: &mut Ui) {
    press(ui, Key::Tab, Modifiers::default());
}

fn arrow(ui: &mut Ui, key: Key) {
    press(ui, key, Modifiers::default());
}

fn command_b(ui: &mut Ui) {
    let modifiers = Modifiers { meta: true, ..Modifiers::default() };
    press(ui, Key::Character("b".into()), modifiers);
}

fn hover(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
}

fn click(ui: &mut Ui, at: Point) {
    hover(ui, at);
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

fn focused_name(ui: &mut Ui) -> Option<String> {
    let tree = ui.access_tree();
    tree.focus.and_then(|id| tree.get(id)).and_then(|n| n.name.clone())
}

fn rows() -> (SidebarItem, SidebarItem) {
    (
        SidebarItem::new(IconName::House, "Home").selected(true).count("3"),
        SidebarItem::new(IconName::Settings, "Settings"),
    )
}

fn page(sidebar: impl View + 'static) -> impl View {
    HStack::new((sidebar, Text::new("Page").grow_text())).height(600.0).align(ravenrs_layout::Align::Stretch)
}

trait GrowText {
    fn grow_text(self) -> VStack<(Text,)>;
}

impl GrowText for Text {
    fn grow_text(self) -> VStack<(Text,)> {
        VStack::new((self,)).grow(1.0)
    }
}

#[test]
fn a_plain_sidebar_stays_open_and_ignores_the_shortcut() {
    let mut ui = ui(page(Sidebar::new(rows())), 800.0);
    command_b(&mut ui);
    assert_eq!(bounds(&mut ui, "Navigation").width(), 248.0);
    assert!(exists(&mut ui, "Home"));
}

#[test]
fn it_closes_to_icons_with_the_names_kept_and_the_icons_in_place() {
    let state = SidebarState::new(true);
    let bar = Sidebar::new(rows()).collapsible(SidebarCollapse::Icons).state(state.clone());
    let mut ui = ui(page(bar), 800.0);
    let open = bounds(&mut ui, "Home");
    assert!(ui.access_tree().nodes.iter().any(|n| n.role == Role::Navigation));

    command_b(&mut ui);
    assert!(!state.is_open());
    assert_eq!(bounds(&mut ui, "Navigation").width(), 60.0);
    let closed = bounds(&mut ui, "Home");
    assert_eq!((closed.x(), closed.width()), (open.x(), 36.0));
    let tree = ui.access_tree();
    assert_eq!(node(&tree, "Home").and_then(|n| n.description.as_deref()), Some("3"));

    command_b(&mut ui);
    assert!(state.is_open());
    assert_eq!(bounds(&mut ui, "Navigation").width(), 248.0);
}

#[test]
fn reduced_to_icons_a_tooltip_shows_on_hover_and_on_keyboard_focus() {
    let closed = SidebarState::new(false);
    let bar = Sidebar::new(rows()).collapsible(SidebarCollapse::Icons).state(closed);
    let mut ui = ui(page(bar), 800.0);
    let glyphs = |ui: &Ui| {
        ui.scene().primitives().iter().filter(|p| matches!(p, ravenrs_core::Primitive::Glyphs(_))).count()
    };
    let idle = glyphs(&ui);
    let home = bounds(&mut ui, "Home");
    hover(&mut ui, home.center());
    assert!(glyphs(&ui) > idle, "hover shows the label");
    hover(&mut ui, Point::new(500.0, 500.0));
    assert_eq!(glyphs(&ui), idle);
    tab(&mut ui);
    assert!(glyphs(&ui) > idle, "keyboard focus shows the label");
}

#[test]
fn hiding_takes_it_out_of_the_tree_and_out_of_the_tab_order() {
    let state = SidebarState::new(true);
    let bar = Sidebar::new(rows()).collapsible(SidebarCollapse::Hidden).state(state.clone());
    let trigger = SidebarTrigger::new(state.clone());
    let mut ui = ui(HStack::new((bar, trigger)), 800.0);
    assert!(exists(&mut ui, "Home"));
    let tree = ui.access_tree();
    assert_eq!(node(&tree, "Toggle sidebar").and_then(|n| n.expanded), Some(true));

    state.set_open(false);
    ui.update();
    assert!(!exists(&mut ui, "Home"));
    assert!(!exists(&mut ui, "Navigation"));
    tab(&mut ui);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Toggle sidebar"));
    let tree = ui.access_tree();
    assert_eq!(node(&tree, "Toggle sidebar").and_then(|n| n.expanded), Some(false));
    arrow(&mut ui, Key::Enter);
    assert!(state.is_open());
}

#[test]
fn a_controlled_state_follows_the_binding_both_ways() {
    let open = State::new(true);
    let state = SidebarState::controlled(open.binding());
    let bar = Sidebar::new(rows()).collapsible(SidebarCollapse::Icons).state(state);
    let mut ui = ui(page(bar), 800.0);
    open.set(false);
    ui.update();
    assert_eq!(bounds(&mut ui, "Navigation").width(), 60.0);
    command_b(&mut ui);
    assert!(open.get());
}

#[test]
fn the_shortcut_can_be_turned_off() {
    let state = SidebarState::new(true);
    let bar = Sidebar::new(rows()).collapsible(SidebarCollapse::Icons).state(state.clone()).shortcut(false);
    let mut ui = ui(page(bar), 800.0);
    command_b(&mut ui);
    assert!(state.is_open());
}

#[test]
fn header_and_footer_stay_while_the_content_scrolls() {
    let many: Vec<SidebarItem> =
        (0..60).map(|i| SidebarItem::new(IconName::House, format!("Row {i}"))).collect();
    let bar = Sidebar::new(many)
        .header(Button::new("Top").on_click(|| {}))
        .footer(Button::new("Bottom").on_click(|| {}));
    let mut ui = ui(page(bar), 800.0);
    let top = bounds(&mut ui, "Top");
    let bottom = bounds(&mut ui, "Bottom");
    assert!(top.y() < 20.0);
    assert!(bottom.max_y() <= 600.0 && bottom.max_y() > 560.0, "{bottom:?}");
    let first = bounds(&mut ui, "Row 0");
    ui.handle_event(&Event::Scroll { position: first.center(), delta: Point::new(0.0, -300.0) });
    ui.update();
    assert!(bounds(&mut ui, "Row 0").y() < first.y(), "the rows scroll");
    assert_eq!(bounds(&mut ui, "Top"), top);
    assert_eq!(bounds(&mut ui, "Bottom"), bottom);
}

#[test]
fn a_collapsible_group_folds_by_click_and_by_arrows_and_its_action_works() {
    let added = Rc::new(Cell::new(0));
    let add = added.clone();
    let group = SidebarGroup::new("Projects", (SidebarItem::new(IconName::House, "Alpha"),))
        .collapsible()
        .action(IconButton::new(IconName::Plus).label("Add").on_click(move || add.set(add.get() + 1)));
    let mut ui = ui(page(Sidebar::new((group,))), 800.0);
    let tree = ui.access_tree();
    let title = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some("Projects"));
    assert_eq!(title.expect("title").expanded, Some(true));
    assert!(exists(&mut ui, "Alpha"));

    let add = bounds(&mut ui, "Add");
    click(&mut ui, add.center());
    assert_eq!(added.get(), 1, "the action does not fold the group");
    assert!(exists(&mut ui, "Alpha"));

    let at = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Projects"))
        .expect("title")
        .bounds
        .center();
    click(&mut ui, at);
    assert!(!exists(&mut ui, "Alpha"));

    // The click left focus on the caption.
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Projects"));
    arrow(&mut ui, Key::ArrowRight);
    assert!(exists(&mut ui, "Alpha"));
    arrow(&mut ui, Key::ArrowLeft);
    assert!(!exists(&mut ui, "Alpha"));
}

#[test]
fn a_group_is_always_unfolded_when_reduced_to_icons() {
    let group = SidebarGroup::new("Projects", (SidebarItem::new(IconName::House, "Alpha"),))
        .collapsible()
        .open(false);
    let state = SidebarState::new(true);
    let bar = Sidebar::new((group,)).collapsible(SidebarCollapse::Icons).state(state.clone());
    let mut ui = ui(page(bar), 800.0);
    assert!(!exists(&mut ui, "Alpha"));
    state.set_open(false);
    ui.update();
    assert!(exists(&mut ui, "Alpha"));
}

#[test]
fn a_submenu_unfolds_nests_and_starts_open_around_the_selected_item() {
    let picked = Rc::new(Cell::new(false));
    let p = picked.clone();
    let settings = SidebarItem::new(IconName::Settings, "Settings")
        .item(SidebarItem::text("General").on_click(move || p.set(true)))
        .item(SidebarItem::text("More").item(SidebarItem::text("Deep")));
    let opened = SidebarItem::new(IconName::Users, "People").item(SidebarItem::text("Ana").selected(true));
    let mut ui = ui(page(Sidebar::new((settings, opened))), 800.0);
    assert!(!exists(&mut ui, "General"));
    assert!(exists(&mut ui, "Ana"), "holds the selected item");
    let row = bounds(&mut ui, "Settings");
    click(&mut ui, row.center());
    let general = bounds(&mut ui, "General");
    assert!(general.x() > row.x(), "indented");
    click(&mut ui, general.center());
    assert!(picked.get());
    let more = bounds(&mut ui, "More");
    click(&mut ui, more.center());
    assert!(bounds(&mut ui, "Deep").x() > more.x());
}

#[test]
fn clicking_a_submenu_row_reduced_to_icons_opens_the_sidebar_and_the_submenu() {
    let state = SidebarState::new(false);
    let settings = SidebarItem::new(IconName::Settings, "Settings").item(SidebarItem::text("General"));
    let bar = Sidebar::new((settings,)).collapsible(SidebarCollapse::Icons).state(state.clone());
    let mut ui = ui(page(bar), 800.0);
    assert!(!exists(&mut ui, "General"));
    let row = bounds(&mut ui, "Settings");
    click(&mut ui, row.center());
    assert!(state.is_open());
    assert!(exists(&mut ui, "General"));
}

#[test]
fn an_action_sits_beside_the_row_as_its_own_control() {
    let (opened, acted) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (o, a) = (opened.clone(), acted.clone());
    let item = SidebarItem::new(IconName::Users, "People")
        .on_click(move || o.set(o.get() + 1))
        .action(IconButton::new(IconName::Plus).label("Invite").on_click(move || a.set(a.get() + 1)));
    let mut ui = ui(page(Sidebar::new((item,))), 800.0);
    let (row, action) = (bounds(&mut ui, "People"), bounds(&mut ui, "Invite"));
    assert!(row.max_x() <= action.x(), "side by side: {row:?} {action:?}");
    click(&mut ui, action.center());
    assert_eq!((opened.get(), acted.get()), (0, 1));
    click(&mut ui, row.center());
    assert_eq!((opened.get(), acted.get()), (1, 1));
}

#[test]
fn a_badge_is_read_with_the_row_and_a_long_label_stays_inside() {
    let long = "A label far too long to fit in the column ".repeat(4);
    let item = SidebarItem::new(IconName::Users, long.clone()).badge(Badge::new("New"));
    let mut ui = ui(page(Sidebar::new((item,))), 800.0);
    let tree = ui.access_tree();
    let row = node(&tree, &long).expect("row");
    assert_eq!(row.description.as_deref(), Some("New"));
    assert!(row.bounds.max_x() <= 248.0);
}

#[test]
fn a_disabled_row_takes_no_click_and_no_focus() {
    let clicked = Rc::new(Cell::new(false));
    let c = clicked.clone();
    let off = SidebarItem::new(IconName::Lock, "Off").disabled(true).on_click(move || c.set(true));
    let bar = Sidebar::new((off, SidebarItem::new(IconName::House, "On")));
    let mut ui = ui(page(bar), 800.0);
    let at = bounds(&mut ui, "Off").center();
    click(&mut ui, at);
    assert!(!clicked.get());
    let tree = ui.access_tree();
    assert!(node(&tree, "Off").expect("row").disabled);
    tab(&mut ui);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("On"));
}

#[test]
fn arrows_home_and_end_move_between_rows_and_wrap() {
    let bar = Sidebar::new((
        SidebarItem::new(IconName::House, "One"),
        SidebarSection::new("Caption"),
        SidebarItem::new(IconName::House, "Two"),
        SidebarItem::new(IconName::House, "Three"),
    ));
    let mut ui = ui(page(bar), 800.0);
    tab(&mut ui);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("One"));
    arrow(&mut ui, Key::ArrowDown);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Two"));
    arrow(&mut ui, Key::End);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Three"));
    arrow(&mut ui, Key::ArrowDown);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("One"));
    arrow(&mut ui, Key::ArrowUp);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Three"));
    arrow(&mut ui, Key::Home);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("One"));
}

#[test]
fn a_narrow_window_shows_it_as_a_sheet_that_closes_on_navigation() {
    let picked = Rc::new(Cell::new(false));
    let p = picked.clone();
    let state = SidebarState::new(true);
    let bar = Sidebar::new((SidebarItem::new(IconName::House, "Home").on_click(move || p.set(true)),))
        .narrow_below(768.0)
        .collapsible(SidebarCollapse::Icons)
        .state(state.clone());
    let trigger = SidebarTrigger::new(state.clone());
    let mut ui = ui(HStack::new((bar, trigger)), 500.0);
    assert!(state.is_narrow());
    assert!(!state.is_open(), "the column's open state does not open the sheet");
    assert!(!exists(&mut ui, "Home"));

    tab(&mut ui);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Toggle sidebar"));
    arrow(&mut ui, Key::Enter);
    let tree = ui.access_tree();
    let sheet = tree.nodes.iter().find(|n| n.role == Role::Dialog).expect("sheet");
    assert_eq!(sheet.name.as_deref(), Some("Navigation"));
    assert!(sheet.modal);

    arrow(&mut ui, Key::Escape);
    assert!(!state.is_open());
    assert!(!exists(&mut ui, "Home"));

    command_b(&mut ui);
    assert!(state.is_open());
    let home = bounds(&mut ui, "Home");
    click(&mut ui, home.center());
    assert!(picked.get());
    assert!(!state.is_open(), "going somewhere closes the sheet");

    state.set_open(true);
    ui.set_viewport(Size::new(1000.0, 600.0), 1.0);
    ui.update();
    assert!(!state.is_narrow());
    assert_eq!(bounds(&mut ui, "Navigation").width(), 248.0, "back to the column");
    ui.set_viewport(Size::new(500.0, 600.0), 1.0);
    ui.update();
    assert!(!state.is_open());
}

#[test]
fn the_right_side_puts_tooltips_and_the_sheet_on_that_side() {
    let state = SidebarState::new(true);
    let bar = Sidebar::new((SidebarItem::new(IconName::House, "Home"),))
        .side(Side::Right)
        .narrow_below(768.0)
        .state(state.clone());
    let mut ui = ui(HStack::new((VStack::new((Text::new("Page"),)).grow(1.0), bar)), 500.0);
    state.set_open(true);
    ui.update();
    let tree = ui.access_tree();
    let sheet = tree.nodes.iter().find(|n| n.role == Role::Dialog).expect("sheet");
    assert_eq!(sheet.bounds.max_x(), 500.0);
}

#[test]
fn floating_and_inset_keep_the_content_and_fill_the_room() {
    let mut floating = ui(page(Sidebar::new(rows()).variant(SidebarVariant::Floating)), 800.0);
    let card = bounds(&mut floating, "Navigation");
    assert_eq!((card.x(), card.y(), card.width()), (8.0, 8.0, 232.0));
    assert_eq!(card.height(), 584.0);

    let inset = HStack::new((
        Sidebar::new(rows()).variant(SidebarVariant::Inset),
        SidebarInset::new(Button::new("Save").on_click(|| {})),
    ))
    .align(ravenrs_layout::Align::Stretch);
    let mut inset = ui(inset, 800.0);
    assert_eq!(bounds(&mut inset, "Navigation").width(), 248.0);
    let save = bounds(&mut inset, "Save");
    assert!(save.x() >= 256.0, "{save:?}");
}

#[test]
fn a_skeleton_shows_rows_and_says_it_is_loading() {
    let bar = Sidebar::new((SidebarSkeleton::new(4),)).collapsible(SidebarCollapse::Icons);
    let mut ui = ui(page(bar), 800.0);
    let group = bounds(&mut ui, "Loading");
    assert_eq!(group.height(), 4.0 * 36.0 + 3.0 * 2.0);
}

#[test]
fn small_and_empty_cases_do_not_panic() {
    let empty: Vec<SidebarItem> = Vec::new();
    let mut a = ui(page(Sidebar::new(empty).collapsible(SidebarCollapse::Hidden)), 800.0);
    command_b(&mut a);
    arrow(&mut a, Key::ArrowDown);
    let empty: Vec<SidebarItem> = Vec::new();
    let mut b = ui(page(Sidebar::new(empty).width(0.0).narrow_below(10.0)), 0.0);
    b.update();
    let label: Vec<SidebarItem> = vec![SidebarItem::text("")];
    let closed = SidebarState::new(false);
    let mut c = ui(page(Sidebar::new(label).collapsible(SidebarCollapse::Icons).state(closed)), 800.0);
    c.update();
}

#[test]
fn closing_animates_the_width_and_keeps_the_labels_until_it_has_narrowed() {
    let state = SidebarState::new(true);
    let bar = Sidebar::new(rows()).collapsible(SidebarCollapse::Icons).state(state.clone());
    let mut ui = Ui::new(page(bar), Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    state.set_open(false);
    ui.update();
    let mut widths = Vec::new();
    for frame in 1..=120 {
        ui.set_time(f64::from(frame) / 60.0);
        ui.update();
        widths.push(bounds(&mut ui, "Navigation").width());
    }
    assert!(widths.iter().any(|w| *w > 60.0 && *w < 248.0), "it moves: {widths:?}");
    assert_eq!(widths.last(), Some(&60.0), "and it arrives");
}
