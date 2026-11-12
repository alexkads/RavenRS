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

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: to });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
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

#[test]
fn a_wizard_step_marked_failed_says_so_and_can_still_be_reopened() {
    let step = State::new(1);
    let mut ui = ui(Wizard::new(
        step.binding(),
        vec![
            WizardStep::new("Client", Text::new("One")).failed(true),
            WizardStep::new("Photos", Text::new("Two")),
            WizardStep::new("Send", Text::new("Three")).failed(true),
        ],
    ));
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Button, "Client").description.as_deref(), Some("Needs attention"));
    let later = named(&tree, Role::ListItem, "Send");
    assert_eq!(later.description.as_deref(), Some("Needs attention"), "ahead of the current one too");
    assert_eq!(named(&tree, Role::ListItem, "Photos").selected, Some(true));
    click_node(&mut ui, Role::Button, "Client");
    assert_eq!(step.get(), 0);
}

#[test]
fn a_step_indicator_shows_each_state_and_goes_to_a_step_when_asked() {
    let went = Rc::new(Cell::new(usize::MAX));
    let log = went.clone();
    let steps = || {
        vec![
            IndicatorStep::new("Client", StepState::Done),
            IndicatorStep::new("Photos", StepState::Error).note("Two are missing"),
            IndicatorStep::new("Review", StepState::Current),
        ]
    };
    let mut ui = self::ui(StepIndicator::new(steps()).vertical().on_select(move |i| log.set(i)));
    let tree = ui.access_tree();
    assert_eq!(
        tree.nodes.iter().find(|n| n.role == Role::List).unwrap().name.as_deref(),
        Some("Step 3 of 3")
    );
    assert_eq!(named(&tree, Role::Button, "Client").description.as_deref(), Some("Done"));
    assert_eq!(
        named(&tree, Role::Button, "Photos").description.as_deref(),
        Some("Two are missing, Needs attention"),
    );
    assert_eq!(named(&tree, Role::Button, "Review").selected, Some(true));
    let (client, photos) =
        (named(&tree, Role::Button, "Client").bounds, named(&tree, Role::Button, "Photos").bounds);
    assert!(photos.y() >= client.max_y(), "one under the other");
    click_node(&mut ui, Role::Button, "Photos");
    assert_eq!(went.get(), 1);
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Enter);
    assert_eq!(went.get(), 2, "the keyboard goes too");

    let mut still = self::ui(StepIndicator::new(steps()));
    let tree = still.access_tree();
    assert!(tree.find(Role::Button, "Client").is_none(), "only buttons when they lead somewhere");
    let (client, photos) =
        (named(&tree, Role::ListItem, "Client").bounds, named(&tree, Role::ListItem, "Photos").bounds);
    assert!(photos.x() >= client.max_x(), "a row by default");
}

#[test]
fn a_step_badge_differs_by_more_than_color() {
    let glyphs = |state| {
        let ui = ui(StepBadge::new(2).state(state));
        ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
    };
    let mut done = ui(StepBadge::new(2).state(StepState::Done));
    let tree = done.access_tree();
    assert_eq!(named(&tree, Role::Image, "2").description.as_deref(), Some("Done"));
    assert!(glyphs(StepState::Upcoming) >= 1);
    let border = |state| quads(&ui(StepBadge::new(2).state(state))).iter().any(|q| q.border.is_some());
    assert!(border(StepState::Error), "a ring");
    assert!(!border(StepState::Done));
}

fn rail(tool: &State<usize>) -> ToolRail {
    ToolRail::new(tool.binding(), [(IconName::Pencil, "Brush")])
        .separator()
        .tool(
            ToolRailItem::new(IconName::Square, "Rectangle")
                .shortcut("M")
                .variant(IconName::Circle, "Ellipse")
                .variant(IconName::Star, "Star"),
        )
        .tool(ToolRailItem::new(IconName::Type, "Text"))
}

fn button_at(ui: &mut Ui, name: &str) -> Point {
    let tree = ui.access_tree();
    named(&tree, Role::Button, name).bounds.center()
}

#[test]
fn the_tools_of_a_rail_are_counted_with_their_variants() {
    let tool = State::new(0);
    let mut ui = ui(rail(&tool));
    click_node(&mut ui, Role::Button, "Rectangle");
    assert_eq!(tool.get(), 1);
    click_node(&mut ui, Role::Button, "Text");
    assert_eq!(tool.get(), 4, "after the variants");
    tool.set(3);
    ui.update();
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Button, "Star").selected, Some(true), "the place shows the one in use");
    assert!(tree.find(Role::Button, "Rectangle").is_none());
}

#[test]
fn a_right_click_opens_the_variants_and_the_place_remembers_the_last_used() {
    let tool = State::new(0);
    let mut ui = ui(rail(&tool));
    let at = button_at(&mut ui, "Rectangle");
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Secondary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Secondary });
    ui.update();
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::MenuItem, "Rectangle").description.as_deref(), Some("M"));
    assert_eq!(named(&tree, Role::Button, "Rectangle").expanded, Some(true));
    assert_eq!(tool.get(), 0, "opening it chooses nothing");
    click_node(&mut ui, Role::MenuItem, "Ellipse");
    assert_eq!(tool.get(), 2);
    ui.update();
    click_node(&mut ui, Role::Button, "Brush");
    assert_eq!(tool.get(), 0);
    ui.update();
    assert!(button_at(&mut ui, "Ellipse").y > 0.0, "the place still shows the variant");
    click_node(&mut ui, Role::Button, "Ellipse");
    assert_eq!(tool.get(), 2, "and a click uses it again");
}

#[test]
fn holding_a_tool_opens_its_variants_and_the_release_does_not_choose() {
    let tool = State::new(0);
    let mut ui = ui(rail(&tool));
    ui.set_time(1.0);
    let at = button_at(&mut ui, "Rectangle");
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.update();
    let due = ui.next_wake().expect("waits for the hold");
    ui.set_time(due);
    ui.update();
    ui.update();
    assert!(ui.access_tree().find(Role::MenuItem, "Star").is_some(), "the hold opens the menu");
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
    assert_eq!(tool.get(), 0, "and letting go chooses nothing");
    assert!(ui.access_tree().find(Role::MenuItem, "Star").is_some(), "the menu stays");
    click_node(&mut ui, Role::MenuItem, "Star");
    assert_eq!(tool.get(), 3);

    // A quick press is a plain click.
    click_node(&mut ui, Role::Button, "Star");
    assert_eq!(tool.get(), 3);
    assert!(ui.access_tree().find(Role::MenuItem, "Rectangle").is_none());
}

#[test]
fn the_keyboard_opens_the_variants_of_a_tool_and_comes_back() {
    let tool = State::new(1);
    let mut ui = ui(rail(&tool));
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::Tab);
    assert_eq!(ui.access_tree().focused().and_then(|n| n.name.clone()).as_deref(), Some("Rectangle"));
    key(&mut ui, Key::ArrowRight);
    ui.update();
    assert!(ui.access_tree().find(Role::MenuItem, "Ellipse").is_some());
    key(&mut ui, Key::ArrowDown);
    key(&mut ui, Key::Enter);
    ui.update();
    assert_eq!(tool.get(), 2);
    assert_eq!(ui.access_tree().focused().and_then(|n| n.name.clone()).as_deref(), Some("Ellipse"));
}

#[test]
fn a_rail_can_show_only_icons_in_two_columns_with_groups_on_their_own_lines() {
    let tool = State::new(0);
    let mut ui = ui(ToolRail::new(
        tool.binding(),
        [(IconName::Pencil, "Brush"), (IconName::Type, "Text"), (IconName::Star, "Star")],
    )
    .separator()
    .tool(ToolRailItem::new(IconName::Circle, "Ellipse"))
    .columns(2));
    let tree = ui.access_tree();
    let frame = |name| named(&tree, Role::Button, name).bounds;
    assert_eq!(frame("Brush").y(), frame("Text").y(), "side by side");
    assert!(frame("Text").x() >= frame("Brush").max_x());
    assert!(frame("Star").y() >= frame("Brush").max_y(), "a new line after two");
    assert!(frame("Ellipse").y() >= frame("Star").max_y(), "a group starts on its own line");
    assert_eq!(frame("Brush").width(), frame("Brush").height(), "square, without the names under them");
}

fn dots(names: [&'static str; 3]) -> SplitPanes {
    let pane = |name: &'static str| SplitPane::new(HStack::new((Dot::new().label(name),)));
    SplitPanes::horizontal()
        .pane(pane(names[0]).size(200.0).range(100.0, 300.0))
        .pane(pane(names[1]).range(150.0, f32::MAX))
        .pane(pane(names[2]).size(200.0).range(100.0, 300.0))
        .height(300.0)
}

fn dot_x(ui: &mut Ui, name: &str) -> f32 {
    let tree = ui.access_tree();
    named(&tree, Role::Image, name).bounds.x()
}

fn bars(ui: &mut Ui) -> Vec<AccessNode> {
    ui.access_tree().nodes.into_iter().filter(|n| n.role == Role::Slider).collect()
}

#[test]
fn split_panes_lay_out_any_number_and_the_one_without_a_size_takes_the_rest() {
    let mut ui = ui(dots(["A", "B", "C"]));
    ui.update();
    let bars = bars(&mut ui);
    assert_eq!(bars.len(), 2, "one bar between each two");
    assert_eq!(dot_x(&mut ui, "A"), 0.0);
    assert_eq!(dot_x(&mut ui, "B"), 210.0);
    assert_eq!(dot_x(&mut ui, "C"), 600.0, "800 less the last pane");
    assert_eq!(bars[0].numeric.map(|n| n.value), Some(200.0));
}

#[test]
fn dragging_a_bar_of_split_panes_stops_at_the_limits() {
    let mut ui = ui(dots(["A", "B", "C"]));
    ui.update();
    let bar = bars(&mut ui)[0].bounds.center();
    drag(&mut ui, bar, Point::new(bar.x + 40.0, bar.y));
    assert_eq!(dot_x(&mut ui, "B"), 250.0, "the pane grew, and the middle gave way");
    let bar = bars(&mut ui)[0].bounds.center();
    drag(&mut ui, bar, Point::new(bar.x + 400.0, bar.y));
    assert_eq!(dot_x(&mut ui, "B"), 310.0, "not past its largest");
    let bar = bars(&mut ui)[1].bounds.center();
    drag(&mut ui, bar, Point::new(bar.x - 400.0, bar.y));
    assert_eq!(dot_x(&mut ui, "C"), 500.0, "the last one stops at its largest as well");
}

#[test]
fn a_narrow_window_takes_room_from_the_panes_down_to_their_smallest() {
    let mut ui = ui(dots(["A", "B", "C"]));
    ui.update();
    ui.set_viewport(Size::new(500.0, 600.0), 1.0);
    ui.update();
    ui.update();
    assert_eq!(dot_x(&mut ui, "A"), 0.0);
    let (b, c) = (dot_x(&mut ui, "B"), dot_x(&mut ui, "C"));
    assert_eq!(c, b + 150.0 + 10.0, "the middle at its smallest");
    assert!(c + 100.0 <= 500.0, "and all of it inside the window: {c}");
}

#[test]
fn the_keyboard_moves_the_bar_and_the_app_can_keep_the_sizes() {
    let (sizes, saved) = (State::new(vec![Some(220.0), None, Some(200.0)]), Rc::new(Cell::new(0)));
    let count = saved.clone();
    let mut ui = ui(dots(["A", "B", "C"]).sizes(sizes.binding()).on_end(move || count.set(count.get() + 1)));
    ui.update();
    assert_eq!(dot_x(&mut ui, "B"), 230.0, "the sizes it was given");
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::ArrowRight);
    assert_eq!(sizes.get()[0], Some(236.0));
    assert_eq!(saved.get(), 1, "told to save");
    ui.handle_event(&Event::KeyDown {
        key: Key::ArrowLeft,
        modifiers: Modifiers { shift: true, ..Modifiers::default() },
    });
    ui.update();
    assert_eq!(sizes.get()[0], Some(172.0), "Shift goes further");
    key(&mut ui, Key::Home);
    assert_eq!(sizes.get()[0], Some(100.0), "Home goes to the limit");
    key(&mut ui, Key::End);
    assert_eq!(sizes.get()[0], Some(300.0));
    assert_eq!(sizes.get()[1], None, "the middle goes on taking the rest");
}

#[test]
fn a_hidden_pane_takes_its_bar_with_it_and_keeps_its_size() {
    let hide = State::new(false);
    let flag = hide.clone();
    let mut ui = ui(ravenrs_widgets::responsive(move |_| {
        SplitPanes::horizontal()
            .pane(SplitPane::new(HStack::new((Dot::new().label("A"),))).size(200.0).visible(!flag.get()))
            .pane(SplitPane::new(HStack::new((Dot::new().label("B"),))))
            .height(300.0)
    }));
    ui.update();
    assert_eq!(bars(&mut ui).len(), 1);
    hide.set(true);
    ui.update();
    ui.update();
    assert!(bars(&mut ui).is_empty());
    assert_eq!(dot_x(&mut ui, "B"), 0.0);
    hide.set(false);
    ui.update();
    ui.update();
    assert_eq!(dot_x(&mut ui, "B"), 210.0);
}

#[test]
fn a_split_handle_changes_a_size_by_drag_and_key_within_its_range() {
    let height = State::new(120.0_f32);
    let mut ui = ui(VStack::new((
        Spacer::new().height(100.0),
        SplitHandle::new(ravenrs_layout::Axis::Vertical, height.binding()).range(80.0, 300.0).inverted(),
    ))
    .align(ravenrs_layout::Align::Stretch));
    let bar = bars(&mut ui)[0].clone();
    assert_eq!(bar.name.as_deref(), Some("Resize"));
    assert!(bar.focusable);
    let at = bar.bounds.center();
    drag(&mut ui, at, Point::new(at.x, at.y - 30.0));
    assert_eq!(height.get(), 150.0, "up makes what is below taller");
    drag(&mut ui, at, Point::new(at.x, at.y + 300.0));
    assert_eq!(height.get(), 80.0, "not past its smallest");
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::ArrowUp);
    assert_eq!(height.get(), 96.0, "the arrow moves the bar the way it points");
}

#[test]
fn the_divider_of_a_splitter_moves_with_the_keyboard_and_stays_in_its_limits() {
    let mut ui =
        ui(Splitter::horizontal(Dot::new().label("A"), Dot::new().label("B")).ratio(0.5).limits(0.4, 0.6));
    let start = dot_x(&mut ui, "B");
    key(&mut ui, Key::Tab);
    key(&mut ui, Key::ArrowRight);
    let moved = dot_x(&mut ui, "B");
    assert!(moved > start, "{start} to {moved}");
    key(&mut ui, Key::End);
    assert_eq!(bars(&mut ui)[0].numeric.map(|n| n.value as f32), Some(0.6));
    let bar = bars(&mut ui)[0].bounds.center();
    drag(&mut ui, bar, Point::new(10.0, bar.y));
    assert_eq!(bars(&mut ui)[0].numeric.map(|n| n.value as f32), Some(0.4), "the drag stops there too");
}

fn free_panel(open: &State<bool>, place: &State<Option<Point>>, minimized: &State<bool>) -> FloatingPanel {
    FloatingPanel::new(open.binding(), "Cash", || Box::new(Dot::new().label("Inside")))
        .size(300.0, 200.0)
        .movable(place.binding())
        .minimized(minimized.binding())
        .summary(|| Box::new(Dot::new().label("Open")))
}

fn panel_frame(ui: &mut Ui) -> ravenrs_core::Rect {
    let tree = ui.access_tree();
    named(&tree, Role::Group, "Cash").bounds
}

#[test]
fn a_movable_panel_has_no_sphere_and_opens_where_the_app_says() {
    let (open, place, small) =
        (State::new(false), State::new(Some(Point::new(40.0, 50.0))), State::new(false));
    let mut ui = ui(free_panel(&open, &place, &small));
    ui.update();
    assert!(ui.access_tree().nodes.iter().all(|n| n.name.as_deref() != Some("Cash")), "nothing while closed");
    open.set(true);
    ui.update();
    let frame = panel_frame(&mut ui);
    assert_eq!((frame.x(), frame.y()), (40.0, 50.0));
    place.set(None);
    ui.update();
    let frame = panel_frame(&mut ui);
    assert!(frame.max_x() > 700.0 && frame.max_y() > 500.0, "a corner without a place: {frame:?}");
    key(&mut ui, Key::Escape);
    assert!(!open.get(), "Escape closes it");
}

#[test]
fn dragging_a_movable_panel_writes_its_place_only_at_the_end_and_stays_in_the_window() {
    let (open, writes) = (State::new(true), Rc::new(Cell::new(0)));
    let store = State::new(Some(Point::new(100.0, 100.0)));
    let (read, write, count) = (store.clone(), store.clone(), writes.clone());
    let place = ravenrs_reactive::Binding::new(
        move || read.get(),
        move |p| {
            count.set(count.get() + 1);
            write.set(p);
        },
    );
    let small = State::new(false);
    let panel = FloatingPanel::new(open.binding(), "Cash", || Box::new(Dot::new().label("Inside")))
        .size(300.0, 200.0)
        .movable(place)
        .minimized(small.binding());
    let mut ui = ui(panel);
    ui.update();
    let header = named(&ui.access_tree(), Role::Button, "Move: Cash").bounds.center();
    ui.handle_event(&Event::PointerMove { position: header });
    ui.handle_event(&Event::PointerDown { position: header, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: Point::new(header.x + 50.0, header.y + 20.0) });
    ui.update();
    let frame = panel_frame(&mut ui);
    assert_eq!((frame.x(), frame.y()), (150.0, 120.0), "it follows the pointer");
    assert_eq!(writes.get(), 0, "and the app hears only at the end");
    ui.handle_event(&Event::PointerMove { position: Point::new(5000.0, header.y) });
    ui.update();
    ui.handle_event(&Event::PointerUp {
        position: Point::new(5000.0, header.y),
        button: PointerButton::Primary,
    });
    ui.update();
    assert_eq!(writes.get(), 1);
    assert_eq!(store.get(), Some(Point::new(496.0, 100.0)), "kept inside the window");

    ui.set_viewport(Size::new(600.0, 400.0), 1.0);
    ui.update();
    let frame = panel_frame(&mut ui);
    assert_eq!(frame.max_x(), 596.0, "a smaller window brings it back: {frame:?}");
    assert_eq!(store.get(), Some(Point::new(496.0, 100.0)), "without forgetting where it was");
}

#[test]
fn the_keyboard_moves_a_panel_by_its_header() {
    let (open, place, small) =
        (State::new(true), State::new(Some(Point::new(100.0, 100.0))), State::new(false));
    let mut ui = ui(free_panel(&open, &place, &small));
    ui.update();
    let tree = ui.access_tree();
    let header = named(&tree, Role::Button, "Move: Cash");
    assert!(header.focusable);
    let id = header.id;
    ui.perform_action(id, ravenrs_core::AccessAction::Focus);
    ui.update();
    key(&mut ui, Key::ArrowRight);
    assert_eq!(place.get(), Some(Point::new(116.0, 100.0)));
    ui.handle_event(&Event::KeyDown {
        key: Key::ArrowDown,
        modifiers: Modifiers { shift: true, ..Modifiers::default() },
    });
    ui.update();
    assert_eq!(place.get(), Some(Point::new(116.0, 164.0)), "Shift goes further");
}

#[test]
fn a_panel_reduces_to_its_header_and_comes_back() {
    let (open, place, small) =
        (State::new(true), State::new(Some(Point::new(100.0, 100.0))), State::new(false));
    let mut ui = ui(free_panel(&open, &place, &small));
    ui.update();
    assert!(shows(&mut ui, "Inside"));
    assert!(!shows(&mut ui, "Open"), "the summary is for when it is reduced");
    let tall = panel_frame(&mut ui).height();
    click_node(&mut ui, Role::Button, "Minimize");
    assert!(small.get());
    assert!(!shows(&mut ui, "Inside"), "the content goes");
    assert!(shows(&mut ui, "Open"), "the summary comes");
    assert!(panel_frame(&mut ui).height() < tall / 2.0);
    click_node(&mut ui, Role::Button, "Restore");
    assert!(!small.get());
    assert!(shows(&mut ui, "Inside"));
}

#[test]
fn a_panel_with_its_sphere_can_reduce_too() {
    let (open, small) = (State::new(true), State::new(false));
    let mut ui = ui(FloatingPanel::new(open.binding(), "Cash", || Box::new(Dot::new().label("Inside")))
        .minimized(small.binding()));
    ui.update();
    assert!(shows(&mut ui, "Inside"));
    click_node(&mut ui, Role::Button, "Minimize");
    assert!(!shows(&mut ui, "Inside"));
    let tree = ui.access_tree();
    assert!(tree.find(Role::Button, "Restore").is_some());
}

#[test]
fn a_column_can_keep_its_width() {
    let mut ui = ui(DataGrid::new(
        GridData::new(vec![vec![Value::Text("a".into()), Value::Text("b".into())]]),
        [GridColumn::text("Free").width(200.0), GridColumn::text("Fixed").width(100.0).resizable(false)],
    )
    .height(200.0));
    let tree = ui.access_tree();
    let header = tree.nodes.iter().find(|n| n.role == Role::ColumnHeader).expect("a header").bounds;
    // The handles are the last 6 pixels of each column.
    let first = named(&tree, Role::ColumnHeader, "Free").bounds.x() + 200.0 - 3.0;
    hover(&mut ui, Point::new(first, header.center().y));
    assert_eq!(ui.cursor(), ravenrs_core::CursorIcon::ColResize, "a handle");
    hover(&mut ui, Point::new(first + 100.0, header.center().y));
    assert_ne!(ui.cursor(), ravenrs_core::CursorIcon::ColResize, "no handle");

    drag(&mut ui, Point::new(first + 100.0, header.center().y), Point::new(first + 180.0, header.center().y));
    let tree = ui.access_tree();
    let (free, fixed) =
        (named(&tree, Role::ColumnHeader, "Free").bounds, named(&tree, Role::ColumnHeader, "Fixed").bounds);
    assert_eq!(fixed.x() - free.x(), 200.0, "dragging there does not change it");

    hover(&mut ui, Point::new(free.x() + 20.0, header.center().y));
    click_node(&mut ui, Role::Button, "Column menu Free");
    assert!(ui.access_tree().find(Role::MenuItem, "Fit width").is_some());
    key(&mut ui, Key::Escape);
    hover(&mut ui, Point::new(fixed.x() + 20.0, header.center().y));
    click_node(&mut ui, Role::Button, "Column menu Fixed");
    assert!(ui.access_tree().find(Role::MenuItem, "Fit width").is_none(), "nor does the menu offer it");
}

fn solid(shade: u8) -> ravenrs_core::ImageData {
    ravenrs_core::ImageData::from_rgba8(1, 1, vec![shade, shade, shade, 255]).expect("one pixel")
}

fn image_opacities(ui: &Ui) -> Vec<f32> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Image(i) = p { Some(i.opacity) } else { None })
        .collect()
}

fn swapping(crossfade: bool) -> (Ui, State<usize>) {
    let shown = State::new(0);
    let read = shown.clone();
    // The same pictures on every build: a new `ImageData` counts as a change of image.
    let pictures = [solid(10), solid(20)];
    let mut ui = ui(ravenrs_widgets::responsive(move |_| {
        let image = Image::new(pictures[read.get()].clone()).size(100.0, 100.0);
        if crossfade { image.crossfade(0.5) } else { image }
    }));
    ui.set_time(1.0);
    ui.update();
    (ui, shown)
}

#[test]
fn an_image_dissolves_into_the_next_and_then_asks_for_nothing() {
    let (mut ui, shade) = swapping(true);
    assert_eq!(image_opacities(&ui), [1.0], "one image to begin with");
    assert_eq!(ui.next_wake(), None, "and it is still");
    shade.set(1);
    ui.set_time(2.0);
    ui.update();
    assert_eq!(image_opacities(&ui), [1.0, 0.0], "the old one over the new one, at first");
    assert!(ui.next_wake().is_some(), "it needs frames now");
    ui.set_time(2.25);
    ui.update();
    let halfway = image_opacities(&ui);
    assert_eq!(halfway.len(), 2);
    assert!((halfway[0] - 0.5).abs() < 0.01 && (halfway[1] - 0.5).abs() < 0.01, "{halfway:?}");
    ui.set_time(2.5);
    ui.update();
    ui.update();
    assert_eq!(image_opacities(&ui), [1.0], "only the new one at the end");
    assert_eq!(ui.next_wake(), None, "and it stops asking for frames");
    assert!(!ui.needs_redraw());
}

#[test]
fn with_reduced_motion_the_next_image_simply_appears() {
    let (mut ui, shade) = swapping(true);
    ui.set_reduced_motion(true);
    ui.update();
    shade.set(1);
    ui.set_time(2.0);
    ui.update();
    assert_eq!(image_opacities(&ui), [1.0]);
    assert_eq!(ui.next_wake(), None);
}

#[test]
fn an_image_without_crossfade_changes_at_once() {
    let (mut ui, shade) = swapping(false);
    shade.set(1);
    ui.set_time(2.0);
    ui.update();
    assert_eq!(image_opacities(&ui), [1.0]);
    assert_eq!(ui.next_wake(), None);
}

#[test]
fn stacked_split_panes_fill_the_height_of_their_column() {
    let pane = |name: &'static str| SplitPane::new(VStack::new((Dot::new().label(name),)));
    let mut ui = ui(VStack::new((SplitPanes::vertical()
        .pane(pane("Top").size(200.0).range(100.0, 400.0))
        .pane(pane("Bottom").range(150.0, f32::MAX)),))
    .width(300.0)
    .height(500.0)
    .align(ravenrs_layout::Align::Stretch));
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Image, "Bottom").bounds.y(), 210.0, "below the first pane and the bar");
    assert_eq!(bars(&mut ui)[0].numeric.map(|n| n.value), Some(200.0));
}

#[test]
fn stacked_split_panes_fill_a_column_stretched_by_its_row() {
    let pane = |name: &'static str| SplitPane::new(VStack::new((Dot::new().label(name),)));
    let column = VStack::new((SplitPanes::vertical()
        .pane(pane("Top").size(200.0).range(100.0, 400.0))
        .pane(pane("Bottom").range(150.0, f32::MAX)),))
    .width(300.0)
    .align(ravenrs_layout::Align::Stretch);
    let mut ui = ui(HStack::new((column,)).height(500.0).align(ravenrs_layout::Align::Stretch));
    ui.update();
    ui.update();
    let tree = ui.access_tree();
    assert_eq!(named(&tree, Role::Image, "Bottom").bounds.y(), 210.0, "below the first pane and the bar");
}
