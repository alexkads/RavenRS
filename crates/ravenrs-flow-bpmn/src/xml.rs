//! BPMN 2.0 XML: a diagram written with its process and its drawing (BPMNDI), and read back
//! from any tool that writes the same.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use ravenrs_core::{Point, Rect};
use ravenrs_widgets::*;

use crate::elements::{self, ASSOCIATION, Family, LANE, MESSAGE};

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The process as BPMN 2.0 XML, with its drawing, named `name`.
pub fn to_xml(graph: &FlowGraph, name: &str) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<bpmn:definitions xmlns:bpmn=\"http://www.omg.org/spec/BPMN/20100524/MODEL\" \
xmlns:bpmndi=\"http://www.omg.org/spec/BPMN/20100524/DI\" \
xmlns:dc=\"http://www.omg.org/spec/DD/20100524/DC\" \
xmlns:di=\"http://www.omg.org/spec/DD/20100524/DI\" \
id=\"Definitions_1\" targetNamespace=\"http://ravenrs.dev/bpmn\" exporter=\"RavenRS\">\n",
    );
    out.push_str(&format!(
        "  <bpmn:process id=\"Process_1\" name=\"{}\" isExecutable=\"false\">\n",
        escaped(name)
    ));
    let lanes: Vec<&FlowNode> = graph.nodes.iter().filter(|n| n.kind == LANE).collect();
    if !lanes.is_empty() {
        out.push_str("    <bpmn:laneSet id=\"LaneSet_1\">\n");
        for lane in &lanes {
            out.push_str(&format!(
                "      <bpmn:lane id=\"{}\" name=\"{}\">\n",
                escaped(&lane.id),
                escaped(&lane.title)
            ));
            for member in graph.children(&lane.id) {
                out.push_str(&format!(
                    "        <bpmn:flowNodeRef>{}</bpmn:flowNodeRef>\n",
                    escaped(&member.id)
                ));
            }
            out.push_str("      </bpmn:lane>\n");
        }
        out.push_str("    </bpmn:laneSet>\n");
    }
    for node in graph.nodes.iter().filter(|n| n.kind != LANE) {
        let kind = if elements::family(&node.kind).is_some() { node.kind.as_str() } else { "task" };
        if kind == "textAnnotation" {
            out.push_str(&format!(
                "    <bpmn:textAnnotation id=\"{}\"><bpmn:text>{}</bpmn:text></bpmn:textAnnotation>\n",
                escaped(&node.id),
                escaped(&node.title)
            ));
            continue;
        }
        let incoming: Vec<&FlowEdge> =
            graph.edges.iter().filter(|e| e.to.node == node.id && e.kind.is_empty()).collect();
        let outgoing: Vec<&FlowEdge> =
            graph.edges.iter().filter(|e| e.from.node == node.id && e.kind.is_empty()).collect();
        out.push_str(&format!(
            "    <bpmn:{kind} id=\"{}\" name=\"{}\">\n",
            escaped(&node.id),
            escaped(&node.title)
        ));
        for e in incoming {
            out.push_str(&format!("      <bpmn:incoming>{}</bpmn:incoming>\n", escaped(&e.id)));
        }
        for e in outgoing {
            out.push_str(&format!("      <bpmn:outgoing>{}</bpmn:outgoing>\n", escaped(&e.id)));
        }
        if kind == "intermediateCatchEvent" {
            out.push_str("      <bpmn:timerEventDefinition />\n");
        }
        out.push_str(&format!("    </bpmn:{kind}>\n"));
    }
    for edge in &graph.edges {
        let tag = match edge.kind.as_str() {
            ASSOCIATION => "association",
            MESSAGE => continue,
            _ => "sequenceFlow",
        };
        let name = edge.label.as_deref().map(|l| format!(" name=\"{}\"", escaped(l))).unwrap_or_default();
        out.push_str(&format!(
            "    <bpmn:{tag} id=\"{}\" sourceRef=\"{}\" targetRef=\"{}\"{name} />\n",
            escaped(&edge.id),
            escaped(&edge.from.node),
            escaped(&edge.to.node)
        ));
    }
    out.push_str("  </bpmn:process>\n");
    out.push_str("  <bpmndi:BPMNDiagram id=\"BPMNDiagram_1\">\n    <bpmndi:BPMNPlane id=\"BPMNPlane_1\" bpmnElement=\"Process_1\">\n");
    for node in &graph.nodes {
        let horizontal = if node.kind == LANE { " isHorizontal=\"true\"" } else { "" };
        out.push_str(&format!(
            "      <bpmndi:BPMNShape id=\"{0}_di\" bpmnElement=\"{0}\"{horizontal}>\n        <dc:Bounds x=\"{1}\" y=\"{2}\" width=\"{3}\" height=\"{4}\" />\n      </bpmndi:BPMNShape>\n",
            escaped(&node.id),
            node.x,
            node.y,
            node.width,
            node.height
        ));
    }
    for edge in graph.edges.iter().filter(|e| e.kind != MESSAGE) {
        let Some(points) = graph.edge_controls(edge) else { continue };
        out.push_str(&format!(
            "      <bpmndi:BPMNEdge id=\"{0}_di\" bpmnElement=\"{0}\">\n",
            escaped(&edge.id)
        ));
        for p in points {
            out.push_str(&format!("        <di:waypoint x=\"{}\" y=\"{}\" />\n", p.x.round(), p.y.round()));
        }
        out.push_str("      </bpmndi:BPMNEdge>\n");
    }
    out.push_str("    </bpmndi:BPMNPlane>\n  </bpmndi:BPMNDiagram>\n</bpmn:definitions>\n");
    out
}

/// What was read of one element, before it is placed.
#[derive(Default)]
struct Read {
    nodes: Vec<(String, String, String)>,
    flows: Vec<(String, String, String, String, Option<String>)>,
    lanes: Vec<(String, String, Vec<String>)>,
    bounds: Vec<(String, Rect)>,
    waypoints: Vec<(String, Vec<Point>)>,
    process: Option<String>,
}

fn attribute(tag: &BytesStart<'_>, name: &str) -> Option<String> {
    let found = tag.attributes().flatten().find(|a| a.key.local_name().as_ref() == name.as_bytes())?;
    let raw = String::from_utf8_lossy(&found.value).into_owned();
    Some(quick_xml::escape::unescape(&raw).map_or(raw.clone(), |text| text.into_owned()))
}

fn number(tag: &BytesStart<'_>, name: &str) -> f32 {
    attribute(tag, name).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

/// The process named in the XML, as a diagram, and its name. Elements this editor does not
/// draw are read as tasks, so nothing of the process is lost; what has no drawing is placed
/// by arranging.
pub fn from_xml(text: &str) -> Result<(FlowGraph, String), String> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut read = Read::default();
    let mut open: Vec<String> = Vec::new();
    let mut element: Option<String> = None;
    let mut lane: Option<(String, String, Vec<String>)> = None;
    let mut text_of: Option<String> = None;
    loop {
        let event = reader.read_event().map_err(|e| format!("XML: {e}"))?;
        match &event {
            Event::Start(tag) | Event::Empty(tag) => {
                let local = String::from_utf8_lossy(tag.local_name().as_ref()).into_owned();
                let id = attribute(tag, "id").unwrap_or_default();
                match local.as_str() {
                    "process" => read.process = attribute(tag, "name"),
                    "lane" => lane = Some((id, attribute(tag, "name").unwrap_or_default(), Vec::new())),
                    "sequenceFlow" | "association" | "messageFlow" => {
                        let kind = match local.as_str() {
                            "association" => ASSOCIATION,
                            "messageFlow" => MESSAGE,
                            _ => "",
                        };
                        read.flows.push((
                            id,
                            kind.to_owned(),
                            attribute(tag, "sourceRef").unwrap_or_default(),
                            attribute(tag, "targetRef").unwrap_or_default(),
                            attribute(tag, "name").filter(|n| !n.is_empty()),
                        ));
                    }
                    "BPMNShape" => {
                        element = attribute(tag, "bpmnElement");
                    }
                    "BPMNEdge" => {
                        let e = attribute(tag, "bpmnElement").unwrap_or_default();
                        read.waypoints.push((e.clone(), Vec::new()));
                        element = Some(e);
                    }
                    "Bounds" => {
                        if let Some(e) = &element {
                            let r = Rect::new(
                                number(tag, "x"),
                                number(tag, "y"),
                                number(tag, "width"),
                                number(tag, "height"),
                            );
                            read.bounds.push((e.clone(), r));
                        }
                    }
                    "waypoint" => {
                        if let Some((_, points)) = read.waypoints.last_mut() {
                            points.push(Point::new(number(tag, "x"), number(tag, "y")));
                        }
                    }
                    "flowNodeRef" => text_of = Some("flowNodeRef".into()),
                    "text" if open.last().is_some_and(|o| o == "textAnnotation") => {
                        text_of = Some("text".into())
                    }
                    _ if elements::family(&local).is_some_and(|f| f != Family::Lane)
                        || local.ends_with("Task")
                        || local.ends_with("Event")
                        || local.ends_with("Gateway")
                        || local == "subProcess"
                        || local == "callActivity" =>
                    {
                        let kind = if elements::family(&local).is_some() {
                            local.clone()
                        } else if local.ends_with("Event") {
                            "intermediateCatchEvent".into()
                        } else if local.ends_with("Gateway") {
                            "exclusiveGateway".into()
                        } else {
                            "task".into()
                        };
                        read.nodes.push((id, kind, attribute(tag, "name").unwrap_or_default()));
                    }
                    _ => {}
                }
                if matches!(event, Event::Start(_)) {
                    open.push(local);
                }
            }
            Event::Text(t) => {
                let value = t.decode().map(|v| v.into_owned()).unwrap_or_default();
                match text_of.take().as_deref() {
                    Some("flowNodeRef") => {
                        if let Some((_, _, members)) = &mut lane {
                            members.push(value.trim().to_owned());
                        }
                    }
                    Some("text") => {
                        if let Some(last) = read.nodes.last_mut().filter(|n| n.1 == "textAnnotation") {
                            last.2 = value.trim().to_owned();
                        }
                    }
                    _ => {}
                }
            }
            Event::End(tag) => {
                let local = String::from_utf8_lossy(tag.local_name().as_ref()).into_owned();
                if local == "lane" {
                    read.lanes.extend(lane.take());
                }
                if local == "BPMNShape" || local == "BPMNEdge" {
                    element = None;
                }
                open.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    build(read)
}

/// The port of `node` of this direction on the side facing `toward`.
fn facing(node: &FlowNode, direction: PortDirection, toward: Point) -> Option<String> {
    node.ports
        .iter()
        .filter(|p| p.direction == direction)
        .filter_map(|p| Some((node.port_point(&p.id)?, p.id.clone())))
        .min_by(|a, b| {
            let d = |q: Point| (q.x - toward.x).hypot(q.y - toward.y);
            d(a.0).total_cmp(&d(b.0))
        })
        .map(|(_, id)| id)
}

fn build(read: Read) -> Result<(FlowGraph, String), String> {
    if read.nodes.is_empty() {
        return Err("Nenhum elemento de processo no arquivo".into());
    }
    let mut graph = FlowGraph::new();
    let placed = |id: &str| read.bounds.iter().find(|(e, _)| e == id).map(|(_, r)| *r);
    for (id, name, _) in &read.lanes {
        let mut lane = elements::element(LANE, id, name);
        if let Some(r) = placed(id) {
            lane = FlowNode { x: r.x(), y: r.y(), width: r.width(), height: r.height(), ..lane };
        }
        graph = graph.node(lane);
    }
    for (id, kind, name) in &read.nodes {
        let mut node = elements::element(kind, id, name);
        if let Some(r) = placed(id) {
            node = FlowNode {
                x: r.x(),
                y: r.y(),
                width: r.width().max(1.0),
                height: r.height().max(1.0),
                ..node
            };
        }
        if let Some((lane, _, _)) = read.lanes.iter().find(|(_, _, members)| members.contains(id)) {
            node = node.parent(lane.clone());
        }
        graph = graph.node(node);
    }
    let all_placed = read.nodes.iter().all(|(id, _, _)| placed(id).is_some());
    for (id, kind, from, to, name) in &read.flows {
        let (Some(a), Some(b)) = (graph.find_node(from).cloned(), graph.find_node(to).cloned()) else {
            continue;
        };
        let route = read.waypoints.iter().find(|(e, _)| e == id).map(|(_, p)| p.clone()).unwrap_or_default();
        // The drawing starts and ends on the ports it uses; without one, the sides facing.
        let first_toward = route.first().copied().unwrap_or(b.rect().center());
        let last_from = route.last().copied().unwrap_or(a.rect().center());
        let out = facing(&a, PortDirection::Output, first_toward)
            .or_else(|| facing(&a, PortDirection::Input, first_toward));
        let into = facing(&b, PortDirection::Input, last_from)
            .or_else(|| facing(&b, PortDirection::Output, last_from));
        let (Some(out), Some(into)) = (out, into) else { continue };
        let mut edge =
            elements::connection(id, FlowEndpoint::new(from, out), FlowEndpoint::new(to, into), kind);
        edge.label = name.clone();
        // The drawing's own corners, between its ends, become bend points.
        if route.len() > 2 {
            edge.waypoints = route[1..route.len() - 1].iter().map(|p| (p.x, p.y)).collect();
        }
        graph = graph.edge(edge);
    }
    if !all_placed {
        FlowLayout::new(FlowDirection::Right).apply(&mut graph);
    }
    Ok((graph, read.process.unwrap_or_default()))
}
