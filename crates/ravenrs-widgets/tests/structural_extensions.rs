//! The structural extensions of existing components: each test fails if its feature is removed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_core::{
    AccessNode, AccessTree, Event, Key, Modifiers, Point, PointerButton, Primitive, Quad, Role, Size,
};
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
}

fn click(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn hover(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
}

fn named<'a>(tree: &'a AccessTree, role: Role, name: &str) -> &'a AccessNode {
    tree.find(role, name).unwrap_or_else(|| {
        let all: Vec<_> = tree.nodes.iter().map(|n| format!("{:?} {:?}", n.role, n.name)).collect();
        panic!("no {role:?} named {name:?} in {all:#?}")
    })
}

fn click_node(ui: &mut Ui, role: Role, name: &str) {
    let tree = ui.access_tree();
    let at = named(&tree, role, name).bounds.center();
    click(ui, at);
}

fn quads(ui: &Ui) -> Vec<Quad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect()
}

fn shows(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(text))
}

#[test]
fn a_disclosure_kept_by_the_app_opens_and_closes_through_its_binding() {
    let open = State::new(false);
    let mut ui = ui(Disclosure::new("Effects", Text::new("Grain")).expanded(open.binding()));
    assert!(!shows(&mut ui, "Grain"));
    click_node(&mut ui, Role::Button, "Effects");
    assert!(open.get(), "a click writes the binding");

    open.set(false);
    ui.update();
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Button, "Effects").expanded, Some(false), "and the binding decides");
    open.set(true);
    ui.update();
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Button, "Effects").expanded, Some(true));
}

#[test]
fn a_disclosure_header_takes_a_view_a_mark_and_a_switch() {
    let (on, open) = (State::new(false), State::new(false));
    let mut ui = ui(Disclosure::new("Effects", Text::new("Grain"))
        .title_view(Dot::new().label("Rich title"))
        .mark(Dot::new().tone(Tone::Warning).label("Changed"))
        .switch(on.binding())
        .expanded(open.binding()));
    assert!(shows(&mut ui, "Rich title"));
    assert!(shows(&mut ui, "Changed"), "the mark is told while it is closed");
    let tree = ui.access_tree();
    let header = named(&tree, Role::Button, "Effects").bounds;
    let switch = named(&tree, Role::Switch, "Effects").bounds;
    assert!(switch.max_x() > header.center().x, "the switch sits at the far end");
    click(&mut ui, switch.center());
    assert!(on.get(), "the switch switches");
    assert!(!open.get(), "and leaves the disclosure as it was");
}

#[test]
fn an_accordion_keeps_a_keyed_disclosure_state_when_the_list_changes() {
    let first = State::new(true);
    let flag = first.clone();
    let mut ui = ui(ravenrs_widgets::responsive(move |_| {
        let mut items = vec![Disclosure::new("B", Text::new("Inside B")).key(2)];
        if flag.get() {
            items.insert(0, Disclosure::new("A", Text::new("Inside A")).key(1));
        }
        Accordion::new(items)
    }));
    click_node(&mut ui, Role::Button, "B");
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Button, "B").expanded, Some(true));
    first.set(false);
    ui.update();
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Button, "B").expanded, Some(true), "B is still open after A left");
}

#[test]
fn a_section_puts_its_actions_at_the_end_of_the_header() {
    let clicks = Rc::new(Cell::new(0));
    let count = clicks.clone();
    let mut ui = ui(VStack::new((Section::new("Notifications", (Text::new("Body"),))
        .action(Button::new("Edit").on_click(move || count.set(count.get() + 1))),))
    .align(ravenrs_layout::Align::Stretch)
    .width(500.0));
    let tree = ui.access_tree();
    let (title, edit) =
        (named(&tree, Role::Heading, "Notifications").bounds, named(&tree, Role::Button, "Edit").bounds);
    assert!(edit.x() > title.max_x() + 100.0, "{edit:?} after {title:?}");
    assert!(edit.y() < title.max_y(), "on the same line");
    click(&mut ui, edit.center());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn a_selected_card_says_so_and_shows_it_beyond_color() {
    let border_width =
        |ui: &Ui| quads(ui).iter().find_map(|q| q.border).map(|b| b.width).expect("the card has a border");
    let mut plain = ui(Card::new(Text::new("One")));
    let mut chosen = ui(Card::new(Text::new("One")).selected(true).on_click(|| {}));
    assert_eq!(border_width(&plain), 1.0);
    assert_eq!(border_width(&chosen), 2.0, "thicker, not only another color");
    let accent = Theme::raven_dark().color(Semantic::Accent);
    assert!(quads(&chosen).iter().any(|q| q.border.is_some_and(|b| b.color == accent)));
    let tree = chosen.access_tree();
    assert!(tree.nodes.iter().any(|n| n.role == Role::Button && n.selected == Some(true)));
    assert!(plain.access_tree().nodes.iter().all(|n| n.selected.is_none()));

    let mut alone = ui(Card::new(Text::new("One")).selected(true));
    assert!(alone.access_tree().nodes.iter().any(|n| n.role == Role::Group && n.selected == Some(true)));
}

#[test]
fn a_breadcrumb_links_every_location_but_the_current_one() {
    let went = Rc::new(RefCell::new(Vec::new()));
    let log = went.clone();
    let mut ui = ui(Breadcrumb::new(["Archive", "2026", "Weddings"])
        .root_icon(IconName::Cloud)
        .on_navigate(move |i| log.borrow_mut().push(i)));
    click_node(&mut ui, Role::Link, "2026");
    assert_eq!(*went.borrow(), [1]);
    let tree = ui.access_tree();
    assert!(tree.find(Role::Link, "Weddings").is_none(), "the current page is not a link");
    assert!(named(&tree, Role::Link, "Archive").focusable);
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Enter);
    assert_eq!(*went.borrow(), [1, 0], "the keyboard goes too");

    let mut plain = self::ui(Breadcrumb::new(["Archive", "2026"]));
    assert!(plain.access_tree().nodes.iter().all(|n| n.role != Role::Link), "no links by default");
}

#[test]
fn segmented_tabs_change_their_look_and_size_and_work_like_the_others() {
    let chosen = State::new(0);
    let height = |size| {
        let picked = State::new(0);
        let mut ui = ui(Tabs::new(picked.binding()).variant(TabsVariant::Segmented).size(size).item("One"));
        let tree = ui.access_tree();
        tree.nodes.iter().find(|n| n.role == Role::TabList).unwrap().bounds.height()
    };
    assert_eq!(height(ButtonSize::Small), 32.0);
    assert_eq!(height(ButtonSize::Medium), 40.0);
    let mut seg = ui(Tabs::new(chosen.binding())
        .variant(TabsVariant::Segmented)
        .tab("One", Text::new("First"))
        .tab("Two", Text::new("Second")));
    assert!(shows(&mut seg, "First"));
    click_node(&mut seg, Role::Tab, "Two");
    assert_eq!(chosen.get(), 1);
    assert!(shows(&mut seg, "Second"));
    key(&mut seg, Key::ArrowLeft);
    assert_eq!(chosen.get(), 0, "the arrows move along the bar");
    let tree = seg.access_tree();
    assert_eq!(named(&tree, Role::Tab, "One").selected, Some(true));
}

#[test]
fn tabs_without_content_are_only_the_bar_and_the_app_holds_the_selection() {
    let chosen = State::new(0);
    let log = chosen.clone();
    // The app decides: asking for the third tab is refused.
    let selected = ravenrs_reactive::Binding::new(
        {
            let chosen = chosen.clone();
            move || chosen.get()
        },
        move |i| {
            if i < 2 {
                log.set(i);
            }
        },
    );
    let mut ui = ui(Tabs::new(selected).item("One").item("Two").item("Locked"));
    assert!(ui.access_tree().nodes.iter().all(|n| n.role != Role::TabPanel), "no panel");
    click_node(&mut ui, Role::Tab, "Two");
    assert_eq!(chosen.get(), 1);
    click_node(&mut ui, Role::Tab, "Locked");
    assert_eq!(chosen.get(), 1);
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Tab, "Two").selected, Some(true));
    assert_eq!(named(&tree, Role::Tab, "Locked").selected, Some(false));
}

#[test]
fn disabled_tabs_ignore_the_pointer_and_the_keyboard() {
    let chosen = State::new(0);
    let mut ui = ui(Tabs::new(chosen.binding()).disabled(true).item("One").item("Two"));
    click_node(&mut ui, Role::Tab, "Two");
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(chosen.get(), 0);
    let tree = ui.access_tree();
    assert!(named(&tree, Role::Tab, "Two").disabled);
    assert!(!named(&tree, Role::Tab, "Two").focusable);
}

fn dialog_in_a_page(dialog: impl Fn(Dialog) -> Dialog + 'static) -> (Ui, State<bool>, Rc<Cell<u32>>) {
    let (open, behind) = (State::new(true), Rc::new(Cell::new(0)));
    let (flag, count) = (open.clone(), behind.clone());
    let page = VStack::new((
        Button::new("Behind").on_click(move || count.set(count.get() + 1)),
        dialog(Dialog::new(flag.binding(), "Sure?")),
    ));
    (ui(page), open, behind)
}

#[test]
fn a_dialog_closes_by_all_three_ways_by_default() {
    let (mut by_escape, open, _) = dialog_in_a_page(|d| d);
    key(&mut by_escape, Key::Escape);
    assert!(!open.get(), "Escape");
    let (mut by_x, open, _) = dialog_in_a_page(|d| d);
    click_node(&mut by_x, Role::Button, "Close");
    assert!(!open.get(), "the X");
    let (mut by_backdrop, open, _) = dialog_in_a_page(|d| d);
    click(&mut by_backdrop, Point::new(5.0, 300.0));
    assert!(!open.get(), "the backdrop");
}

#[test]
fn each_way_of_closing_a_dialog_turns_off_by_itself() {
    let (mut ui, open, _) = dialog_in_a_page(|d| d.close_on_escape(false));
    key(&mut ui, Key::Escape);
    assert!(open.get(), "Escape does nothing");
    click(&mut ui, Point::new(5.0, 300.0));
    assert!(!open.get(), "the backdrop still closes");

    let (mut ui, open, behind) = dialog_in_a_page(|d| d.close_on_backdrop(false));
    click(&mut ui, Point::new(5.0, 10.0));
    assert!(open.get(), "the backdrop does nothing");
    assert_eq!(behind.get(), 0, "and the click does not reach what is behind");
    key(&mut ui, Key::Escape);
    assert!(!open.get(), "Escape still closes");

    let (mut ui, open, _) = dialog_in_a_page(|d| d.close_button(false));
    assert!(ui.access_tree().find(Role::Button, "Close").is_none(), "no X");
    key(&mut ui, Key::Escape);
    assert!(!open.get());
}

fn toast_card(ui: &mut Ui) -> ravenrs_core::Rect {
    let tree = ui.access_tree();
    tree.nodes.iter().find(|n| n.role == Role::Status).expect("a toast").bounds
}

#[test]
fn a_toaster_stacks_where_it_is_told_and_a_toast_can_go_elsewhere() {
    let toasts = Toasts::new();
    let mut top = ui(Toaster::new(toasts.clone()).position(ToastPosition::TopCenter));
    toasts.show("Saved", "", Tone::Success);
    top.update();
    let card = toast_card(&mut top);
    assert!(card.y() < 40.0, "at the top: {card:?}");
    assert!((card.center().x - 400.0).abs() < 1.0, "in the middle: {card:?}");

    let toasts = Toasts::new();
    let mut corner = ui(Toaster::new(toasts.clone()).position(ToastPosition::TopLeft));
    toasts.show_with("Slow", "", Tone::Warning, 8.0, ToastExtras::new().position(ToastPosition::BottomRight));
    corner.update();
    let card = toast_card(&mut corner);
    assert!(card.max_x() > 700.0 && card.max_y() > 500.0, "where it asked: {card:?}");
    toasts.show("Saved", "", Tone::Success);
    corner.update();
    let tree = corner.access_tree();
    let cards: Vec<_> = tree.nodes.iter().filter(|n| n.role == Role::Status).map(|n| n.bounds).collect();
    assert!(cards.iter().any(|c| c.x() < 40.0 && c.y() < 40.0), "and the others stay: {cards:?}");
    assert!(cards.iter().any(|c| c.max_y() > 500.0), "{cards:?}");
}

#[test]
fn a_toast_can_stay_until_closed_without_keeping_the_app_awake() {
    let toasts = Toasts::new();
    let mut ui = ui(Toaster::new(toasts.clone()));
    ui.set_time(10.0);
    toasts.show_until_closed("Sending", "Three files left.", Tone::Accent);
    ui.update();
    ui.set_time(1.0e6);
    ui.update();
    ui.update();
    assert_eq!(toasts.len(), 1, "time does not take it");
    assert!(ui.next_wake().is_none(), "and nothing wakes for it");
    click_node(&mut ui, Role::Button, "Dismiss");
    assert!(toasts.is_empty(), "a person does");
}

#[test]
fn a_popover_opens_where_it_is_told_with_or_without_its_panel() {
    let open = State::new(true);
    let page = |placement, surface: bool| {
        let open = open.clone();
        VStack::new((
            Spacer::new().height(200.0),
            Popover::new(open.binding(), Button::new("Anchor"), Dot::new().label("Inside").size(40.0))
                .placement(placement)
                .surface(surface),
        ))
    };
    let frames = |ui: &mut Ui| {
        let tree = ui.access_tree();
        (named(&tree, Role::Button, "Anchor").bounds, named(&tree, Role::Image, "Inside").bounds)
    };
    let mut below = ui(page(Placement::Below, true));
    let (anchor, inside) = frames(&mut below);
    assert!(inside.y() >= anchor.max_y(), "below by default");
    let panel_quads = quads(&below).len();
    let mut above = ui(page(Placement::Above, true));
    let (anchor, inside) = frames(&mut above);
    assert!(inside.max_y() <= anchor.y(), "above when asked");
    let mut right = ui(page(Placement::Right, true));
    let (anchor, inside) = frames(&mut right);
    assert!(inside.x() >= anchor.max_x(), "at the side when asked");
    let mut bare = ui(page(Placement::Below, false));
    assert!(quads(&bare).len() < panel_quads, "no panel without the surface");
    let (anchor, inside) = frames(&mut bare);
    assert!(inside.y() - anchor.max_y() < 12.0, "and no padding around the content");
    key(&mut bare, Key::Escape);
    assert!(!open.get(), "Escape still closes it");
}

#[test]
fn a_right_layer_changes_sides_only_when_it_does_not_fit() {
    let open = State::new(true);
    let page = |gap: f32| {
        HStack::new((
            Spacer::new().width(gap),
            Popover::new(open.binding(), Button::new("Anchor"), Dot::new().label("Inside").size(40.0))
                .placement(Placement::Right)
                .min_width(300.0),
        ))
    };
    let frames = |ui: &mut Ui| {
        let tree = ui.access_tree();
        (named(&tree, Role::Button, "Anchor").bounds, named(&tree, Role::Image, "Inside").bounds)
    };
    let (anchor, inside) = frames(&mut ui(page(0.0)));
    assert!(inside.x() >= anchor.max_x(), "room at the right");
    let (anchor, inside) = frames(&mut ui(page(650.0)));
    assert!(inside.max_x() <= anchor.x(), "none at the right: {anchor:?} {inside:?}");
}

fn menu_with_submenu() -> DropdownMenu {
    DropdownMenu::new("File")
        .item(MenuItem::new("New"))
        .item(MenuItem::new("Export").item(MenuItem::new("As PDF")).item(MenuItem::new("As CSV")))
        .item(MenuItem::new("Print"))
}

fn open_export(ui: &mut Ui) -> (ravenrs_core::Rect, ravenrs_core::Rect) {
    click_node(ui, Role::Button, "File");
    let tree = ui.access_tree();
    let (export, print) =
        (named(&tree, Role::MenuItem, "Export").bounds, named(&tree, Role::MenuItem, "Print").bounds);
    hover(ui, export.center());
    assert!(shows(ui, "As PDF"));
    (export, print)
}

#[test]
fn the_pointer_can_cross_another_row_on_its_way_to_a_submenu() {
    let mut ui = ui(menu_with_submenu());
    let (export, print) = open_export(&mut ui);
    // Down and to the right: the row under the pointer changes, the aim does not.
    let crossing = Point::new(export.center().x + 40.0, print.center().y);
    hover(&mut ui, crossing);
    assert!(shows(&mut ui, "As PDF"), "still open while crossing");
    let tree = ui.access_tree();
    let inside = named(&tree, Role::MenuItem, "As CSV").bounds.center();
    hover(&mut ui, inside);
    ui.set_time(5.0);
    ui.update();
    ui.update();
    assert!(shows(&mut ui, "As PDF"), "and stays once the pointer gets there");
}

#[test]
fn a_pointer_that_stops_on_another_row_closes_the_submenu_after_a_moment() {
    let mut ui = ui(menu_with_submenu());
    ui.set_time(1.0);
    let (export, print) = open_export(&mut ui);
    hover(&mut ui, Point::new(export.center().x + 40.0, print.center().y));
    let due = ui.next_wake().expect("a moment to get there");
    assert!(due > 1.0 && due <= 1.5, "short: {due}");
    assert!(shows(&mut ui, "As PDF"));
    ui.set_time(due);
    ui.update();
    ui.update();
    assert!(!shows(&mut ui, "As PDF"), "the row it stopped on takes over");
    assert_eq!(ui.next_wake(), None, "and nothing is left waiting");
}

#[test]
fn going_straight_down_closes_a_submenu_at_once() {
    let mut ui = ui(menu_with_submenu());
    let (_, print) = open_export(&mut ui);
    hover(&mut ui, Point::new(print.center().x, print.center().y));
    // The centers of the two rows line up, so this move has no sideways part.
    assert!(!shows(&mut ui, "As PDF"));
}

#[test]
fn a_submenu_opens_on_the_left_at_the_edge_of_the_window() {
    let mut ui = ui(HStack::new((Spacer::new().width(600.0), menu_with_submenu())));
    let (export, _) = open_export(&mut ui);
    let tree = ui.access_tree();
    let pdf = named(&tree, Role::MenuItem, "As PDF").bounds;
    assert!(pdf.max_x() <= export.x(), "{pdf:?} left of {export:?}");

    let mut room = self::ui(menu_with_submenu());
    let (export, _) = open_export(&mut room);
    let tree = room.access_tree();
    assert!(named(&tree, Role::MenuItem, "As PDF").bounds.x() >= export.max_x(), "right when it fits");
}
