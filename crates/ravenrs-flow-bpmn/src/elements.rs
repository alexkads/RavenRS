//! The elements of BPMN 2.0 as nodes and connections of a `FlowGraph`: each node's `kind` is
//! the element's name in the XML (`task`, `exclusiveGateway`), each connection's `kind` is
//! empty for a sequence flow, or `association` or `messageFlow`.

use ravenrs_core::Rect;
use ravenrs_widgets::*;

/// A sequence flow's kind: none.
pub const SEQUENCE: &str = "";
/// An association, from an activity to a piece of data or a note.
pub const ASSOCIATION: &str = "association";
/// A message between participants.
pub const MESSAGE: &str = "messageFlow";
/// A lane, a group of the steps someone does.
pub const LANE: &str = "lane";

/// What a kind of element is, for the canvas, the palette and the checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Something that happens: a start, an end, a wait.
    Event,
    /// Work someone or something does.
    Activity,
    /// Where the way splits or joins.
    Gateway,
    /// Data and notes, outside the flow.
    Artifact,
    /// A lane.
    Lane,
}

/// The elements offered, by their XML name, with the words shown for each.
pub const ELEMENTS: [(&str, &str, Family); 14] = [
    ("startEvent", "Início", Family::Event),
    ("endEvent", "Fim", Family::Event),
    ("intermediateCatchEvent", "Espera", Family::Event),
    ("task", "Tarefa", Family::Activity),
    ("userTask", "Tarefa de usuário", Family::Activity),
    ("serviceTask", "Tarefa de serviço", Family::Activity),
    ("scriptTask", "Tarefa de script", Family::Activity),
    ("exclusiveGateway", "Decisão", Family::Gateway),
    ("parallelGateway", "Paralelo", Family::Gateway),
    ("inclusiveGateway", "Inclusivo", Family::Gateway),
    ("dataObjectReference", "Dado", Family::Artifact),
    ("dataStoreReference", "Base de dados", Family::Artifact),
    ("textAnnotation", "Anotação", Family::Artifact),
    (LANE, "Raia", Family::Lane),
];

/// The family of an element's kind, `None` for a kind that is not BPMN.
pub fn family(kind: &str) -> Option<Family> {
    ELEMENTS.iter().find(|(k, _, _)| *k == kind).map(|(_, _, f)| *f)
}

/// The words for an element's kind.
pub fn caption(kind: &str) -> &'static str {
    ELEMENTS.iter().find(|(k, _, _)| *k == kind).map_or("Elemento", |(_, c, _)| *c)
}

/// An element of this kind, at the origin.
pub fn element(kind: &str, id: &str, name: &str) -> FlowNode {
    let base = |w: f32, h: f32| FlowNode::new(id, kind, name, 0.0, 0.0).size(w, h);
    let flow = |node: FlowNode| {
        node.port(FlowPort::input("in").side(PortSide::Left))
            .port(FlowPort::input("top").side(PortSide::Top))
            .port(FlowPort::output("out").side(PortSide::Right))
            .port(FlowPort::output("down").side(PortSide::Bottom))
    };
    match kind {
        "startEvent" => base(40.0, 40.0)
            .shape(FlowShape::Ellipse)
            .styled(FlowStyle::new().stroke_width(2.0).title_below())
            .port(FlowPort::output("out").side(PortSide::Right))
            .port(FlowPort::output("down").side(PortSide::Bottom)),
        "endEvent" => base(40.0, 40.0)
            .shape(FlowShape::Ellipse)
            .styled(FlowStyle::new().stroke_width(4.0).title_below())
            .port(FlowPort::input("in").side(PortSide::Left))
            .port(FlowPort::input("top").side(PortSide::Top)),
        "intermediateCatchEvent" => flow(
            base(40.0, 40.0)
                .shape(FlowShape::Ellipse)
                .styled(FlowStyle::new().stroke_width(2.0).title_below()),
        ),
        "exclusiveGateway" | "parallelGateway" | "inclusiveGateway" => {
            let marker = match kind {
                "exclusiveGateway" => "×",
                "parallelGateway" => "+",
                _ => "O",
            };
            flow(
                base(56.0, 56.0)
                    .shape(FlowShape::Diamond)
                    .styled(FlowStyle::new().title_below().marker(marker)),
            )
            .port(FlowPort::output("up").side(PortSide::Top))
        }
        "dataObjectReference" => base(44.0, 56.0)
            .shape(FlowShape::Document)
            .styled(FlowStyle::new().title_below())
            .port(FlowPort::input("in").side(PortSide::Left))
            .port(FlowPort::output("out").side(PortSide::Right)),
        "dataStoreReference" => base(56.0, 52.0)
            .shape(FlowShape::Cylinder)
            .styled(FlowStyle::new().title_below())
            .port(FlowPort::input("in").side(PortSide::Left))
            .port(FlowPort::output("out").side(PortSide::Right)),
        "textAnnotation" => {
            base(160.0, 56.0).shape(FlowShape::Note).port(FlowPort::input("in").side(PortSide::Left))
        }
        LANE => {
            FlowNode { kind: LANE.into(), ..FlowNode::new_group(id, name, Rect::new(0.0, 0.0, 960.0, 200.0)) }
        }
        // Tasks are cards, which show their kind and its icon above the name.
        _ => flow(base(176.0, 72.0)),
    }
}

/// The icon a task's card shows.
pub fn icon(kind: &str) -> IconName {
    match kind {
        "userTask" => IconName::Users,
        "serviceTask" => IconName::Zap,
        "scriptTask" => IconName::FileText,
        _ => IconName::Square,
    }
}

/// A connection of `kind` from one element's port to another's, drawn as BPMN draws it.
pub fn connection(id: &str, from: FlowEndpoint, to: FlowEndpoint, kind: &str) -> FlowEdge {
    let edge = FlowEdge::new(id, from, to).kind(kind).shape(EdgeShape::Step);
    match kind {
        ASSOCIATION => FlowEdge { end: EdgeCap::None, ..edge.dashed().shape(EdgeShape::Straight) },
        MESSAGE => FlowEdge { start: EdgeCap::Circle, end: EdgeCap::OpenArrow, ..edge.dashed() },
        _ => edge,
    }
}

/// Makes every connection look as its ends say: one touching data or a note is an
/// association; the rest are sequence flows unless they are messages. Says whether anything
/// changed.
pub fn normalize(graph: &mut FlowGraph) -> bool {
    let families: Vec<(String, Option<Family>)> =
        graph.nodes.iter().map(|n| (n.id.clone(), family(&n.kind))).collect();
    let of = |id: &str| families.iter().find(|(i, _)| i == id).and_then(|(_, f)| *f);
    let mut changed = false;
    for edge in &mut graph.edges {
        let artifact =
            of(&edge.from.node) == Some(Family::Artifact) || of(&edge.to.node) == Some(Family::Artifact);
        let kind = match () {
            _ if artifact => ASSOCIATION,
            _ if edge.kind == MESSAGE => MESSAGE,
            _ => SEQUENCE,
        };
        if edge.kind != kind {
            let fixed = connection(&edge.id, edge.from.clone(), edge.to.clone(), kind);
            *edge = FlowEdge { label: edge.label.clone(), waypoints: edge.waypoints.clone(), ..fixed };
            changed = true;
        }
    }
    changed
}
