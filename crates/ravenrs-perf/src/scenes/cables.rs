//! Cables with physics (#575): many `CablePlug`s between widgets, a `FlowCanvas` whose
//! connections are cables and a full `PatchPanel` (#578). A window simulates them on the GPU; `--cpu-cables` measures the
//! twin on the CPU, the one WebGL2 uses.

use ravenrs_core::{Color, Point};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_widgets::*;

use crate::{Ceiling, Class, PerfScene, Setup, Step, ceiling, hover_between, nothing, resize};

/// How many cables the panel ties.
const CABLES: usize = 200;

/// Twenty columns of jacks on top and as many at the bottom, each top one tied to one
/// below, a few columns over; `shift` slides the bottom row sideways.
struct Panel {
    shift: State<f32>,
}

impl View for Panel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let jack = |cx: &mut BuildCx<'_>, x: f32, y: f32| {
            let style = LayoutStyle {
                absolute: Some((x, y)),
                width: Some(16.0),
                height: Some(16.0),
                ..LayoutStyle::leaf()
            };
            cx.container(style, &[], Paint::Quad(QuadPaint::new(Color::hex(0x2A2F3A))))
        };
        let shift = self.shift.get();
        let mut nodes = Vec::with_capacity(CABLES * 2);
        let mut pairs = Vec::with_capacity(CABLES);
        for i in 0..CABLES {
            let (column, row) = ((i % 20) as f32, (i / 20) as f32);
            let a = jack(cx, 20.0 + column * 62.0, 20.0 + row * 22.0);
            let b = jack(cx, 20.0 + ((column + 3.0) % 20.0) * 62.0 + shift, 560.0 + row * 22.0);
            nodes.extend([a, b]);
            pairs.push((a, b));
        }
        for (i, (a, b)) in pairs.into_iter().enumerate() {
            CablePlug::between(a, b)
                .sides(CableSide::Bottom, CableSide::Top)
                .key(i as u64)
                .thickness(3.0)
                .plugs(true)
                // At rest from the start, so `idle` measures settled cables and `drag` the swing.
                .grow(false)
                .build(cx);
        }
        let style =
            LayoutStyle { width: Some(1280.0), height: Some(800.0), ..LayoutStyle::stack(Axis::Vertical) };
        cx.container(style, &nodes, Paint::None)
    }
}

/// Moves the bottom row a pixel each round, as a hand dragging it: every cable swings.
fn slide(shift: State<f32>) -> Step {
    Box::new(move |_, round| {
        shift.set((round % 40) as f32);
        true
    })
}

fn panel(interaction: &str) -> Setup {
    let shift = State::new(0.0);
    let root = Panel { shift: shift.clone() };
    let step = match interaction {
        "drag" => slide(shift),
        "hover" => hover_between(Point::new(300.0, 400.0), Point::new(900.0, 420.0)),
        "resize" => resize(),
        _ => nothing(),
    };
    Setup { root: Box::new(root), step }
}

/// Sixty nodes in six rows, each tied to the next and to the one below.
fn graph() -> FlowGraph {
    grid(10, 6)
}

/// `columns` × `rows` nodes, each tied to the next in its row and to the one below: 1.000
/// nodes and about 1.900 connections at 40 × 25, as the #238 asks to measure.
fn grid(columns: usize, rows: usize) -> FlowGraph {
    let count = columns * rows;
    let mut graph = FlowGraph::new();
    for i in 0..count {
        let (column, row) = ((i % columns) as f32, (i / columns) as f32);
        graph = graph.node(
            FlowNode::new(
                format!("n{i}"),
                "step",
                format!("Etapa {i}"),
                20.0 + column * 125.0,
                20.0 + row * 125.0,
            )
            .port(FlowPort::input("in"))
            .port(FlowPort::output("out")),
        );
    }
    for i in 0..count {
        for next in [i + 1, i + columns] {
            if next < count && (next != i + 1 || next % columns != 0) {
                graph = graph.edge(FlowEdge::new(
                    format!("e{i}-{next}"),
                    FlowEndpoint::new(format!("n{i}"), "out"),
                    FlowEndpoint::new(format!("n{next}"), "in"),
                ));
            }
        }
    }
    graph
}

fn flow(interaction: &str) -> Setup {
    flow_of(graph(), interaction)
}

fn flow_large(interaction: &str) -> Setup {
    flow_of(grid(40, 25), interaction)
}

fn flow_of(graph: FlowGraph, interaction: &str) -> Setup {
    let graph = State::new(graph);
    let canvas = FlowCanvas::new(graph.binding()).cables(true).minimap(false).size(1280.0, 800.0);
    let step = match interaction {
        // A node carried across: its cables swing and the ones around follow.
        "drag" => Box::new(move |_: &mut ravenrs_widgets::Ui, round: u32| {
            graph.update(|g| {
                if let Some(node) = g.nodes.first_mut() {
                    node.x = 20.0 + (round % 40) as f32 * 2.0;
                }
            });
            true
        }) as Step,
        "hover" => hover_between(Point::new(300.0, 300.0), Point::new(700.0, 500.0)),
        "resize" => resize(),
        _ => nothing(),
    };
    Setup { root: Box::new(canvas), step }
}

/// A panel of 48 jacks with 24 cables plugged: the back of a full rack.
fn patch(interaction: &str) -> Setup {
    let mut jacks = Vec::new();
    let mut links = Vec::new();
    for i in 0..24 {
        let x = 40.0 + (i % 12) as f32 * 100.0;
        let y = if i < 12 { 60.0 } else { 420.0 };
        jacks.push(Jack::output(format!("o{i}"), format!("Saída {i}")).signal("áudio").at(x, y));
        jacks.push(Jack::input(format!("i{i}"), format!("Entrada {i}")).signal("áudio").at(x, y + 200.0));
        links.push(PatchLink::new(format!("o{i}"), format!("i{}", (i + 5) % 24)));
    }
    let links = State::new(links);
    let root = PatchPanel::new(jacks, links.binding()).size(1240.0, 760.0);
    let step = match interaction {
        "hover" => hover_between(Point::new(140.0, 60.0), Point::new(640.0, 260.0)),
        "resize" => resize(),
        _ => nothing(),
    };
    Setup { root: Box::new(root), step }
}

/// Settled cables cost nothing: no frames while nothing moves.
const CABLE_CEILINGS: &[Ceiling] = &[ceiling("idle", "frames", 0)];

pub(super) fn scenes() -> Vec<PerfScene> {
    vec![
        PerfScene {
            name: "cables",
            covers: &["CablePlug"],
            class: Class::Animated,
            interactions: &["drag", "hover", "idle", "resize"],
            make: panel,
            ceilings: CABLE_CEILINGS,
        },
        PerfScene {
            name: "flow-cables",
            covers: &["FlowCanvas"],
            class: Class::Animated,
            interactions: &["drag", "hover", "idle", "resize"],
            make: flow,
            ceilings: CABLE_CEILINGS,
        },
        PerfScene {
            name: "flow-cables-large",
            covers: &["FlowCanvas"],
            class: Class::Animated,
            interactions: &["drag", "idle", "resize"],
            make: flow_large,
            ceilings: CABLE_CEILINGS,
        },
        PerfScene {
            name: "patch-panel",
            covers: &["PatchPanel"],
            class: Class::Static,
            interactions: &["hover", "idle", "resize"],
            make: patch,
            ceilings: CABLE_CEILINGS,
        },
    ]
}
