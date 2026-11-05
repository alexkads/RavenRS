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
