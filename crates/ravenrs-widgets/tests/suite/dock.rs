//! Panels in a tree that move, resize, close and are saved: each test fails if its feature goes.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use super::lists_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_core::{Border, Brush, Event, Key, Modifiers, Point, PointerButton, Primitive, Role};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

use lists_support::*;

const PANELS: [(&str, &str); 6] = [
    ("photo", "Photo"),
    ("grid", "Grid"),
    ("folders", "Folders"),
    ("search", "Search"),
    ("layers", "Layers"),
    ("log", "Log"),
];

fn panels() -> Vec<DockPanel> {
    PANELS
        .iter()
        .map(|(id, title)| {
            let panel = DockPanel::new(*id, *title);
            if *id == "photo" { panel.closable(false) } else { panel }
        })
        .collect()
}

fn start() -> DockLayout {
    DockLayout::new()
        .center(DockNode::tabs(["photo", "grid"]))
        .left(DockNode::tabs(["folders", "search"]), 280.0)
        .right(DockNode::tabs(["layers"]), 200.0)
        .bottom(DockNode::tabs(["log"]), 150.0)
}

/// A panel's content, which counts its builds and says who it is.
struct Content(String);

impl View for Content {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let line = cx.child(&Text::new("A line of text that fits and is the same as the panel gets wider"));
        let node = cx.container(
            LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Vertical) },
            &[line],
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Label).name(format!("content {}", self.0)));
        node
    }
}

fn area(layout: &State<DockLayout>) -> DockArea {
    DockArea::new(layout.binding(), panels(), |id| -> AnyView { Box::new(Content(id.to_owned())) })
        .height(600.0)
}

fn dock_ui(layout: &State<DockLayout>) -> Ui {
    ui(area(layout))
}

fn drag_to(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    settle(ui);
    ui.handle_event(&Event::PointerMove { position: to });
    settle(ui);
}

fn release(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    settle(ui);
}

/// How many times the drop mark shows: a line or a tinted box in the accent.
fn marks(ui: &Ui) -> usize {
    let accent = Theme::raven_dark().color(Semantic::Accent);
    ui.scene()
        .primitives()
        .iter()
        .filter(|p| match p {
            Primitive::Quad(q) => match q.fill {
                Brush::Solid(c) if c == accent => q.rect.width() <= 2.5,
                Brush::Solid(c) => c.a < 0.5 && q.border == Some(Border { width: 2.0, color: accent }),
                _ => false,
            },
            _ => false,
        })
        .count()
}

fn tab(ui: &mut Ui, name: &str) -> ravenrs_core::AccessNode {
    of_role(ui, Role::Tab)
        .into_iter()
        .find(|n| n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no tab {name}"))
}

fn shown(ui: &mut Ui) -> Vec<String> {
    let mut names: Vec<String> = ui
        .access_tree()
        .nodes
        .iter()
        .filter_map(|n| n.name.as_deref().and_then(|name| name.strip_prefix("content ")).map(str::to_owned))
        .collect();
    names.sort();
    names
}

#[test]
fn it_shows_the_panel_at_the_front_of_each_group_and_marks_its_tab() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    assert_eq!(shown(&mut ui), ["folders", "layers", "log", "photo"]);
    assert_eq!(tab(&mut ui, "Photo").selected, Some(true));
    assert_eq!(tab(&mut ui, "Grid").selected, Some(false));
    assert_eq!(of_role(&mut ui, Role::TabList).len(), 4);
    assert_eq!(of_role(&mut ui, Role::TabPanel).len(), 4);
}

#[test]
fn a_click_changes_the_tab_and_builds_only_what_shows() {
    let asked: Rc<RefCell<Vec<String>>> = Rc::default();
    let layout = State::new(start());
    let log = asked.clone();
    let view = DockArea::new(layout.binding(), panels(), move |id| -> AnyView {
        log.borrow_mut().push(id.to_owned());
        Box::new(Content(id.to_owned()))
    })
    .height(600.0);
    let mut ui = ui(view);
    assert!(!asked.borrow().iter().any(|id| id == "grid"), "behind another, it is not built");
    let grid = center(&tab(&mut ui, "Grid"));
    click_at(&mut ui, grid, Modifiers::default());
    assert!(layout.get().is_active("grid"));
    assert_eq!(shown(&mut ui), ["folders", "grid", "layers", "log"]);
    assert!(asked.borrow().iter().any(|id| id == "grid"));
}

#[test]
fn dragging_a_bar_resizes_within_its_limits_and_tells_when_it_settles() {
    let ends = Rc::new(Cell::new(0));
    let layout = State::new(start());
    let count = ends.clone();
    let view =
        area(&layout).dock_limits(DockSide::Left, 150.0, 300.0).on_end(move |_| count.set(count.get() + 1));
    let mut ui = ui(view);
    let bar = center(&named(&mut ui, "Resize left dock"));
    drag_to(&mut ui, bar, Point::new(bar.x - 30.0, bar.y));
    assert_eq!(layout.get().size(DockSide::Left), 250.0);
    assert_eq!(ends.get(), 0, "not while it moves");
    drag_to_more(&mut ui, Point::new(bar.x + 400.0, bar.y));
    assert_eq!(layout.get().size(DockSide::Left), 300.0, "up to its largest");
    release(&mut ui, Point::new(bar.x + 400.0, bar.y));
    assert_eq!(ends.get(), 1);
}

fn drag_to_more(ui: &mut Ui, to: Point) {
    ui.handle_event(&Event::PointerMove { position: to });
    settle(ui);
}

#[test]
fn the_right_and_bottom_bars_grow_their_dock_toward_the_center() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let bar = center(&named(&mut ui, "Resize right dock"));
    drag_to(&mut ui, bar, Point::new(bar.x - 20.0, bar.y));
    release(&mut ui, Point::new(bar.x - 20.0, bar.y));
    assert_eq!(layout.get().size(DockSide::Right), 220.0);
    let bar = center(&named(&mut ui, "Resize bottom dock"));
    drag_to(&mut ui, bar, Point::new(bar.x, bar.y - 20.0));
    release(&mut ui, Point::new(bar.x, bar.y - 20.0));
    assert_eq!(layout.get().size(DockSide::Bottom), 170.0);
}

#[test]
fn a_bar_answers_the_keyboard_with_its_limits() {
    let layout = State::new(start());
    let mut ui = ui(area(&layout).dock_limits(DockSide::Left, 150.0, 300.0));
    tab_to(&mut ui, "Resize left dock");
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(layout.get().size(DockSide::Left), 296.0);
    press(&mut ui, Key::Home, Modifiers::default());
    assert_eq!(layout.get().size(DockSide::Left), 150.0);
    press(&mut ui, Key::End, Modifiers::default());
    assert_eq!(layout.get().size(DockSide::Left), 300.0);
    let bar = named(&mut ui, "Resize left dock");
    assert_eq!(bar.role, Role::Slider);
}

#[test]
fn a_bar_inside_a_split_keeps_the_room_of_the_pair_and_each_smallest() {
    let layout = State::new(
        DockLayout::new().center(DockNode::horizontal([DockNode::tabs(["photo"]), DockNode::tabs(["grid"])])),
    );
    let mut ui = dock_ui(&layout);
    let name = "Resize between Photo and Grid";
    let bar = center(&named(&mut ui, name));
    let (photo, grid) = (tab(&mut ui, "Photo").bounds, tab(&mut ui, "Grid").bounds);
    assert!(photo.x() < bar.x && bar.x < grid.x(), "side by side");
    drag_to(&mut ui, bar, Point::new(bar.x - 100.0, bar.y));
    let Some(DockNode::Split { children, .. }) = layout.get().node(DockSide::Center).cloned() else {
        panic!("a split");
    };
    assert!((children[0].share - 0.37).abs() < 0.01, "{}", children[0].share);
    assert!((children[0].share + children[1].share - 1.0).abs() < 0.001);
    drag_to_more(&mut ui, Point::new(0.0, bar.y));
    release(&mut ui, Point::new(0.0, bar.y));
    let left = of_role(&mut ui, Role::TabPanel)[0].bounds;
    assert_eq!(left.width(), 100.0, "down to the smallest of a group");
}

#[test]
fn dragging_a_tab_marks_the_target_and_moves_it_on_release() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let (from, to) = (center(&tab(&mut ui, "Search")), tab(&mut ui, "Grid").bounds);
    assert_eq!(marks(&ui), 0);
    let target = Point::new(to.max_x() - 1.0, to.y() + 4.0);
    drag_to(&mut ui, from, target);
    assert_eq!(marks(&ui), 1, "the line that shows where it lands");
    assert!(layout.get().side_of("search") == Some(DockSide::Left), "nothing moves before the release");
    release(&mut ui, target);
    assert_eq!(marks(&ui), 0);
    let now = layout.get();
    assert_eq!(
        now.node(DockSide::Center),
        Some(&DockNode::Tabs { panels: vec!["photo".into(), "grid".into(), "search".into()], active: 2 })
    );
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Search"), "focus goes with it");
    assert!(has(&mut ui, "Moved to center"), "and it is said");
}

#[test]
fn dropping_on_the_side_of_a_group_splits_it() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let body = of_role(&mut ui, Role::TabPanel)
        .into_iter()
        .find(|n| n.name.as_deref() == Some("Photo"))
        .expect("the panel")
        .bounds;
    let from = center(&tab(&mut ui, "Search"));
    let target = Point::new(body.max_x() - 4.0, body.center().y);
    drag_to(&mut ui, from, target);
    assert_eq!(marks(&ui), 1);
    release(&mut ui, target);
    let now = layout.get();
    let Some(DockNode::Split { children, .. }) = now.node(DockSide::Center) else { panic!("a split") };
    assert_eq!(children[1].node, DockNode::tabs(["search"]));
    assert_eq!(shown(&mut ui).iter().filter(|id| *id == "search").count(), 1);
}

#[test]
fn a_tab_reorders_within_its_group_by_drag() {
    let layout = State::new(DockLayout::new().center(DockNode::tabs(["photo", "grid", "log"])));
    let mut ui = dock_ui(&layout);
    let (from, end) = (center(&tab(&mut ui, "Photo")), tab(&mut ui, "Log").bounds);
    let target = Point::new(end.max_x() - 1.0, end.center().y);
    drag_to(&mut ui, from, target);
    release(&mut ui, target);
    let now = layout.get();
    assert_eq!(now.panels(), ["grid", "log", "photo"]);
}

#[test]
fn a_drop_that_changes_nothing_marks_nothing_and_a_click_stays_a_click() {
    let layout =
        State::new(DockLayout::new().center(DockNode::tabs(["photo"])).left(DockNode::tabs(["log"]), 200.0));
    let mut ui = dock_ui(&layout);
    let before = layout.get();
    let from = center(&tab(&mut ui, "Photo"));
    let on_itself = Point::new(from.x + 8.0, from.y + 60.0);
    drag_to(&mut ui, from, on_itself);
    assert_eq!(marks(&ui), 0, "a panel alone in its group is already there");
    release(&mut ui, on_itself);
    assert_eq!(layout.get(), before);
    let from = center(&tab(&mut ui, "Log"));
    drag_to(&mut ui, from, Point::new(from.x + 2.0, from.y));
    assert_eq!(marks(&ui), 0);
    release(&mut ui, Point::new(from.x + 2.0, from.y));
    assert_eq!(layout.get(), before);
}

#[test]
fn escape_calls_a_drag_off() {
    let layout = State::new(start());
    let before = layout.get();
    let mut ui = dock_ui(&layout);
    let (from, to) = (center(&tab(&mut ui, "Search")), center(&tab(&mut ui, "Grid")));
    drag_to(&mut ui, from, to);
    assert_eq!(marks(&ui), 1);
    press(&mut ui, Key::Escape, Modifiers::default());
    assert_eq!(marks(&ui), 0);
    release(&mut ui, to);
    assert_eq!(layout.get(), before);
}

#[test]
fn a_panel_keeps_what_it_holds_as_it_moves_to_another_group() {
    struct Keeps;
    impl View for Keeps {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let count: State<u32> = cx.use_state(|| 0);
            let node = cx.container(LayoutStyle { grow: 1.0, ..LayoutStyle::leaf() }, &[], Paint::None);
            let more = count.clone();
            cx.on_click(node, Rc::new(move || more.update(|n| *n += 1)));
            cx.semantics(node, Semantics::new(Role::Button).name(format!("count {}", count.get())));
            node
        }
    }
    let layout = State::new(start());
    let view = DockArea::new(layout.binding(), panels(), |id| -> AnyView {
        if id == "layers" { Box::new(Keeps) } else { Box::new(Content(id.to_owned())) }
    })
    .height(600.0);
    let mut ui = ui(view);
    let button = center(&named(&mut ui, "count 0"));
    click_at(&mut ui, button, Modifiers::default());
    assert!(has(&mut ui, "count 1"));
    let mut next = layout.get();
    assert!(next.move_panel("layers", DockDrop::Side(DockSide::Left)));
    layout.set(next);
    settle(&mut ui);
    assert!(has(&mut ui, "count 1"), "the same panel, somewhere else");
}

#[test]
fn the_keyboard_walks_between_tabs_reorders_and_closes() {
    let layout = State::new(DockLayout::new().center(DockNode::tabs(["grid", "log", "search"])));
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Grid");
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Log"));
    assert!(layout.get().is_active("log"), "the arrows open what they reach");
    press(&mut ui, Key::ArrowRight, alt());
    assert_eq!(layout.get().panels(), ["grid", "search", "log"]);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Log"));
    assert!(has(&mut ui, "Moved to position 3 of 3"));
    press(&mut ui, Key::Home, alt());
    assert_eq!(layout.get().panels(), ["log", "grid", "search"]);
    press(&mut ui, Key::Delete, Modifiers::default());
    assert_eq!(layout.get().panels(), ["grid", "search"]);
    assert!(layout.get().closed_panels().contains(&"log"));
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Grid"), "focus stays among the tabs");
}

#[test]
fn alt_and_shift_with_an_arrow_send_a_tab_to_a_dock() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Grid");
    let both = Modifiers { alt: true, shift: true, ..Modifiers::default() };
    press(&mut ui, Key::ArrowLeft, both);
    assert_eq!(layout.get().side_of("grid"), Some(DockSide::Left));
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Grid"));
    press(&mut ui, Key::ArrowDown, both);
    assert_eq!(layout.get().side_of("grid"), Some(DockSide::Bottom));
}

#[test]
fn the_button_of_a_strip_has_the_menu_with_move_to_from_the_keyboard() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Options of Folders");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Move to…"));
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Right dock"), "the first place it can go");
    press(&mut ui, Key::Enter, Modifiers::default());
    settle(&mut ui);
    assert_eq!(layout.get().side_of("folders"), Some(DockSide::Right));
}

#[test]
fn the_menu_of_a_tab_moves_closes_and_reopens() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let at = center(&tab(&mut ui, "Log"));
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Secondary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Secondary });
    settle(&mut ui);
    let close = center(&named(&mut ui, "Close panel"));
    click_at(&mut ui, close, Modifiers::default());
    assert_eq!(layout.get().closed_panels(), ["log"]);
    assert!(!has(&mut ui, "Resize bottom dock"), "a dock with nothing goes");

    tab_to(&mut ui, "Options of Photo");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(has(&mut ui, "Reopen"));
    assert!(!has(&mut ui, "Close panel"), "this one cannot close");
    let reopen = center(&named(&mut ui, "Reopen"));
    click_at(&mut ui, reopen, Modifiers::default());
    let log = center(&named(&mut ui, "Log"));
    click_at(&mut ui, log, Modifiers::default());
    assert_eq!(layout.get().side_of("log"), Some(DockSide::Bottom), "it comes back where it was");
    assert!(layout.get().closed_panels().is_empty());
}

#[test]
fn a_panel_that_cannot_close_has_no_way_to() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    assert!(!has(&mut ui, "Close Photo"));
    assert!(has(&mut ui, "Close Grid"));
    tab_to(&mut ui, "Photo");
    press(&mut ui, Key::Delete, Modifiers::default());
    assert!(layout.get().contains("photo"));
}

#[test]
fn docks_collapse_and_come_back_with_their_size() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let before = of_role(&mut ui, Role::TabPanel).len();
    let hide = named(&mut ui, "Hide left dock");
    assert_eq!(hide.expanded, Some(true));
    assert_eq!(hide.role, Role::Button);
    tab_to(&mut ui, "Hide left dock");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(!layout.get().is_open(DockSide::Left));
    assert_eq!(of_role(&mut ui, Role::TabPanel).len(), before - 1);
    assert!(!has(&mut ui, "Resize left dock"));
    let show = center(&named(&mut ui, "Show left dock"));
    click_at(&mut ui, show, Modifiers::default());
    assert!(layout.get().is_open(DockSide::Left));
    assert_eq!(layout.get().size(DockSide::Left), 280.0);
}

#[test]
fn a_drop_on_the_border_opens_a_collapsed_dock_and_the_strips_can_go() {
    let layout = State::new(start());
    let mut next = layout.get();
    next.set_open(DockSide::Left, false);
    layout.set(next);
    let mut ui = dock_ui(&layout);
    let from = center(&tab(&mut ui, "Grid"));
    let target = Point::new(4.0, 300.0);
    drag_to(&mut ui, from, target);
    assert_eq!(marks(&ui), 1);
    release(&mut ui, target);
    assert_eq!(layout.get().side_of("grid"), Some(DockSide::Left));
    assert!(layout.get().is_open(DockSide::Left));

    let plain = ui_of(area(&layout).edge_toggles(false));
    assert!(!plain);
}

fn ui_of(view: DockArea) -> bool {
    let mut made = ui(view);
    has(&mut made, "Hide left dock")
}

#[test]
fn the_arrangement_survives_saving_and_restoring() {
    let saved = Rc::new(RefCell::new(String::new()));
    let layout = State::new(start());
    let keep = saved.clone();
    let view = area(&layout).on_end(move |now| *keep.borrow_mut() = now.to_text());
    let mut ui = ui(view);
    let (from, to) = (center(&tab(&mut ui, "Layers")), center(&tab(&mut ui, "Grid")));
    drag_to(&mut ui, from, to);
    release(&mut ui, to);
    let text = saved.borrow().clone();
    assert!(text.contains("\"version\": 1"), "{text}");
    let restored = DockLayout::parse(&text).expect("it reads");
    assert_eq!(restored, layout.get());

    let again = State::new(restored);
    let mut second = dock_ui(&again);
    assert!(tab(&mut second, "Layers").selected == Some(true));
    assert_eq!(shown(&mut second), shown(&mut ui));
    assert_eq!(again.get().to_text(), text, "drawing it changes nothing");
}

#[test]
fn an_unknown_id_in_what_was_saved_is_left_out_and_the_rest_goes_on() {
    let saved = DockLayout::new()
        .center(DockNode::horizontal([DockNode::tabs(["photo", "gone"]), DockNode::tabs(["gone-too"])]))
        .left(DockNode::tabs(["removed-in-a-later-version"]), 220.0);
    let layout = State::new(DockLayout::parse(&saved.to_text()).expect("it reads"));
    let mut ui = dock_ui(&layout);
    assert_eq!(of_role(&mut ui, Role::Tab).len(), 1);
    assert_eq!(shown(&mut ui), ["photo"]);
    let now = layout.get();
    assert_eq!(now.panels(), ["photo"]);
    assert_eq!(
        now.node(DockSide::Center),
        Some(&DockNode::tabs(["photo"])),
        "the split with one part gave way"
    );
    assert!(!now.to_text().contains("gone"), "the next save leaves it out");
    assert_eq!(now.size(DockSide::Left), 220.0, "what it knew of the place stays");
    assert_eq!(now.closed_panels().len(), 5, "the panels it did not mention can be opened");
}

#[test]
fn a_text_that_cannot_be_read_gives_an_error_not_a_panic() {
    for text in ["", "ravenrs", "{", "{}", "null", r#"{"version":"x"}"#, r#"{"version":2}"#] {
        let error = DockLayout::parse(text).expect_err(text);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn closing_the_last_panel_leaves_a_way_back() {
    let layout = State::new(DockLayout::new().center(DockNode::tabs(["grid"])));
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Grid");
    press(&mut ui, Key::Delete, Modifiers::default());
    assert!(layout.get().panels().is_empty());
    assert!(of_role(&mut ui, Role::Tab).is_empty());
    assert!(has(&mut ui, "No panels here"));
    assert_eq!(DockLayout::parse(&layout.get().to_text()), Ok(layout.get()), "it is still an arrangement");
    let back = center(&named(&mut ui, "Grid"));
    click_at(&mut ui, back, Modifiers::default());
    assert_eq!(layout.get().panels(), ["grid"]);
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Grid"));
}

#[test]
fn a_panel_can_go_to_an_empty_center_by_a_drag() {
    let layout = State::new(DockLayout::new().left(DockNode::tabs(["grid"]), 200.0));
    let mut ui = dock_ui(&layout);
    let from = center(&tab(&mut ui, "Grid"));
    let target = Point::new(600.0, 300.0);
    drag_to(&mut ui, from, target);
    assert_eq!(marks(&ui), 1);
    release(&mut ui, target);
    assert_eq!(layout.get().side_of("grid"), Some(DockSide::Center));
    assert!(layout.get().node(DockSide::Left).is_none());
}

#[test]
fn panels_that_do_not_move_have_no_drag_no_keys_and_no_item() {
    let layout = State::new(start());
    let before = layout.get();
    let mut ui = ui(area(&layout).movable(false));
    assert!(has(&mut ui, "Options of Folders"), "closing is still there");
    let (from, to) = (center(&tab(&mut ui, "Search")), center(&tab(&mut ui, "Grid")));
    drag_to(&mut ui, from, to);
    assert_eq!(marks(&ui), 0);
    release(&mut ui, to);
    tab_to(&mut ui, "Grid");
    press(&mut ui, Key::ArrowLeft, Modifiers { alt: true, shift: true, ..Modifiers::default() });
    assert_eq!(layout.get().side_of("grid"), before.side_of("grid"));
    assert_eq!(layout.get().panels(), before.panels());
    tab_to(&mut ui, "Options of Folders");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(!has(&mut ui, "Move to…"));
}

#[test]
fn a_group_can_go_without_its_tabs() {
    let layout = State::new(start());
    let mut ui = ui(area(&layout).tab_bar(DockTabBar::WhenSeveral));
    assert_eq!(of_role(&mut ui, Role::Tab).len(), 4, "Layers and Log are alone, and show no tab");
    assert_eq!(shown(&mut ui), ["folders", "layers", "log", "photo"]);
    let mut none = ravenrs_widgets::Ui::new(area(&layout).tab_bar(DockTabBar::Never), Theme::raven_dark());
    none.set_viewport(ravenrs_core::Size::new(800.0, 600.0), 1.0);
    settle(&mut none);
    assert!(of_role(&mut none, Role::Tab).is_empty());
    assert_eq!(shown(&mut none), ["folders", "layers", "log", "photo"]);
}

#[test]
fn its_words_come_from_labels() {
    let layout = State::new(start());
    let view = Localized::new(Labels::pt_br(), area(&layout));
    let mut ui = ui(view);
    assert!(has(&mut ui, "Ocultar doca esquerda"));
    assert!(has(&mut ui, "Redimensionar doca direita"));
    assert!(has(&mut ui, "Fechar Grid"));
    assert!(has(&mut ui, "Opções de Folders"));
    tab_to(&mut ui, "Opções de Folders");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(has(&mut ui, "Mover para…"));
}

#[test]
fn moving_a_bar_shapes_no_text_again() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let bar = center(&named(&mut ui, "Resize left dock"));
    drag_to(&mut ui, bar, Point::new(bar.x + 5.0, bar.y));
    let _ = ui.take_frame_work();
    for step in 1..=10 {
        let position = Point::new(bar.x + 5.0 + step as f32, bar.y);
        ui.handle_event(&Event::PointerMove { position });
        ui.update();
    }
    let work = ui.take_frame_work();
    assert_eq!(work.shapes, 0, "the titles are the same text, at the same size");
    release(&mut ui, bar);
}

#[test]
fn changing_tabs_shapes_only_the_new_tab_s_state() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let elements_before = {
        let _ = ui.take_frame_work();
        ui.element_count()
    };
    let grid = center(&tab(&mut ui, "Grid"));
    click_at(&mut ui, grid, Modifiers::default());
    assert_eq!(ui.element_count(), elements_before, "another panel, of the same size");
    assert_eq!(ui.take_frame_work().shapes, 0, "no title changed");
}
