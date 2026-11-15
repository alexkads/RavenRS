//! BPMN: its XML against itself and against what other tools write, its checks, its
//! notation on the components, and the plans of a local model.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use ravenrs_flow_bpmn::{elements, plan as assistant, rules, xml};
use ravenrs_widgets::*;

#[test]
fn a_process_written_as_bpmn_reads_back_the_same() {
    let graph = ravenrs_flow_bpmn::example_process();
    let text = xml::to_xml(&graph, "Compra");
    assert!(text.contains("<bpmn:exclusiveGateway id=\"amount\" name=\"Valor alto?\">"));
    assert!(
        text.contains(
            "<bpmn:sequenceFlow id=\"f3\" sourceRef=\"amount\" targetRef=\"approve\" name=\"Sim\" />"
        )
    );
    assert!(text.contains("<bpmn:flowNodeRef>approve</bpmn:flowNodeRef>"));
    assert!(text.contains("<di:waypoint x=\"452\" y=\"100\" />"), "bend points are in the drawing");

    let (back, name) = xml::from_xml(&text).expect("it reads");
    assert_eq!(name, "Compra");
    for node in &graph.nodes {
        let read = back.find_node(&node.id).unwrap_or_else(|| panic!("{} is back", node.id));
        assert_eq!(
            (&read.kind, &read.title, read.x, read.y),
            (&node.kind, &node.title, node.x, node.y),
            "{}",
            node.id
        );
        assert_eq!(read.parent, node.parent, "{} stays in its lane", node.id);
    }
    for edge in &graph.edges {
        let read = back.find_edge(&edge.id).unwrap_or_else(|| panic!("{} is back", edge.id));
        assert_eq!(
            (&read.from, &read.to, &read.label, &read.kind),
            (&edge.from, &edge.to, &edge.label, &edge.kind),
            "{}",
            edge.id
        );
        assert_eq!(read.waypoints, edge.waypoints, "{}", edge.id);
    }
}

/// What a modeling tool writes: other prefixes, a condition, its own drawing.
const FOREIGN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" xmlns:bpmndi="http://www.omg.org/spec/BPMN/20100524/DI" xmlns:omgdc="http://www.omg.org/spec/DD/20100524/DC" xmlns:omgdi="http://www.omg.org/spec/DD/20100524/DI" id="d">
  <process id="p" name="Atendimento &amp; suporte">
    <startEvent id="s" name="Chamado aberto"><outgoing>a</outgoing></startEvent>
    <userTask id="t" name="Analisar"><incoming>a</incoming><outgoing>b</outgoing></userTask>
    <exclusiveGateway id="g" name="Resolvido?" />
    <endEvent id="e" name="Fechado" />
    <manualTask id="m" name="Ligar para o cliente" />
    <sequenceFlow id="a" sourceRef="s" targetRef="t" />
    <sequenceFlow id="b" sourceRef="t" targetRef="g" />
    <sequenceFlow id="c" name="Sim" sourceRef="g" targetRef="e"><conditionExpression>ok</conditionExpression></sequenceFlow>
    <sequenceFlow id="d2" name="Não" sourceRef="g" targetRef="m" />
  </process>
  <bpmndi:BPMNDiagram id="dd"><bpmndi:BPMNPlane id="pl" bpmnElement="p">
    <bpmndi:BPMNShape id="s_di" bpmnElement="s"><omgdc:Bounds x="100" y="100" width="36" height="36" /></bpmndi:BPMNShape>
    <bpmndi:BPMNShape id="t_di" bpmnElement="t"><omgdc:Bounds x="200" y="78" width="100" height="80" /></bpmndi:BPMNShape>
    <bpmndi:BPMNShape id="g_di" bpmnElement="g" isMarkerVisible="true"><omgdc:Bounds x="360" y="93" width="50" height="50" /></bpmndi:BPMNShape>
    <bpmndi:BPMNShape id="e_di" bpmnElement="e"><omgdc:Bounds x="480" y="100" width="36" height="36" /></bpmndi:BPMNShape>
    <bpmndi:BPMNShape id="m_di" bpmnElement="m"><omgdc:Bounds x="335" y="200" width="100" height="80" /></bpmndi:BPMNShape>
    <bpmndi:BPMNEdge id="a_di" bpmnElement="a"><omgdi:waypoint x="136" y="118" /><omgdi:waypoint x="200" y="118" /></bpmndi:BPMNEdge>
    <bpmndi:BPMNEdge id="d2_di" bpmnElement="d2"><omgdi:waypoint x="385" y="143" /><omgdi:waypoint x="385" y="200" /></bpmndi:BPMNEdge>
  </bpmndi:BPMNPlane></bpmndi:BPMNDiagram>
</definitions>"#;

#[test]
fn a_file_from_another_tool_opens_with_its_drawing() {
    let (graph, name) = xml::from_xml(FOREIGN).expect("it reads");
    assert_eq!(name, "Atendimento & suporte");
    assert_eq!(graph.nodes.len(), 5);
    let gateway = graph.find_node("g").expect("the gateway");
    assert_eq!((gateway.kind.as_str(), gateway.x, gateway.width), ("exclusiveGateway", 360.0, 50.0));
    assert_eq!(
        graph.find_node("m").map(|n| n.kind.as_str()),
        Some("task"),
        "a kind not drawn here is a task"
    );
    assert_eq!(graph.edges.len(), 4);
    let down = graph.find_edge("d2").expect("the way down");
    assert_eq!(down.from.port, "down", "it leaves by the side facing where it goes");
    assert_eq!(down.label.as_deref(), Some("Não"));
    assert!(
        rules::problems(&graph).is_empty() || !rules::problems(&graph).iter().any(|p| p.text.contains("início"))
    );
}

#[test]
fn a_file_without_a_drawing_is_arranged() {
    let plain =
        FOREIGN.split("<bpmndi:BPMNDiagram").next().expect("the process").to_owned() + "</definitions>";
    let (graph, _) = xml::from_xml(&plain).expect("it reads");
    let xs: Vec<f32> = graph.nodes.iter().map(|n| n.x).collect();
    assert!(xs.iter().any(|x| *x > 0.0), "placed by arranging: {xs:?}");
    assert!(xml::from_xml("<definitions/>").is_err());
}

#[test]
fn checks_say_what_is_wrong() {
    assert!(
        rules::problems(&ravenrs_flow_bpmn::example_process()).is_empty(),
        "{:?}",
        rules::problems(&ravenrs_flow_bpmn::example_process())
    );
    let mut broken = ravenrs_flow_bpmn::example_process();
    broken.remove(&["done".into()], &[]);
    broken.edges.retain(|e| e.id != "f3");
    let found: Vec<String> = rules::problems(&broken).into_iter().map(|p| p.text).collect();
    let found = found.join("\n");
    assert!(found.contains("não tem evento de fim"), "{found}");
    assert!(found.contains("“Aprovar pedido” não é alcançado"), "{found}");
    assert!(found.contains("tem um caminho só"), "{found}");
    let end = |n: &str, p: &str| FlowEndpoint::new(n, p);
    let g = ravenrs_flow_bpmn::example_process();
    assert!(rules::validate(&g, &end("request", "out"), &end("start", "in")).is_err());
    assert!(rules::validate(&g, &end("done", "out"), &end("request", "in")).is_err());
}

#[test]
fn lines_to_data_and_notes_become_associations() {
    let mut g = ravenrs_flow_bpmn::example_process();
    g.edges.push(FlowEdge::new("x", FlowEndpoint::new("approve", "out"), FlowEndpoint::new("note", "in")));
    assert!(elements::normalize(&mut g));
    let x = g.find_edge("x").expect("the line");
    assert_eq!((x.kind.as_str(), x.dashed, x.end), (elements::ASSOCIATION, true, EdgeCap::None));
    assert!(!elements::normalize(&mut g), "already tidy");
}

#[test]
fn a_plan_from_the_model_becomes_a_process_and_anything_else_does_not() {
    let answer = r#"Claro! Aqui está:
{"etapas":[{"id":"e1","tipo":"inicio","nome":"Pedido recebido"},{"id":"e2","tipo":"usuario","nome":"Conferir estoque"},
{"id":"e3","tipo":"decisao","nome":"Tem estoque?"},{"id":"e4","tipo":"servico","nome":"Emitir nota"},
{"id":"e5","tipo":"tarefa","nome":"Comprar do fornecedor"},{"id":"e6","tipo":"fim","nome":"Pedido enviado"},{"id":"e7","tipo":"esquisito","nome":"Algo"}],
"ligacoes":[{"de":"e1","para":"e2"},{"de":"e2","para":"e3"},{"de":"e3","para":"e4","rotulo":"Sim"},{"de":"e3","para":"e5","rotulo":"Não"},
{"de":"e5","para":"e4"},{"de":"e4","para":"e6"},{"de":"e6","para":"e1"},{"de":"e2","para":"zz"}]}
Espero ter ajudado."#;
    let graph = assistant::plan(answer).expect("a plan");
    assert_eq!(graph.nodes.len(), 7);
    assert_eq!(graph.find_node("e3").map(|n| n.kind.as_str()), Some("exclusiveGateway"));
    assert_eq!(graph.find_node("e7").map(|n| n.kind.as_str()), Some("task"), "an unknown type is a task");
    assert_eq!(graph.edges.len(), 6, "nothing leaves an end, nothing goes nowhere");
    let ways: Vec<&str> =
        graph.edges.iter().filter(|e| e.from.node == "e3").map(|e| e.from.port.as_str()).collect();
    assert_eq!(ways, ["out", "down"], "a decision's ways leave by different sides");
    assert!(graph.nodes.iter().any(|n| n.x > 0.0), "laid out");
    // Only the step the plan left loose is pointed out.
    let found = rules::problems(&graph);
    assert!(
        !found.is_empty() && found.iter().all(|p| p.text.contains("“Algo”") && p.nodes == ["e7"]),
        "{found:?}"
    );
    assert!(assistant::plan("Não sei fazer isso.").is_err());
    assert!(assistant::plan("{\"etapas\":[]}").is_err());
    assert!(
        assistant::summary(&graph)
            .contains("Tem estoque? (Decisão) -> Emitir nota (Tarefa de serviço) [Sim]")
    );
}

#[test]
fn the_notation_shapes_lines_and_feeds_the_palette() {
    let bpmn = ravenrs_flow_bpmn::notation();
    assert_eq!(bpmn.caption("exclusiveGateway"), Some("Decisão"));
    let mut ui = Ui::new(FlowToolbox::new().notation(&bpmn), ravenrs_style::Theme::raven_light());
    ui.set_viewport(ravenrs_core::Size::new(240.0, 900.0), 1.0);
    ui.update();
    let names: Vec<String> = ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
    assert!(names.iter().any(|n| n == "Eventos") && names.iter().any(|n| n == "Início"), "{names:?}");
}
