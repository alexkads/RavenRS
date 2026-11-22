//! The test drivers of the grid, the diagram, the scheduler and the dock (#663, #665): they act
//! by keys, as a person does, through `ravenrs-e2e`'s real events.
#![cfg(feature = "flow")]
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::Size;
use ravenrs_e2e::{E2e, ErrorCode, Query};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;
use ravenrs_widgets_charts::testing::FlowCanvasDriver;
use ravenrs_widgets_dock::testing::DockDriver;
use ravenrs_widgets_grid::testing::DataGridDriver;
use ravenrs_widgets_scheduler::testing::SchedulerDriver;

fn people() -> DataGrid {
    let rows = [(40, "Bruno", 12.0), (41, "Ana", 80.0), (42, "Carla", 35.0), (43, "Diana", 5.0)]
        .into_iter()
        .map(|(id, name, amount)| vec![Value::from(id as f64), Value::from(name), Value::from(amount)])
        .collect();
    DataGrid::new(
        GridData::new(rows),
        vec![
            GridColumn::number("Id").width(80.0),
            GridColumn::text("Name").width(200.0).editable(),
            GridColumn::number("Amount").width(120.0),
        ],
    )
    .key_column(0)
    .filter_row(true)
    .toolbar(false)
    .height(400.0)
}

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_light()).viewport(Size::new(900.0, 600.0), 1.0)
}

/// The keys of the rows as the grid shows them, top to bottom.
fn order(e2e: &E2e) -> Vec<String> {
    let mut rows: Vec<_> = e2e
        .tree()
        .nodes
        .iter()
        .filter_map(|n| n.test_id.as_deref()?.strip_prefix("row:").map(|k| (n.bounds.y(), k.to_owned())))
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, k)| k).collect()
}

#[test]
fn a_cell_is_edited_by_the_rows_key_and_the_column() {
    let mut e2e = session(people());
    assert!(e2e.by_test_id("cell:42:Name").node().is_ok(), "{}", e2e.snapshot());
    DataGridDriver::new(&mut e2e).edit_cell("42", "Name", "Camila").unwrap();
    e2e.by_test_id("cell:42:Name").expect().to_have_name("Camila").unwrap();
    e2e.by_test_id("cell:41:Name").expect().to_have_name("Ana").unwrap();
    assert!(!e2e.snapshot().contains("Carla"), "{}", e2e.snapshot());
}

#[test]
fn a_cell_that_does_not_edit_is_refused() {
    let mut e2e = session(people());
    let refused = DataGridDriver::new(&mut e2e).edit_cell("42", "Amount", "1").unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Disabled, "{refused}");
}

#[test]
fn sorting_clicks_the_header_until_the_order_is_the_one_asked() {
    let mut e2e = session(people());
    assert_eq!(order(&e2e), ["40", "41", "42", "43"]);
    let mut grid = DataGridDriver::new(&mut e2e);
    grid.sort_by("Amount", SortDirection::Descending).unwrap();
    assert_eq!(order(&e2e), ["41", "42", "40", "43"]);
    let mut grid = DataGridDriver::new(&mut e2e);
    // From descending, the way to ascending passes through no order.
    grid.sort_by("Amount", SortDirection::Ascending).unwrap();
    assert_eq!(order(&e2e), ["43", "40", "42", "41"]);
    DataGridDriver::new(&mut e2e).clear_sort("Amount").unwrap();
    assert_eq!(order(&e2e), ["40", "41", "42", "43"]);
}

#[test]
fn filtering_writes_in_the_columns_field() {
    let mut e2e = session(people());
    DataGridDriver::new(&mut e2e).filter("Name", "an").unwrap();
    assert_eq!(order(&e2e), ["41", "43"], "{}", e2e.snapshot());
    DataGridDriver::new(&mut e2e).filter("Name", "").unwrap();
    assert_eq!(order(&e2e).len(), 4);
}

fn diagram(quiet: bool) -> (FlowCanvas, State<FlowGraph>) {
    let graph = State::new(
        FlowGraph::new()
            .node(FlowNode::new("pedido", "step", "Pedido", 48.0, 48.0).port(FlowPort::output("out")))
            .node(FlowNode::new("aprovar", "step", "Aprovar", 408.0, 200.0).port(FlowPort::input("in"))),
    );
    let canvas = FlowCanvas::new(graph.binding())
        .view(State::new(FlowViewport::at(ravenrs_core::Point::new(0.0, 0.0), 1.0)).binding())
        .kind("step", FlowKind::new(IconName::Package, "Passo"))
        .ports_on_hover(quiet)
        .size(900.0, 600.0);
    (canvas, graph)
}

#[test]
fn two_nodes_are_joined_by_their_ports() {
    for quiet in [false, true] {
        let (canvas, graph) = diagram(quiet);
        let mut e2e = session(canvas);
        FlowCanvasDriver::new(&mut e2e).connect("pedido.out", "aprovar.in").unwrap();
        let edges = graph.get().edges;
        assert_eq!(edges.len(), 1, "quiet ports: {quiet}\n{}", e2e.snapshot());
        assert_eq!((edges[0].from.node.as_str(), edges[0].to.node.as_str()), ("pedido", "aprovar"));
    }
}

#[test]
fn a_port_that_is_not_there_fails_without_joining_anything() {
    let (canvas, graph) = diagram(false);
    let mut e2e = session(canvas);
    let missing = FlowCanvasDriver::new(&mut e2e).connect("pedido.out", "aprovar.nada").unwrap_err();
    assert_eq!(missing.code(), ErrorCode::NotFound);
    assert!(graph.get().edges.is_empty());
    let bad = FlowCanvasDriver::new(&mut e2e).connect("pedido", "aprovar.in").unwrap_err();
    assert!(bad.to_string().contains("node.port"), "{bad}");
    let _ = Query::test_id("x");
}

/// A plan whose events follow what the scheduler says changed, as an app keeps them.
struct Plan {
    events: State<Vec<SchedulerEvent>>,
    selected: State<Option<u64>>,
}

impl View for Plan {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let events = self.events.clone();
        let scheduler = Scheduler::new(State::new(TimeAxis::days(day(1).date(), 14)).binding())
            .resources([Resource::new(1, "Ana"), Resource::new(2, "Bruno"), Resource::new(3, "Lia")])
            .events(self.events.get())
            .selection(self.selected.binding())
            .on_change(move |change| {
                events.update(|all| {
                    if let Some(e) = all.iter_mut().find(|e| Some(e.id) == change.id) {
                        (e.resource, e.start, e.end) = (change.resource, change.start, change.end);
                    }
                });
            });
        cx.child(&scheduler)
    }
}

fn day(d: u8) -> TimePoint {
    TimePoint::from_date(Date::new(2026, 11, d).expect("a date"))
}

fn plan() -> (E2e, State<Vec<SchedulerEvent>>) {
    let events = State::new(vec![
        SchedulerEvent::new(7, 1, day(3), day(5), "Viagem"),
        SchedulerEvent::new(8, 2, day(6), day(7), "Feira").locked(true),
        SchedulerEvent::new(9, 3, day(8), day(9), "Visita"),
    ]);
    let view = Plan { events: events.clone(), selected: State::new(None) };
    let e2e = E2e::new(view, Theme::raven_light()).viewport(Size::new(1400.0, 700.0), 1.0);
    (e2e, events)
}

fn event(events: &State<Vec<SchedulerEvent>>, id: u64) -> SchedulerEvent {
    events.get().into_iter().find(|e| e.id == id).expect("the event")
}

#[test]
fn an_event_moves_and_stretches_by_steps_of_the_snap() {
    let (mut e2e, events) = plan();
    let step = event(&events, 7);
    SchedulerDriver::new(&mut e2e).move_event(7, 2).unwrap();
    let moved = event(&events, 7);
    assert!(moved.start > step.start, "{moved:?}");
    let axis = TimeAxis::days(day(1).date(), 14);
    let length = |e: &SchedulerEvent| axis.x_of(e.end) - axis.x_of(e.start);
    assert!((length(&moved) - length(&step)).abs() < 0.5, "moving keeps the length");
    SchedulerDriver::new(&mut e2e).resize_event(7, 1).unwrap();
    let longer = event(&events, 7);
    assert_eq!(longer.start, moved.start);
    assert!(longer.end > moved.end, "{longer:?}");
    SchedulerDriver::new(&mut e2e).move_event(7, -2).unwrap();
    assert!(event(&events, 7).start < longer.start);
}

#[test]
fn an_event_goes_to_another_resource_by_its_id() {
    let (mut e2e, events) = plan();
    SchedulerDriver::new(&mut e2e).move_to_resource(7, 3).unwrap();
    assert_eq!(event(&events, 7).resource, 3, "{}", e2e.snapshot());
    SchedulerDriver::new(&mut e2e).move_to_resource(7, 1).unwrap();
    assert_eq!(event(&events, 7).resource, 1);
    let missing = SchedulerDriver::new(&mut e2e).move_to_resource(7, 99).unwrap_err();
    assert_eq!(missing.code(), ErrorCode::NotFound);
    assert_eq!(event(&events, 7).resource, 1, "nothing moved");
}

#[test]
fn a_locked_event_does_not_move_and_the_driver_says_so() {
    let (mut e2e, events) = plan();
    let before = event(&events, 8);
    let stuck = SchedulerDriver::new(&mut e2e).move_event(8, 1).unwrap_err();
    assert_eq!(stuck.code(), ErrorCode::Expectation, "{stuck}");
    assert!(stuck.to_string().contains("locked"), "{stuck}");
    assert_eq!(event(&events, 8), before);
}

fn dock(floatable: bool) -> (E2e, State<DockLayout>) {
    let layout = State::new(
        DockLayout::new().center(DockNode::tabs(["editor"])).bottom(DockNode::tabs(["output", "log"]), 160.0),
    );
    let panels = vec![
        DockPanel::new("editor", "Editor"),
        DockPanel::new("output", "Saída"),
        DockPanel::new("log", "Log"),
    ];
    let area = DockArea::new(layout.binding(), panels, |id| -> AnyView {
        Box::new(Text::new(format!("painel {id}")))
    })
    .floatable(floatable)
    .height(600.0);
    (E2e::new(area, Theme::raven_light()).viewport(Size::new(1200.0, 700.0), 1.0), layout)
}

#[test]
fn a_panel_floats_and_docks_back_by_its_id() {
    let (mut e2e, layout) = dock(true);
    DockDriver::new(&mut e2e).float("output").unwrap();
    assert!(layout.get().is_floating("output"));
    e2e.by_test_id("floating:output").expect().to_be_visible().unwrap();
    DockDriver::new(&mut e2e).dock_back("output").unwrap();
    assert!(!layout.get().is_floating("output"));
    e2e.by_test_id("panel:output").expect().to_be_visible().unwrap();
}

#[test]
fn an_area_that_does_not_float_says_so() {
    let (mut e2e, layout) = dock(false);
    let refused = DockDriver::new(&mut e2e).float("output").unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Expectation);
    assert!(!layout.get().is_floating("output"));
}

#[test]
fn the_arrows_move_the_event_that_was_clicked_not_the_one_that_had_the_focus() {
    let (mut e2e, events) = plan();
    let (first, before) = (event(&events, 7), event(&events, 9));
    SchedulerDriver::new(&mut e2e).move_event(9, 1).unwrap();
    assert!(event(&events, 9).start > before.start);
    assert_eq!(event(&events, 7), first, "the first event, the keyboard's way in before the click, stays");
}
