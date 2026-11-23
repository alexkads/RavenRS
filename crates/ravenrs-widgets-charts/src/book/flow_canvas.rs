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
        let arg_status_icons = args.bool("status_icons");
        let arg_run = args.bool("run");

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
            // A width of 0 takes the parent's; the height is always the story's.
            let (wide, tall) = (args.number("width") as f32, args.number("height") as f32);
            it = if wide > 0.0 { it.size(wide, tall) } else { it.height(tall) };
            it = it.cables(arg_cables);
            it = it.controls(arg_controls);
            it = it.editable_titles(arg_editable_titles);
            it = it.grid(arg_grid);
            if arg_grow > 0.0 && args.number("width") > 0.0 {
                it = it.grow(arg_grow);
            }
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
            it = it.status_icons(arg_status_icons);
            // A run told from outside, as a CI server tells the stages of a build.
            if arg_run {
                it = it.execution(
                    FlowRunView::new()
                        .node("sale", FlowNodeRun::new(FlowNodeStatus::Success))
                        .node("stock", FlowNodeRun::new(FlowNodeStatus::Running))
                        .node("enough", FlowNodeRun::new(FlowNodeStatus::Waiting("Waiting".into()))),
                );
            }
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
            .arg(ArgType::number("width", 880.0, 0.0, 2000.0, 1.0).description("0 takes the width of the parent."))
            .arg(ArgType::number("height", 368.0, 80.0, 1200.0, 1.0))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("locked", false))
            .arg(ArgType::bool("minimap", true))
            .arg(ArgType::bool("ports_on_hover", false))
            .arg(ArgType::bool("quick_create", true))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::bool("resizable", true))
            .arg(ArgType::bool("rotatable", true))
            .arg(ArgType::bool("snap", true))
            .arg(ArgType::bool("status_icons", false))
            .arg(ArgType::bool("run", false).description("A run told from outside, with execution."))
            .arg(ArgType::bool("json", false).description("How the example starts."))
            .story("Example", Args::new())
            .story("Read only", Args::new().with("read_only", true))
            .story(
                "Stage strip",
                Args::new()
                    .with("read_only", true)
                    .with("run", true)
                    .with("status_icons", true)
                    .with("minimap", false)
                    .with("width", 0.0)
                    .with("height", 198.0),
            )
            .story(
                "Run with status icons",
                Args::new().with("read_only", true).with("run", true).with("status_icons", true).with("minimap", false),
            )
            .story("Json", Args::new().with("json", true))
            .story("Cables", Args::new().with("cables", true))
            .story_with("Cards and a selection", Args::new(), cards_and_selection)
    }
}

/// Nodes that draw their own card (`content`), as an app that shows its own state in each node,
/// joined by cables, with one of them selected (#831).
fn cards_and_selection(_: &Args) -> AnyView {
    let graph = State::new(
        FlowGraph::new()
            .node(
                FlowNode::new("plan", "agent", "Planner", 0.0, 48.0)
                    .size(248.0, 128.0)
                    .status(FlowNodeStatus::Running)
                    .port(FlowPort::output("out").side(PortSide::Right)),
            )
            .node(
                FlowNode::new("build", "agent", "Builder", 312.0, 48.0)
                    .size(248.0, 128.0)
                    .status(FlowNodeStatus::Success)
                    .port(FlowPort::input("in").side(PortSide::Left))
                    .port(FlowPort::output("out").side(PortSide::Bottom)),
            )
            .node(
                FlowNode::new("review", "agent", "Reviewer", 312.0, 240.0)
                    .size(248.0, 128.0)
                    .port(FlowPort::input("in").side(PortSide::Top)),
            )
            .edge(
                FlowEdge::new("e1", FlowEndpoint::new("plan", "out"), FlowEndpoint::new("build", "in"))
                    .animated(true),
            )
            .edge(FlowEdge::new("e2", FlowEndpoint::new("build", "out"), FlowEndpoint::new("review", "in"))),
    );
    let selection = State::new(FlowSelection::node("review"));
    let card = |node: &FlowNode| -> Option<AnyView> {
        let (state, tone, icon) = match node.status {
            FlowNodeStatus::Running => ("running", Tone::Warning, IconName::LoaderCircle),
            FlowNodeStatus::Success => ("ready", Tone::Success, IconName::Check),
            _ => ("waiting", Tone::Neutral, IconName::Clock),
        };
        Some(Box::new(
            VStack::new((
                HStack::new((
                    Text::new(node.title.clone()).font_weight(Weight::SEMIBOLD).truncate().grow(1.0),
                    Badge::new(state).tone(tone).icon(icon),
                ))
                .align(Align::Center)
                .spacing(8.0),
                Text::new("Claude Code · read only")
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted)
                    .truncate(),
                Text::new("No output yet")
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted)
                    .truncate()
                    .padding(8.0)
                    .background(Semantic::Canvas)
                    .corner_radius(ravenrs_style::RADII.md)
                    .stretch(),
            ))
            .spacing(8.0)
            .align(Align::Stretch)
            .padding(12.0)
            .grow(1.0),
        ))
    };
    Box::new(
        FlowCanvas::new(graph.binding())
            .selection(selection.binding())
            .content(card)
            .cables(true)
            .minimap(false)
            .kind("agent", FlowKind::new(IconName::Sparkles, "Agent"))
            .size(880.0, 420.0),
    )
}
