//! The test drivers of the grid and the diagram (#663): they act by keys, as a person does,
//! through `ravenrs-e2e`'s real events.
#![cfg(feature = "flow")]

use ravenrs_core::Size;
use ravenrs_e2e::{E2e, ErrorCode, Query};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;
use ravenrs_widgets_charts::testing::FlowCanvasDriver;
use ravenrs_widgets_grid::testing::DataGridDriver;

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
