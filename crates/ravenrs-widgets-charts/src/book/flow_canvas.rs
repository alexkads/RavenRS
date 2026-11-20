//! The stories of `FlowCanvas`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use ravenrs_core::*;
#[allow(unused_imports)]
use ravenrs_layout::*;
#[allow(unused_imports)]
use ravenrs_reactive::*;
#[allow(unused_imports)]
use ravenrs_style::*;
#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `FlowCanvas`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_cables = args.bool("cables");
        let arg_controls = args.bool("controls");
        let arg_editable_titles = args.bool("editable_titles");
        let arg_grid = args.number("grid") as f32;
        let arg_grow = args.number("grow") as f32;
        let arg_label = args.text("label");
        let arg_locked = args.bool("locked");
        let arg_minimap = args.bool("minimap");
        let arg_ports_on_hover = args.bool("ports_on_hover");
        let arg_quick_create = args.bool("quick_create");
        let arg_read_only = args.bool("read_only");
        let arg_resizable = args.bool("resizable");
        let arg_rotatable = args.bool("rotatable");
        let arg_snap = args.bool("snap");

        let graph = State::new(
            FlowGraph::new()
                .node(
                    FlowNode::new("sale", "trigger", "New sale", 0.0, 96.0)
                        .subtitle("Registers 1 and 2")
                        .port(FlowPort::output("out").kind("sale")),
                )
                .node(
                    FlowNode::new("stock", "action", "Check stock", 264.0, 96.0)
                        .subtitle("For each item")
                        .port(FlowPort::input("in").kind("sale"))
                        .port(FlowPort::output("out").kind("sale")),
                )
                .node(
                    FlowNode::new("enough", "condition", "Enough in stock?", 528.0, 96.0)
                        .port(FlowPort::input("in").kind("sale"))
                        .port(FlowPort::output("yes").side(PortSide::Bottom).kind("sale"))
                        .port(FlowPort::output("no").side(PortSide::Bottom).kind("shortage")),
                )
                .node(
                    FlowNode::new("invoice", "action", "Issue receipt", 360.0, 312.0)
                        .port(FlowPort::input("in").side(PortSide::Top).kind("sale")),
                )
                .node(
                    FlowNode::new("warn", "action", "Tell the manager", 648.0, 312.0)
                        .status(FlowNodeStatus::Warning("No chat chosen".into()))
                        .port(FlowPort::input("in").side(PortSide::Top).kind("shortage")),
                )
                .edge(
                    FlowEdge::new("e1", FlowEndpoint::new("sale", "out"), FlowEndpoint::new("stock", "in"))
                        .animated(true),
                )
                .edge(FlowEdge::new(
                    "e2",
                    FlowEndpoint::new("stock", "out"),
                    FlowEndpoint::new("enough", "in"),
                ))
                .edge(
                    FlowEdge::new(
                        "e3",
                        FlowEndpoint::new("enough", "yes"),
                        FlowEndpoint::new("invoice", "in"),
                    )
                    .label("Yes"),
                )
                .edge(
                    FlowEdge::new("e4", FlowEndpoint::new("enough", "no"), FlowEndpoint::new("warn", "in"))
                        .shape(EdgeShape::Step)
                        .label("No"),
                ),
        );
        let history = State::new(FlowHistory::new());
        let json = State::new(args.bool("json"));
        let (undo, redo) = (history.clone(), history.clone());
        let (shown, written) = (json.clone(), graph.clone());
        let canvas = {
            let mut it = FlowCanvas::new(graph.binding())
                .history(history.binding())
                .kind("trigger", FlowKind::new(IconName::Zap, "Trigger").tone(Semantic::Warning))
                .kind("action", FlowKind::new(IconName::Package, "Action"))
                .kind("condition", FlowKind::new(IconName::GitBranch, "Condition").tone(Semantic::Success))
                .validate(|graph, from, to| {
                    let carries = |end: &FlowEndpoint| graph.port(end).and_then(|port| port.kind.clone());
                    if carries(from) == carries(to) {
                        Ok(())
                    } else {
                        Err(String::from("This input takes something else"))
                    }
                })
                .size(880.0, 368.0);
            it = it.cables(arg_cables);
            it = it.controls(arg_controls);
            it = it.editable_titles(arg_editable_titles);
            it = it.grid(arg_grid);
            it = it.grow(arg_grow);
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it = it.locked(arg_locked);
            it = it.minimap(arg_minimap);
            it = it.ports_on_hover(arg_ports_on_hover);
            it = it.quick_create(arg_quick_create);
            it = it.read_only(arg_read_only);
            it = it.resizable(arg_resizable);
            it = it.rotatable(arg_rotatable);
            it = it.snap(arg_snap);
            it
        };
        VStack::new((
            view_fn(move || {
                // Undo and redo here go through the same history as Cmd+Z in the canvas.
                let (back, again) = (undo.clone(), redo.clone());
                let (g1, g2) = (written.clone(), written.clone());
                let toggle = shown.clone();
                HStack::new((
                    Button::new("Undo")
                        .style(ButtonStyle::Secondary)
                        .icon(IconName::Undo2)
                        .disabled(!undo.get().can_undo())
                        .on_click(move || {
                            let mut h = back.get();
                            if let Some(before) = h.undo(g1.get()) {
                                back.set(h);
                                g1.set(before);
                            }
                        }),
                    Button::new("Redo")
                        .style(ButtonStyle::Secondary)
                        .icon(IconName::Redo2)
                        .disabled(!redo.get().can_redo())
                        .on_click(move || {
                            let mut h = again.get();
                            if let Some(after) = h.redo(g2.get()) {
                                again.set(h);
                                g2.set(after);
                            }
                        }),
                    Button::new("Show JSON")
                        .style(ButtonStyle::Ghost)
                        .icon(IconName::Code)
                        .selected(shown.get())
                        .on_click(move || toggle.update(|on| *on = !*on)),
                ))
                .spacing(8.0)
            }),
            // The diagram as it is now, as the app would save it; `FlowGraph::from_json` reads it back.
            view_fn(move || {
                json.get().then(|| ScrollView::new((CodeBlock::new(graph.get().to_json()),)).height(200.0))
            }),
            canvas,
        ))
        .spacing(12.0)
        .align(Align::Stretch)
    })
}

impl ComponentBook for FlowCanvas {
    fn book() -> Meta {
        Meta::new("Data/FlowCanvas", render)
            .summary("Nodes joined by connections on an endless surface: pan, zoom, ports that check what connects, a map and keys for everything.")
            .arg(ArgType::bool("cables", false))
            .arg(ArgType::bool("controls", true))
            .arg(ArgType::bool("editable_titles", true))
            .arg(ArgType::number("grid", 24.0, 0.0, 96.0, 1.0))
            .arg(ArgType::number("grow", 1.0, 0.0, 4.0, 0.1))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("locked", false))
            .arg(ArgType::bool("minimap", true))
            .arg(ArgType::bool("ports_on_hover", false))
            .arg(ArgType::bool("quick_create", true))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::bool("resizable", true))
            .arg(ArgType::bool("rotatable", true))
            .arg(ArgType::bool("snap", true))
            .arg(ArgType::bool("json", false).description("How the example starts."))
            .story("Example", Args::new())
            .story("Read only", Args::new().with("read_only", true))
            .story("Json", Args::new().with("json", true))
    }
}
