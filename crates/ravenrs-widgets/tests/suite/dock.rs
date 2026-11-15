//! Panels in a tree that move, resize, close and are saved: each test fails if its feature goes.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use super::lists_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_core::{Border, Brush, Event, Key, Modifiers, Point, PointerButton, Primitive, Rect, Role};
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

fn dock_ui_of(view: DockArea) -> Ui {
    ui(view)
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
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Top dock"), "the first place it can go");
    press(&mut ui, Key::Enter, Modifiers::default());
    settle(&mut ui);
    assert_eq!(layout.get().side_of("folders"), Some(DockSide::Top));
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
    assert!(text.contains("\"version\": 4"), "{text}");
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
    for text in ["", "ravenrs", "{", "{}", "null", r#"{"version":"x"}"#, r#"{"version":5}"#] {
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

/// The words of the bubbles that say why a change was refused.
fn refusals(ui: &mut Ui) -> Vec<String> {
    of_role(ui, Role::Status).into_iter().filter_map(|n| n.name).filter(|name| !name.is_empty()).collect()
}

/// The changes an area's app was asked about, in order.
type Asked = Rc<RefCell<Vec<DockChange>>>;

/// An area whose app refuses every change of one kind, writing down each one it is asked about,
/// with how many times it settled.
fn guarded(
    layout: &State<DockLayout>,
    refuse: fn(&DockChange) -> bool,
) -> (DockArea, Asked, Rc<Cell<usize>>) {
    let (asked, ends) = (Rc::new(RefCell::new(Vec::new())), Rc::new(Cell::new(0)));
    let (log, count) = (asked.clone(), ends.clone());
    let view = area(layout)
        .on_change(move |_, _, change| {
            log.borrow_mut().push(change.clone());
            if refuse(change) { Err("Not now".into()) } else { Ok(()) }
        })
        .on_end(move |_| count.set(count.get() + 1));
    (view, asked, ends)
}

#[test]
fn a_refused_close_keeps_the_panel_says_why_and_saves_nothing() {
    let layout = State::new(start());
    let (view, asked, ends) = guarded(&layout, |c| matches!(c, DockChange::Close { .. }));
    let mut ui = ui(view);
    tab_to(&mut ui, "Grid");
    press(&mut ui, Key::Delete, Modifiers::default());
    assert!(layout.get().contains("grid"), "the panel stays");
    assert!(layout.get().closed_panels().is_empty());
    assert_eq!(*asked.borrow(), [DockChange::Close { panel: "grid".into() }]);
    assert_eq!(refusals(&mut ui), ["Not now"], "the reason shows, as a status");
    assert_eq!(ends.get(), 0, "nothing to save");
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Grid"), "focus stays on the tab");
}

#[test]
fn the_reason_goes_after_a_few_seconds_or_at_the_next_gesture() {
    let layout = State::new(start());
    let (view, _, _) = guarded(&layout, |c| matches!(c, DockChange::Close { .. }));
    let mut ui = ui(view);
    ui.set_time(10.0);
    tab_to(&mut ui, "Grid");
    press(&mut ui, Key::Delete, Modifiers::default());
    assert_eq!(refusals(&mut ui), ["Not now"]);
    assert_eq!(ui.next_wake(), Some(14.0), "wakes when it is to go");
    ui.set_time(14.0);
    ui.update();
    ui.update();
    assert!(refusals(&mut ui).is_empty(), "gone on the clock");
    press(&mut ui, Key::Delete, Modifiers::default());
    assert_eq!(refusals(&mut ui), ["Not now"]);
    let photo = center(&tab(&mut ui, "Photo"));
    click_at(&mut ui, photo, Modifiers::default());
    assert!(layout.get().is_active("photo"), "an allowed change goes ahead");
    assert!(refusals(&mut ui).is_empty(), "and the next gesture takes the reason away");
}

#[test]
fn a_refused_drop_leaves_the_tab_where_it_was() {
    let layout = State::new(start());
    let (view, asked, ends) = guarded(&layout, |c| matches!(c, DockChange::Move { .. }));
    let mut ui = ui(view);
    let (from, to) = (center(&tab(&mut ui, "Search")), tab(&mut ui, "Grid").bounds);
    let target = Point::new(to.max_x() - 1.0, to.y() + 4.0);
    drag_to(&mut ui, from, target);
    assert!(asked.borrow().is_empty(), "nothing is asked while it moves");
    release(&mut ui, target);
    assert_eq!(layout.get().side_of("search"), Some(DockSide::Left));
    assert_eq!(asked.borrow().len(), 1, "asked once, when let go");
    let DockChange::Move { panel, to } = &asked.borrow()[0] else { panic!("a move") };
    assert_eq!(panel, "search");
    assert!(matches!(to, DockDrop::Tab { beside, .. } if beside == "photo"));
    assert_eq!(refusals(&mut ui), ["Not now"]);
    assert_eq!(ends.get(), 0);
}

#[test]
fn a_refused_bar_goes_back_and_is_asked_once_when_let_go() {
    let layout = State::new(start());
    let (view, asked, ends) = guarded(&layout, |c| matches!(c, DockChange::Resize { .. }));
    let mut ui = ui(view);
    let bar = center(&named(&mut ui, "Resize left dock"));
    drag_to(&mut ui, bar, Point::new(bar.x - 30.0, bar.y));
    drag_to_more(&mut ui, Point::new(bar.x - 50.0, bar.y));
    assert_eq!(layout.get().size(DockSide::Left), 230.0, "it follows the pointer while it moves");
    release(&mut ui, Point::new(bar.x - 50.0, bar.y));
    assert_eq!(*asked.borrow(), [DockChange::Resize { side: DockSide::Left }]);
    assert_eq!(layout.get().size(DockSide::Left), 280.0, "and goes back when refused");
    assert_eq!(refusals(&mut ui), ["Not now"]);
    assert_eq!(ends.get(), 0);
    tab_to(&mut ui, "Resize left dock");
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    assert_eq!(asked.borrow().len(), 2, "each key that moves it is asked by itself");
    assert_eq!(layout.get().size(DockSide::Left), 280.0);
}

#[test]
fn every_change_a_person_makes_is_named_and_what_the_app_writes_is_not() {
    let layout = State::new(start());
    let seen: Rc<RefCell<Vec<(DockChange, bool)>>> = Rc::default();
    let log = seen.clone();
    let view = area(&layout).on_change(move |before, after, change| {
        // The arrangement it is given is the one before and the one the change would leave.
        let moved = match change {
            DockChange::Move { panel, .. } => before.side_of(panel) != after.side_of(panel),
            _ => before != after,
        };
        log.borrow_mut().push((change.clone(), moved));
        Ok(())
    });
    let mut ui = ui(view);
    let grid = center(&tab(&mut ui, "Grid"));
    click_at(&mut ui, grid, Modifiers::default());
    let both = Modifiers { alt: true, shift: true, ..Modifiers::default() };
    press(&mut ui, Key::ArrowLeft, both);
    tab_to(&mut ui, "Hide left dock");
    press(&mut ui, Key::Enter, Modifiers::default());
    tab_to(&mut ui, "Log");
    press(&mut ui, Key::Delete, Modifiers::default());
    layout.update(|now| {
        now.reopen("log");
    });
    ui.update();
    assert_eq!(
        *seen.borrow(),
        [
            (DockChange::Activate { panel: "grid".into() }, true),
            (DockChange::Move { panel: "grid".into(), to: DockDrop::Side(DockSide::Left) }, true),
            (DockChange::Collapse { side: DockSide::Left, open: false }, true),
            (DockChange::Close { panel: "log".into() }, true),
        ]
    );
    assert!(layout.get().contains("log"), "the app brought it back without being asked");
}

#[test]
fn alt_and_shift_with_up_send_a_tab_to_the_top_dock_and_with_enter_to_the_center() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Search");
    let both = Modifiers { alt: true, shift: true, ..Modifiers::default() };
    press(&mut ui, Key::ArrowUp, both);
    assert_eq!(layout.get().side_of("search"), Some(DockSide::Top));
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Search"));
    assert!(has(&mut ui, "Moved to top dock"), "and it is said");
    assert!(has(&mut ui, "Resize top dock"), "the top dock has its bar");
    press(&mut ui, Key::Enter, both);
    assert_eq!(layout.get().side_of("search"), Some(DockSide::Center));
}

#[test]
fn a_drop_at_the_very_top_opens_the_top_dock_and_its_strip_collapses_it() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let (from, grid) = (center(&tab(&mut ui, "Search")), tab(&mut ui, "Grid").bounds);
    let target = Point::new(grid.max_x() + 40.0, grid.y() + 1.0);
    drag_to(&mut ui, from, target);
    assert_eq!(marks(&ui), 1, "the room the top dock would take");
    release(&mut ui, target);
    assert_eq!(layout.get().side_of("search"), Some(DockSide::Top));
    assert_eq!(layout.get().size(DockSide::Top), 120.0, "at its usual height");
    tab_to(&mut ui, "Hide top dock");
    press(&mut ui, Key::Enter, Modifiers::default());
    assert!(!layout.get().is_open(DockSide::Top));
    assert!(!has(&mut ui, "Resize top dock"));
    assert!(has(&mut ui, "Show top dock"));
}

#[test]
fn the_top_dock_has_its_limits() {
    let layout = State::new(
        DockLayout::new().center(DockNode::tabs(["photo"])).top(DockNode::tabs(["search"]), 120.0),
    );
    let mut ui = ui(area(&layout).dock_limits(DockSide::Top, 80.0, 200.0));
    tab_to(&mut ui, "Resize top dock");
    press(&mut ui, Key::Home, Modifiers::default());
    assert_eq!(layout.get().size(DockSide::Top), 80.0);
    press(&mut ui, Key::End, Modifiers::default());
    assert_eq!(layout.get().size(DockSide::Top), 200.0);
}

#[test]
fn the_pin_hides_a_panel_at_the_edge_of_its_dock_with_a_tab_on_a_rail() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let photo_before = tab(&mut ui, "Photo").bounds;
    let pin = center(&named(&mut ui, "Auto-hide Folders"));
    click_at(&mut ui, pin, Modifiers::default());
    settle(&mut ui);
    let now = layout.get();
    assert!(now.is_auto_hidden("folders"));
    assert_eq!(now.side_of("search"), Some(DockSide::Left), "the rest of the group stays");
    let rail = named(&mut ui, "Folders, hidden in the left dock");
    assert_eq!(rail.role, Role::Button);
    assert_eq!(rail.expanded, Some(false));
    assert!(
        rail.bounds.x() < 1.0 && rail.bounds.width() <= 28.5,
        "on a rail at the very edge: {:?}",
        rail.bounds
    );
    assert!(rail.bounds.height() > rail.bounds.width(), "the name runs down the rail");
    assert_eq!(
        focused_name(&mut ui).as_deref(),
        Some("Folders, hidden in the left dock"),
        "focus goes with it"
    );
    assert!(has(&mut ui, "Folders, hidden in the left dock"), "and it is said");
    assert!(tab(&mut ui, "Photo").bounds.x() >= photo_before.x(), "the rail takes its room from the side");
    assert!(
        DockLayout::parse(&now.to_text()).expect("it reads").is_auto_hidden("folders"),
        "and it is saved"
    );
}

#[test]
fn a_click_on_the_rail_opens_the_panel_over_the_rest_and_its_pin_puts_it_back() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Folders");
    let command = if apple_keys() {
        Modifiers { meta: true, alt: true, ..Modifiers::default() }
    } else {
        Modifiers { ctrl: true, alt: true, ..Modifiers::default() }
    };
    press(&mut ui, Key::Character("p".into()), command);
    assert!(layout.get().is_auto_hidden("folders"), "Ctrl+Alt+P hides it");
    let center_before = tab(&mut ui, "Photo").bounds;
    let on_rail = center(&named(&mut ui, "Folders, hidden in the left dock"));
    click_at(&mut ui, on_rail, Modifiers::default());
    settle(&mut ui);
    assert_eq!(named(&mut ui, "Folders, hidden in the left dock").expanded, Some(true));
    assert!(shown(&mut ui).contains(&"folders".to_owned()), "what it shows is built");
    let open = named(&mut ui, "Folders");
    assert_eq!(open.role, Role::Group);
    // As wide as the dock was, with the bar that resizes it, touching the rail.
    assert_eq!(open.bounds, Rect::new(28.0, 0.0, 280.0 + 10.0, 600.0));
    assert_eq!(tab(&mut ui, "Photo").bounds, center_before, "over the rest, without moving it");
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Pin Folders"), "focus goes into it");
    press(&mut ui, Key::Escape, Modifiers::default());
    settle(&mut ui);
    assert!(!has(&mut ui, "Pin Folders"), "Escape lets it go back");
    click_at(&mut ui, on_rail, Modifiers::default());
    settle(&mut ui);
    let pin = center(&named(&mut ui, "Pin Folders"));
    click_at(&mut ui, pin, Modifiers::default());
    settle(&mut ui);
    let now = layout.get();
    assert!(!now.is_auto_hidden("folders"));
    assert_eq!(now.side_of("folders"), Some(DockSide::Left), "back in its dock");
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Folders"));
    assert!(has(&mut ui, "Folders pinned"));
}

#[test]
fn resting_the_pointer_on_the_rail_opens_the_panel_and_leaving_lets_it_go() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    ui.set_time(10.0);
    let pin = center(&named(&mut ui, "Auto-hide Layers"));
    click_at(&mut ui, pin, Modifiers::default());
    settle(&mut ui);
    let on_rail = center(&named(&mut ui, "Layers, hidden in the right dock"));
    ui.handle_event(&Event::PointerMove { position: on_rail });
    ui.update();
    assert!(!has(&mut ui, "Pin Layers"), "not at once");
    assert_eq!(ui.next_wake(), Some(10.4));
    ui.set_time(10.4);
    ui.update();
    ui.update();
    assert!(has(&mut ui, "Pin Layers"), "after a moment of rest");
    ui.handle_event(&Event::PointerMove { position: Point::new(300.0, 300.0) });
    ui.update();
    ui.update();
    assert!(has(&mut ui, "Pin Layers"), "not at once either");
    ui.set_time(11.1);
    ui.update();
    ui.update();
    assert!(!has(&mut ui, "Pin Layers"), "a moment after the pointer left");
}

#[test]
fn the_app_can_refuse_hiding_and_the_center_has_no_pin() {
    let layout = State::new(start());
    let (view, asked, _) = guarded(&layout, |c| matches!(c, DockChange::AutoHide { .. }));
    let mut ui = ui(view);
    assert!(!has(&mut ui, "Auto-hide Photo"), "the center has no edge to hide at");
    let pin = center(&named(&mut ui, "Auto-hide Log"));
    click_at(&mut ui, pin, Modifiers::default());
    settle(&mut ui);
    assert_eq!(*asked.borrow(), [DockChange::AutoHide { panel: "log".into() }]);
    assert!(!layout.get().is_auto_hidden("log"));
    assert_eq!(refusals(&mut ui), ["Not now"]);
    let off = State::new(start());
    let mut without = dock_ui_of(area(&off).auto_hide(false));
    assert!(!has(&mut without, "Auto-hide Folders"), "an app can leave it out");
}

#[test]
fn the_arrows_walk_the_tabs_of_a_rail() {
    let mut start = start();
    assert!(start.auto_hide("folders"));
    assert!(start.auto_hide("search"));
    let layout = State::new(start);
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Folders, hidden in the left dock");
    press(&mut ui, Key::ArrowDown, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Search, hidden in the left dock"));
    press(&mut ui, Key::ArrowUp, Modifiers::default());
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Folders, hidden in the left dock"));
}

fn floating_start() -> DockLayout {
    let mut layout = start();
    assert!(layout.float("layers", Rect::new(300.0, 100.0, 240.0, 200.0)));
    layout
}

fn command_alt() -> Modifiers {
    if apple_keys() {
        Modifiers { meta: true, alt: true, ..Modifiers::default() }
    } else {
        Modifiers { ctrl: true, alt: true, ..Modifiers::default() }
    }
}

#[test]
fn float_in_the_menu_lifts_a_panel_into_a_box_of_its_own_with_focus_on_it() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Options of Layers");
    press(&mut ui, Key::Enter, Modifiers::default());
    while focused_name(&mut ui).as_deref() != Some("Float") {
        press(&mut ui, Key::ArrowDown, Modifiers::default());
    }
    press(&mut ui, Key::Enter, Modifiers::default());
    let now = layout.get();
    assert!(now.is_floating("layers"));
    assert_eq!(now.side_of("layers"), None, "out of the tree");
    let dialog = named(&mut ui, "Layers, floating");
    assert_eq!(dialog.role, Role::Dialog);
    assert_eq!(dialog.bounds, Rect::new(300.0, 150.0, 200.0, 300.0), "in the middle, as wide as it was");
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Move Layers"), "focus on its header");
    assert!(shown(&mut ui).contains(&"layers".to_owned()), "what it shows is built");
}

#[test]
fn the_header_moves_it_asking_once_and_draws_it_to_an_edge_near_it() {
    let layout = State::new(floating_start());
    let (view, asked, ends) = guarded(&layout, |_| false);
    let mut ui = ui(view);
    let header = center(&named(&mut ui, "Move Layers"));
    drag_to(&mut ui, header, Point::new(header.x + 50.0, header.y + 30.0));
    assert_eq!(named(&mut ui, "Layers, floating").bounds.x(), 350.0, "it follows the pointer");
    assert!(asked.borrow().is_empty(), "nothing is asked while it moves");
    release(&mut ui, Point::new(header.x + 50.0, header.y + 30.0));
    assert_eq!(*asked.borrow(), [DockChange::Place { panel: "layers".into() }]);
    assert_eq!(layout.get().floating_rect("layers"), Some(Rect::new(350.0, 130.0, 240.0, 200.0)));
    assert_eq!(ends.get(), 1);
    let header = center(&named(&mut ui, "Move Layers"));
    drag_to(&mut ui, header, Point::new(header.x - 345.0, header.y));
    release(&mut ui, Point::new(header.x - 345.0, header.y));
    assert_eq!(
        layout.get().floating_rect("layers").map(|r| r.x()),
        Some(0.0),
        "drawn to the border 5 px away"
    );
}

#[test]
fn the_header_lets_go_on_the_tabs_of_a_group_to_dock_there_and_passes_over_the_rest() {
    let layout = State::new(floating_start());
    let mut ui = dock_ui(&layout);
    let header = center(&named(&mut ui, "Move Layers"));
    let grid = center(&tab(&mut ui, "Grid"));
    let body = Point::new(grid.x, grid.y + 200.0);
    drag_to(&mut ui, header, body);
    assert_eq!(marks(&ui), 0, "over the middle of a group nothing marks");
    drag_to_more(&mut ui, grid);
    assert_eq!(marks(&ui), 1, "on its tabs, the gap marks");
    release(&mut ui, grid);
    let now = layout.get();
    assert!(!now.is_floating("layers"));
    assert_eq!(now.side_of("layers"), Some(DockSide::Center));
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Layers"));
}

#[test]
fn a_corner_resizes_it_no_smaller_than_its_least() {
    let layout = State::new(floating_start());
    let mut ui = dock_ui(&layout);
    let corner = Point::new(300.0 + 240.0 + 3.0, 100.0 + 200.0 + 3.0);
    drag_to(&mut ui, corner, Point::new(corner.x + 60.0, corner.y + 40.0));
    release(&mut ui, Point::new(corner.x + 60.0, corner.y + 40.0));
    assert_eq!(layout.get().floating_rect("layers"), Some(Rect::new(300.0, 100.0, 300.0, 240.0)));
    let corner = Point::new(303.0, 103.0);
    drag_to(&mut ui, corner, Point::new(corner.x + 400.0, corner.y + 400.0));
    release(&mut ui, Point::new(corner.x + 400.0, corner.y + 400.0));
    let rect = layout.get().floating_rect("layers").expect("still floating");
    assert_eq!((rect.width(), rect.height()), (200.0, 120.0), "{rect:?}");
    assert_eq!((rect.max_x(), rect.max_y()), (600.0, 340.0), "the other corner stays");
}

#[test]
fn the_keys_move_resize_and_dock_it_back_and_a_double_click_does_too() {
    let layout = State::new(floating_start());
    let mut ui = dock_ui(&layout);
    tab_to(&mut ui, "Move Layers");
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    press(&mut ui, Key::ArrowDown, Modifiers { shift: true, ..Modifiers::default() });
    press(&mut ui, Key::ArrowRight, Modifiers { alt: true, ..Modifiers::default() });
    assert_eq!(layout.get().floating_rect("layers"), Some(Rect::new(308.0, 132.0, 248.0, 200.0)));
    press(&mut ui, Key::Character("f".into()), command_alt());
    assert_eq!(layout.get().side_of("layers"), Some(DockSide::Right), "back where it came from");
    let mut again = floating_start();
    again.set_floating_rect("layers", Rect::new(300.0, 100.0, 240.0, 200.0));
    layout.set(again);
    settle(&mut ui);
    let header = center(&named(&mut ui, "Move Layers"));
    ui.handle_event(&Event::DoubleClick { position: header });
    settle(&mut ui);
    assert_eq!(layout.get().side_of("layers"), Some(DockSide::Right));
}

#[test]
fn a_tab_dragged_with_ctrl_floats_where_it_is_let_go() {
    let layout = State::new(start());
    let mut ui = dock_ui(&layout);
    let from = center(&tab(&mut ui, "Search"));
    let to = Point::new(400.0, 300.0);
    drag_to(&mut ui, from, Point::new(from.x + 40.0, from.y + 40.0));
    let ctrl = Modifiers { ctrl: true, ..Modifiers::default() };
    ui.handle_event(&Event::KeyUp { key: Key::Other("Control".into()), modifiers: ctrl });
    drag_to_more(&mut ui, to);
    assert_eq!(marks(&ui), 0, "with Ctrl nothing lands");
    release(&mut ui, to);
    let now = layout.get();
    assert!(now.is_floating("search"));
    let rect = now.floating_rect("search").expect("a box");
    assert!(rect.contains(to), "under the pointer: {rect:?}");
    assert_eq!(focused_name(&mut ui).as_deref(), Some("Move Search"));
}

#[test]
fn a_smaller_area_keeps_a_floating_panel_inside_without_changing_what_is_saved() {
    let layout = State::new(floating_start());
    let mut ui = dock_ui_of(area(&layout).height(260.0));
    let shown = named(&mut ui, "Layers, floating").bounds;
    assert_eq!((shown.y(), shown.max_y()), (60.0, 260.0), "moved up to fit");
    assert_eq!(layout.get().floating_rect("layers"), Some(Rect::new(300.0, 100.0, 240.0, 200.0)));
}

#[test]
fn the_app_can_refuse_floating() {
    let layout = State::new(start());
    let (view, asked, _) = guarded(&layout, |c| matches!(c, DockChange::Float { .. }));
    let mut ui = ui(view);
    tab_to(&mut ui, "Layers");
    press(&mut ui, Key::Character("f".into()), command_alt());
    assert_eq!(*asked.borrow(), [DockChange::Float { panel: "layers".into() }]);
    assert!(!layout.get().is_floating("layers"));
    assert_eq!(refusals(&mut ui), ["Not now"]);
}

/// An area with documents in the center and tools around it.
fn documents_area(layout: &State<DockLayout>) -> DockArea {
    let panels = vec![
        DockPanel::new("sales", "sales.rs").kind(DockPanelKind::Document),
        DockPanel::new("report", "Report")
            .kind(DockPanelKind::Document)
            .close_action(DockCloseAction::Remove),
        DockPanel::new("client", "client.rs").kind(DockPanelKind::Document),
        DockPanel::new("files", "Files"),
        DockPanel::new("output", "Output"),
    ];
    DockArea::new(layout.binding(), panels, |id| -> AnyView { Box::new(Content(id.to_owned())) })
        .height(600.0)
}

fn documents_start() -> DockLayout {
    DockLayout::new()
        .center(DockNode::tabs(["sales", "report", "client"]))
        .left(DockNode::tabs(["files"]), 240.0)
        .bottom(DockNode::tabs(["output"]), 150.0)
}

#[test]
fn a_document_stays_in_the_center_and_says_why() {
    let layout = State::new(documents_start());
    let mut ui = dock_ui_of(documents_area(&layout));
    tab_to(&mut ui, "sales.rs");
    let both = Modifiers { alt: true, shift: true, ..Modifiers::default() };
    press(&mut ui, Key::ArrowLeft, both);
    assert_eq!(layout.get().side_of("sales"), Some(DockSide::Center));
    assert_eq!(refusals(&mut ui), ["Documents stay in the center"]);
    let (from, files) = (center(&tab(&mut ui, "client.rs")), tab(&mut ui, "Files").bounds);
    let target = Point::new(files.max_x() - 1.0, files.y() + 8.0);
    drag_to(&mut ui, from, target);
    assert_eq!(marks(&ui), 0, "a dock marks nothing for a document");
    release(&mut ui, target);
    assert_eq!(layout.get().side_of("client"), Some(DockSide::Center));
}

#[test]
fn a_tool_stays_in_the_docks_and_its_menu_has_no_center() {
    let layout = State::new(documents_start());
    let mut ui = dock_ui_of(documents_area(&layout));
    let (from, sales) = (center(&tab(&mut ui, "Files")), tab(&mut ui, "sales.rs").bounds);
    let target = Point::new(sales.max_x() - 1.0, sales.y() + 8.0);
    drag_to(&mut ui, from, target);
    assert_eq!(marks(&ui), 0);
    release(&mut ui, target);
    assert_eq!(layout.get().side_of("files"), Some(DockSide::Left));
    tab_to(&mut ui, "Options of Files");
    press(&mut ui, Key::Enter, Modifiers::default());
    press(&mut ui, Key::ArrowRight, Modifiers::default());
    let mut seen = Vec::new();
    for _ in 0..8 {
        seen.push(focused_name(&mut ui).unwrap_or_default());
        press(&mut ui, Key::ArrowDown, Modifiers::default());
    }
    assert!(seen.iter().any(|name| name == "Top dock"), "{seen:?}");
    assert!(!seen.iter().any(|name| name == "Center" || name.starts_with("With sales")), "{seen:?}");
}

#[test]
fn free_placement_lets_them_go_anywhere_and_an_area_without_documents_is_as_before() {
    let layout = State::new(documents_start());
    let mut ui = dock_ui_of(documents_area(&layout).free_placement(true));
    tab_to(&mut ui, "sales.rs");
    let both = Modifiers { alt: true, shift: true, ..Modifiers::default() };
    press(&mut ui, Key::ArrowLeft, both);
    assert_eq!(layout.get().side_of("sales"), Some(DockSide::Left));
    let plain = State::new(start());
    let mut ui = dock_ui(&plain);
    tab_to(&mut ui, "Folders");
    press(&mut ui, Key::Enter, both);
    assert_eq!(
        plain.get().side_of("folders"),
        Some(DockSide::Center),
        "with no documents, tools go anywhere"
    );
}

#[test]
fn close_others_closes_the_rest_of_the_group_and_stops_where_the_app_says_no() {
    let layout = State::new(documents_start());
    let view = documents_area(&layout).on_change(|_, _, change| match change {
        DockChange::Close { panel } if panel == "client" => Err("client.rs has changes".into()),
        _ => Ok(()),
    });
    let mut ui = dock_ui_of(view);
    tab_to(&mut ui, "Options of sales.rs");
    press(&mut ui, Key::Enter, Modifiers::default());
    while focused_name(&mut ui).as_deref() != Some("Close others") {
        press(&mut ui, Key::ArrowDown, Modifiers::default());
    }
    press(&mut ui, Key::Enter, Modifiers::default());
    let now = layout.get();
    assert!(!now.contains("report"), "the first goes");
    assert!(now.contains("client"), "the one the app keeps stays");
    assert!(now.contains("sales"));
    assert!(refusals(&mut ui).contains(&"client.rs has changes".to_owned()));
    assert!(has(&mut ui, "Report closed"), "and what closed is said");
}

#[test]
fn a_panel_that_closes_for_good_is_not_offered_back_and_the_app_fills_an_empty_center() {
    let mut empty = documents_start();
    for id in ["sales", "report", "client"] {
        assert!(empty.close(id));
    }
    let layout = State::new(empty);
    let view =
        documents_area(&layout).on_documents_empty(|| -> AnyView { Box::new(Text::new("No document open")) });
    let mut ui = dock_ui_of(view);
    assert!(has(&mut ui, "No document open"), "the app's own words");
    assert!(has(&mut ui, "sales.rs") && has(&mut ui, "client.rs"), "what can come back, under them");
    assert!(!has(&mut ui, "Report"), "not what closed for good");
}
