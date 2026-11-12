//! `Timeline` and `ToolRail`: blocks on tracks, what is done to them, and tools.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessNode, AccessTree, Event, Point, PointerButton, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn node<'a>(tree: &'a AccessTree, name: &str) -> &'a AccessNode {
    tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name)
}

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(1000.0, 400.0), 1.0);
    ui.update();
    ui
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove { position: to });
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
}

type Log = Rc<RefCell<Vec<TimelineEdit>>>;

fn timeline(edits: &Log, seeks: &Rc<RefCell<Vec<u64>>>, picks: &Rc<RefCell<Vec<u64>>>) -> Timeline {
    let (edits, seeks, picks) = (edits.clone(), seeks.clone(), picks.clone());
    Timeline::new(vec![
        TimelineTrack::new(
            "Notes",
            34.0,
            vec![TimelineItem::new(1, 2_000, 2_000, "Callout").movable().trimmable()],
        ),
        TimelineTrack::new("Film", 56.0, vec![TimelineItem::new(2, 0, 10_000, "Take").tone(Tone::Accent)]),
    ])
    .markers(vec![TimelineMarker { at_ms: 5_000, title: "Chapter".to_owned() }])
    .position_ms(1_000)
    .pixels_per_second(100.0)
    .on_seek(move |ms| seeks.borrow_mut().push(ms))
    .on_select(move |id| picks.borrow_mut().push(id))
    .on_edit(move |edit| edits.borrow_mut().push(edit))
}

#[test]
fn blocks_sit_by_time_on_their_tracks_and_select_with_a_click() {
    let (edits, seeks, picks): (Log, _, _) = (Log::default(), Rc::default(), Rc::default());
    let mut ui = ui(timeline(&edits, &seeks, &picks));
    let tree = ui.access_tree();
    let (note, film) = (node(&tree, "Callout").bounds, node(&tree, "Take").bounds);
    assert!((note.x() - film.x() - 200.0).abs() < 1.0, "two seconds at 100 pixels each");
    assert!(note.y() < film.y());
    let id = node(&tree, "Take").id;
    ui.perform_action(id, ravenrs_core::AccessAction::Click);
    assert_eq!(*picks.borrow(), [2]);
}

#[test]
fn a_marker_takes_the_playhead_to_it() {
    let (edits, seeks, picks): (Log, _, _) = (Log::default(), Rc::default(), Rc::default());
    let mut ui = ui(timeline(&edits, &seeks, &picks));
    let tree = ui.access_tree();
    ui.perform_action(node(&tree, "Chapter").id, ravenrs_core::AccessAction::Click);
    assert_eq!(*seeks.borrow(), [5_000]);
}

#[test]
fn dragging_moves_and_trims_what_allows_it() {
    let (edits, seeks, picks): (Log, _, _) = (Log::default(), Rc::default(), Rc::default());
    let mut ui = ui(timeline(&edits, &seeks, &picks));
    let tree = ui.access_tree();
    let note = node(&tree, "Callout").bounds;
    let film = node(&tree, "Take").bounds;

    let middle = note.center();
    drag(&mut ui, middle, Point::new(middle.x + 100.0, middle.y));
    assert!(edits.borrow().contains(&TimelineEdit::Began { id: 1 }));
    assert!(
        edits
            .borrow()
            .iter()
            .any(|e| matches!(e, TimelineEdit::Moved { id: 1, start_ms } if start_ms.abs_diff(3_000) < 20))
    );

    edits.borrow_mut().clear();
    let end = Point::new(note.max_x() - 3.0, note.center().y);
    drag(&mut ui, end, Point::new(end.x + 50.0, end.y));
    assert!(
        edits
            .borrow()
            .iter()
            .any(|e| matches!(e, TimelineEdit::TrimmedEnd { id: 1, by_ms } if by_ms.abs_diff(500) < 20))
    );

    edits.borrow_mut().clear();
    let start = Point::new(note.x() + 3.0, note.center().y);
    drag(&mut ui, start, Point::new(start.x - 50.0, start.y));
    assert!(
        edits
            .borrow()
            .iter()
            .any(|e| matches!(e, TimelineEdit::TrimmedStart { id: 1, by_ms } if by_ms.abs_diff(-500) < 20))
    );

    edits.borrow_mut().clear();
    drag(&mut ui, film.center(), Point::new(film.center().x + 100.0, film.center().y));
    assert!(edits.borrow().is_empty(), "this one allows neither");
}

#[test]
fn dragging_the_ruler_moves_the_playhead() {
    let (edits, seeks, picks): (Log, _, _) = (Log::default(), Rc::default(), Rc::default());
    let mut ui = ui(timeline(&edits, &seeks, &picks));
    let tree = ui.access_tree();
    let film = node(&tree, "Take").bounds;
    let on_ruler = Point::new(film.x() + 100.0, film.y() - 34.0 - 26.0 + 2.0);
    drag(&mut ui, on_ruler, Point::new(film.x() + 400.0, on_ruler.y));
    assert!(seeks.borrow().iter().any(|ms| ms.abs_diff(4_000) < 50), "{:?}", seeks.borrow());
}

#[test]
fn a_tool_rail_opens_the_tool_clicked() {
    let open = State::new(0);
    let mut ui = ui(ToolRail::new(open.binding(), [(IconName::Type, "Text"), (IconName::ZoomIn, "Zoom")]));
    let tree = ui.access_tree();
    ui.perform_action(node(&tree, "Zoom").id, ravenrs_core::AccessAction::Click);
    assert_eq!(open.get(), 1);
    assert!(node(&ui.access_tree(), "Zoom").focusable);
}
