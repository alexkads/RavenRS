//! Cables, every feature of `CablePlug`: eight uses made only of it and the components that
//! already exist (flow diagrams, processes, the back of a rack, a switchboard, a modular
//! synthesizer, a network rack, matching columns and a tip tied to a button), and a lab with the
//! knobs: dashes, weight, held points, plug colors, what happens when it goes, more points, and a
//! cable carried in the hand. The physics runs on the GPU where there is one.

use std::rc::Rc;

use ravenrs::core::{Border, Brush, Role};
use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;
use ravenrs::widgets::cable::{CableEnd, CablePhysics, CableRelease, CableTexture};

/// The control's name, as the session knows it.
pub const ID: &str = "CablePlugDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Cabos";

/// The uses, in the order of the tabs.
pub const USES: [&str; 9] =
    ["Fluxo", "Processos", "Rack", "Central", "Sintetizador", "Rede", "Colunas", "Dica", "Laboratório"];

/// One jack of a panel.
#[derive(Clone, Debug)]
struct Jack {
    name: &'static str,
    /// Cables leave it (an output); otherwise they arrive.
    out: bool,
    /// What it carries: a cable goes between jacks of the same signal; 0 takes any.
    signal: u8,
    x: f32,
    y: f32,
}

const fn jack(name: &'static str, out: bool, signal: u8, x: f32, y: f32) -> Jack {
    Jack { name, out, signal, x, y }
}

/// How a panel's cables look and behave.
#[derive(Clone)]
struct Style {
    background: Color,
    face: Color,
    ink: Color,
    color: fn(u8) -> Color,
    texture: fn(u8) -> CableTexture,
    thickness: f32,
    retract: bool,
    /// The kind carried while a cable is in the hand, so panels do not mix.
    kind: &'static str,
}

/// One jack: an output is where a cable is pulled from, an input takes one of its signal.
struct Socket {
    index: usize,
    jacks: Rc<Vec<Jack>>,
    links: State<Vec<(usize, usize)>>,
    style: Style,
}

impl View for Socket {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (i, s) = (self.index, &self.style);
        let j = &self.jacks[i];
        let paint = Paint::Quad(QuadPaint {
            fill: Brush::Solid(Color::hex(0x0B0D12)),
            corner_radius: 11.0,
            border: Some(Border { width: 2.5, color: if j.out { s.ink } else { (s.color)(j.signal) } }),
            shadow: None,
        });
        let style = LayoutStyle {
            absolute: Some((j.x, j.y)),
            width: Some(22.0),
            height: Some(22.0),
            ..LayoutStyle::leaf()
        };
        let node = cx.container(style, &[], paint);
        let mut semantics = Semantics::new(Role::Button);
        semantics.name = Some(j.name.to_string());
        cx.semantics(node, semantics);
        if j.out {
            cx.drag_source(node, DragPayload::new(s.kind, j.name, i));
            return node;
        }
        let (jacks, links, kind) = (self.jacks.clone(), self.links.clone(), s.kind);
        let (check_jacks, check_links) = (jacks.clone(), links.clone());
        cx.drop_target(
            node,
            move |payload| {
                let from = payload.data::<usize>().copied().filter(|_| payload.kind() == kind);
                let Some(from) = from else { return Err("Outro painel".into()) };
                let (a, b) = (&check_jacks[from], &check_jacks[i]);
                if a.signal != 0 && b.signal != 0 && a.signal != b.signal {
                    return Err("Sinal diferente".into());
                }
                if check_links.get().iter().any(|(_, to)| *to == i) {
                    return Err("Tomada ocupada".into());
                }
                Ok(())
            },
            move |payload, _| {
                if let Some(&from) = payload.data::<usize>() {
                    links.update(|l| l.push((from, i)));
                }
            },
        );
        node
    }
}

/// A panel of jacks: pull a cable out of an output and let it go on an input of the same
/// signal; a click on a cable unplugs it.
struct Patch {
    title: &'static str,
    jacks: Rc<Vec<Jack>>,
    links: State<Vec<(usize, usize)>>,
    /// A word about the cable clicked last, when the panel says it.
    said: Option<State<String>>,
    size: Size,
    style: Style,
}

impl View for Patch {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let s = &self.style;
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        for (i, j) in self.jacks.iter().enumerate() {
            let socket =
                Socket { index: i, jacks: self.jacks.clone(), links: self.links.clone(), style: s.clone() };
            let node = cx.keyed_child(i as u64, &socket);
            let label = cx.child(&Text::new(j.name).style(TextStyle::Caption).foreground(s.ink));
            let at = LayoutStyle {
                absolute: Some((j.x - 18.0, j.y + 26.0)),
                width: Some(58.0),
                ..LayoutStyle::leaf()
            };
            children.push(cx.container(at, &[label], Paint::None));
            children.push(node);
            nodes.push(node);
        }
        // The cables plugged in: a click unplugs one.
        for &(a, b) in &self.links.get() {
            let signal = self.jacks[a].signal.max(self.jacks[b].signal);
            let links = self.links.clone();
            let said = self.said.clone();
            let words = format!("{} → {}", self.jacks[a].name, self.jacks[b].name);
            CablePlug::between(nodes[a], nodes[b])
                .key((a * 1000 + b) as u64)
                .color((s.color)(signal))
                .texture((s.texture)(signal))
                .thickness(s.thickness)
                .plugs(true)
                .retract(s.retract)
                .on_click(move || {
                    if let Some(said) = &said {
                        said.set(words.clone());
                    } else {
                        links.update(|l| l.retain(|&(x, y)| (x, y) != (a, b)));
                    }
                })
                .build(cx);
        }
        // The cable in the hand, out of the jack it was pulled from.
        if let Some(carrying) = cx.carrying()
            && carrying.payload.kind() == s.kind
            && let Some(&from) = carrying.payload.data::<usize>()
        {
            CablePlug::from(nodes[from])
                .key(0xC0FFEE)
                .color((s.color)(self.jacks[from].signal))
                .texture((s.texture)(self.jacks[from].signal))
                .thickness(s.thickness)
                .plugs(true)
                .retract(s.retract)
                .build(cx);
        }
        let panel = LayoutStyle {
            width: Some(self.size.width),
            height: Some(self.size.height),
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let panel = cx.container(
            panel,
            &children,
            Paint::Quad(QuadPaint {
                fill: Brush::Solid(s.face),
                corner_radius: 12.0,
                border: None,
                shadow: None,
            }),
        );
        let title = cx.child(&Text::new(self.title).style(TextStyle::Headline));
        let mut column = vec![title, panel];
        if let Some(said) = &self.said {
            column.push(cx.child(&Text::new(said.get()).foreground(s.ink)));
        }
        cx.container(
            LayoutStyle {
                spacing: 12.0,
                padding: Insets::uniform(16.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &column,
            Paint::Quad(QuadPaint::new(s.background)),
        )
    }
}

fn signal_color(signal: u8) -> Color {
    match signal {
        1 => Color::hex(0xD64545),
        2 => Color::hex(0x2F9E6A),
        3 => Color::hex(0xE08A2E),
        4 => Color::hex(0x4C66EE),
        _ => Color::hex(0xB07FFF),
    }
}

fn smooth(_: u8) -> CableTexture {
    CableTexture::Smooth
}

fn cords(_: u8) -> Color {
    Color::hex(0xE8DCC2)
}

fn braided(_: u8) -> CableTexture {
    CableTexture::Braided
}

/// The synthesizer's cables: coiled, striped, and one with a picture along it.
fn synth_texture(signal: u8) -> CableTexture {
    match signal {
        1 => CableTexture::Coiled,
        2 => CableTexture::Striped(Color::hex(0xF5F7FA)),
        3 => stripes().map_or(CableTexture::Smooth, |image| CableTexture::Image { image, repeat: 18.0 }),
        _ => CableTexture::Smooth,
    }
}

/// Diagonal stripes, a picture to repeat along a cable.
fn stripes() -> Option<ImageData> {
    ImageData::from_fn(
        16,
        16,
        |x, y| if (x + y) % 8 < 4 { Color::hex(0xF2C14E) } else { Color::hex(0x2A2F3A) },
    )
}

fn network_texture(_: u8) -> CableTexture {
    CableTexture::Striped(Color::hex(0xFFFFFF).with_alpha(0.6))
}

fn rack() -> Vec<Jack> {
    vec![
        jack("Guitarra", true, 1, 40.0, 50.0),
        jack("Microfone", true, 1, 120.0, 50.0),
        jack("Sintetizador", true, 2, 200.0, 50.0),
        jack("Clock", true, 3, 280.0, 50.0),
        jack("Entrada 1", false, 1, 60.0, 230.0),
        jack("Entrada 2", false, 1, 160.0, 230.0),
        jack("Linha", false, 2, 260.0, 230.0),
        jack("Sync", false, 3, 360.0, 230.0),
    ]
}

fn switchboard() -> Vec<Jack> {
    let mut jacks = Vec::new();
    for (i, name) in
        ["Ramal 21", "Ramal 22", "Ramal 23", "Ramal 24", "Ramal 25", "Ramal 26"].into_iter().enumerate()
    {
        jacks.push(jack(name, false, 0, 40.0 + i as f32 * 70.0, 40.0));
    }
    for (i, name) in ["Cordão A", "Cordão B", "Cordão C"].into_iter().enumerate() {
        jacks.push(jack(name, true, 0, 80.0 + i as f32 * 120.0, 260.0));
    }
    jacks
}

fn synthesizer() -> Vec<Jack> {
    vec![
        jack("VCO", true, 1, 40.0, 40.0),
        jack("LFO", true, 2, 120.0, 40.0),
        jack("Envelope", true, 3, 200.0, 40.0),
        jack("Ruído", true, 4, 280.0, 40.0),
        jack("VCF", false, 1, 60.0, 220.0),
        jack("Pitch", false, 2, 150.0, 220.0),
        jack("VCA", false, 3, 240.0, 220.0),
        jack("Mixer", false, 4, 330.0, 220.0),
        jack("Mod", false, 2, 420.0, 140.0),
    ]
}

fn network() -> Vec<Jack> {
    let mut jacks = Vec::new();
    for (i, name) in ["Switch 1", "Switch 2", "Switch 3", "Switch 4"].into_iter().enumerate() {
        jacks.push(jack(name, true, 4, 40.0 + i as f32 * 80.0, 40.0));
    }
    for (i, name) in ["Servidor A", "Servidor B", "Storage", "Firewall"].into_iter().enumerate() {
        jacks.push(jack(name, false, 4, 40.0 + i as f32 * 100.0, 230.0));
    }
    jacks
}

fn columns() -> Vec<Jack> {
    vec![
        jack("Rio", true, 0, 40.0, 40.0),
        jack("Lisboa", true, 0, 40.0, 130.0),
        jack("Tóquio", true, 0, 40.0, 220.0),
        jack("Japão", false, 0, 340.0, 40.0),
        jack("Brasil", false, 0, 340.0, 130.0),
        jack("Portugal", false, 0, 340.0, 220.0),
    ]
}

/// The right answers of the matching columns: Rio-Brasil, Lisboa-Portugal, Tóquio-Japão.
const ANSWERS: [(usize, usize); 3] = [(0, 4), (1, 5), (2, 3)];

fn flow_graph() -> FlowGraph {
    FlowGraph::new()
        .node(FlowNode::new("a", "step", "Pedido", 40.0, 60.0).port(FlowPort::output("out")))
        .node(
            FlowNode::new("b", "step", "Estoque", 340.0, 40.0)
                .port(FlowPort::input("in"))
                .port(FlowPort::output("out")),
        )
        .node(FlowNode::new("c", "step", "Entrega", 640.0, 120.0).port(FlowPort::input("in")))
        .edge(FlowEdge::new("e1", FlowEndpoint::new("a", "out"), FlowEndpoint::new("b", "in")))
        .edge(FlowEdge::new("e2", FlowEndpoint::new("b", "out"), FlowEndpoint::new("c", "in")))
}

fn process_graph() -> FlowGraph {
    FlowGraph::new()
        .node(FlowNode::new("start", "event", "Início", 40.0, 120.0).port(FlowPort::output("out")))
        .node(
            FlowNode::new("check", "gate", "Aprovado?", 300.0, 120.0)
                .port(FlowPort::input("in"))
                .port(FlowPort::output("yes"))
                .port(FlowPort::output("no")),
        )
        .node(FlowNode::new("ok", "event", "Faturar", 600.0, 30.0).port(FlowPort::input("in")))
        .node(FlowNode::new("ko", "event", "Recusar", 600.0, 230.0).port(FlowPort::input("in")))
        .edge(FlowEdge::new("e1", FlowEndpoint::new("start", "out"), FlowEndpoint::new("check", "in")))
        .edge(
            FlowEdge::new("e2", FlowEndpoint::new("check", "yes"), FlowEndpoint::new("ok", "in"))
                .color("#2F9E6A")
                .label("Sim"),
        )
        .edge(
            FlowEdge::new("e3", FlowEndpoint::new("check", "no"), FlowEndpoint::new("ko", "in"))
                .color("#D64545")
                .dashed()
                .label("Não"),
        )
}

/// A tip that hangs from the button it is about, by a thin cable.
struct Tip {
    shown: State<bool>,
    slack: State<f32>,
}

impl View for Tip {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let shown = self.shown.clone();
        let button = cx.child(&Button::new("Exportar").icon(IconName::Download));
        let toggle = cx.child(&Toggle::new(shown.binding()).label("Mostrar a dica"));
        let (more, less) = (self.slack.clone(), self.slack.clone());
        let looser = cx.child(&Button::new("Mais solto").style(ButtonStyle::Secondary).on_click(move || {
            more.update(|s| *s = (*s + 0.1).min(1.8));
        }));
        let tighter =
            cx.child(&Button::new("Mais esticado").style(ButtonStyle::Secondary).on_click(move || {
                less.update(|s| *s = (*s - 0.1).max(1.0));
            }));
        let mut children = vec![toggle, looser, tighter];
        let at = LayoutStyle { absolute: Some((80.0, 120.0)), ..LayoutStyle::leaf() };
        children.push(cx.container(at, &[button], Paint::None));
        if shown.get() {
            let card = cx.child(
                &Card::new(VStack::new((
                    Text::new("Dica").style(TextStyle::Headline),
                    Text::new("Exporte em PDF por aqui."),
                )))
                .width(220.0),
            );
            let at = LayoutStyle { absolute: Some((420.0, 260.0)), ..LayoutStyle::leaf() };
            children.push(cx.container(at, &[card], Paint::None));
            CablePlug::between(button, card)
                .sides(CableSide::Bottom, CableSide::Top)
                .thickness(2.0)
                .shine(0.0)
                .slack(self.slack.get())
                .color(cx.theme.color(Semantic::Accent))
                .key(1)
                .build(cx);
        }
        cx.container(
            LayoutStyle {
                width: Some(760.0),
                height: Some(420.0),
                padding: Insets::uniform(16.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

/// The knobs of a cable, each a switch, and a cable carried in the hand.
struct Lab {
    dashed: State<bool>,
    heavy: State<bool>,
    held: State<bool>,
    colored: State<bool>,
    vanish: State<bool>,
    finer: State<bool>,
    shown: State<bool>,
    hand: State<bool>,
    dropped: State<Option<Point>>,
    boxed: State<bool>,
}

impl View for Lab {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let at = |cx: &mut BuildCx<'_>, x: f32, y: f32, w: f32, h: f32, color: Color| {
            let style = LayoutStyle {
                absolute: Some((x, y)),
                width: Some(w),
                height: Some(h),
                ..LayoutStyle::leaf()
            };
            cx.container(
                style,
                &[],
                Paint::Quad(QuadPaint {
                    fill: Brush::Solid(color),
                    corner_radius: 8.0,
                    border: None,
                    shadow: None,
                }),
            )
        };
        let switches = cx.child(&Grid::new(
            4,
            (
                Toggle::new(self.shown.binding()).label("Cabo de ajustes"),
                Toggle::new(self.dashed.binding()).label("Tracejado"),
                Toggle::new(self.heavy.binding()).label("Mais pesado"),
                Toggle::new(self.held.binding()).label("Preso no caminho"),
                Toggle::new(self.colored.binding()).label("Plugues coloridos"),
                Toggle::new(self.vanish.binding()).label("Some ao sair"),
                Toggle::new(self.finer.binding()).label("Mais pontos"),
                Toggle::new(self.boxed.binding()).label("Ponta na caixa"),
            ),
        ));
        let (hand, dropped) = (self.hand.clone(), self.dropped.clone());
        let take = cx.child(&Button::new("Pegar o cabo").style(ButtonStyle::Secondary).on_click(move || {
            hand.set(true);
            dropped.set(None);
        }));
        let start = at(cx, 40.0, 140.0, 24.0, 24.0, Color::hex(0x4C66EE));
        let target = at(cx, 520.0, 160.0, 90.0, 60.0, Color::hex(0x2A2F3A));
        let source = at(cx, 40.0, 300.0, 24.0, 24.0, Color::hex(0xE08A2E));
        if self.shown.get() {
            let mut plug = CablePlug::anchored(
                CableAnchor::Node(start, CableSide::Right),
                CableAnchor::Point(CableEnd::at(Point::new(470.0, 190.0)).toward(-1.0, 0.0)),
            )
            .key(1)
            .plugs(true)
            .grow(false)
            .release(if self.vanish.get() { CableRelease::Vanish } else { CableRelease::Fall });
            if self.boxed.get() {
                plug = plug.to(target).sides(CableSide::Right, CableSide::Left);
            }
            if self.dashed.get() {
                plug = plug.dash(10.0, 6.0);
            }
            if self.heavy.get() {
                plug = plug.gravity(6000.0);
            }
            if self.held.get() {
                plug = plug.via([Point::new(260.0, 120.0)]);
            }
            if self.colored.get() {
                plug = plug.plug_color(Color::hex(0xD64545));
            }
            if self.finer.get() {
                plug = plug.physics(CablePhysics { points: 28, ..CablePhysics::default() });
            }
            plug.build(cx);
        }
        // A cable in the hand: it follows the pointer until it is let go, and stays there.
        let (hand, dropped) = (self.hand.clone(), self.dropped.clone());
        if self.hand.get() {
            CablePlug::from(source)
                .key(2)
                .color(Color::hex(0xE08A2E))
                .plugs(true)
                .loose()
                .on_drop(move |p| {
                    hand.set(false);
                    dropped.set(Some(p));
                })
                .build(cx);
        } else if let Some(p) = dropped.get() {
            CablePlug::from(source).key(2).color(Color::hex(0xE08A2E)).plugs(true).to_point(p).build(cx);
        }
        let (row, place) = (
            cx.container(
                LayoutStyle { spacing: 12.0, ..LayoutStyle::stack(Axis::Horizontal) },
                &[take],
                Paint::None,
            ),
            cx.container(
                LayoutStyle { width: Some(760.0), height: Some(360.0), ..LayoutStyle::stack(Axis::Vertical) },
                &[start, target, source],
                Paint::None,
            ),
        );
        cx.container(
            LayoutStyle {
                spacing: 12.0,
                padding: Insets::uniform(16.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[switches, row, place],
            Paint::None,
        )
    }
}

/// The page: a tab per use.
#[derive(Debug, Default)]
pub struct CablesPage;

impl View for CablesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let tab = cx.use_state(|| 0_usize);
        let flow = cx.use_state(flow_graph);
        let process = cx.use_state(process_graph);
        let rack_links = cx.use_state(|| vec![(0, 4), (2, 6)]);
        let board_links = cx.use_state(|| vec![(6, 1)]);
        let synth_links = cx.use_state(|| vec![(0, 4), (1, 5), (1, 8), (2, 6)]);
        let net_links = cx.use_state(|| vec![(0, 4), (1, 5), (2, 7)]);
        let net_said = cx.use_state(|| "Clique num cabo para ver a ligação.".to_string());
        let column_links = cx.use_state(Vec::<(usize, usize)>::new);
        let shown = cx.use_state(|| true);
        let slack = cx.use_state(|| 1.15_f32);
        let lab = Lab {
            dashed: cx.use_state(|| false),
            heavy: cx.use_state(|| false),
            held: cx.use_state(|| false),
            colored: cx.use_state(|| false),
            vanish: cx.use_state(|| false),
            finer: cx.use_state(|| false),
            shown: cx.use_state(|| true),
            hand: cx.use_state(|| false),
            dropped: cx.use_state(|| None),
            boxed: cx.use_state(|| false),
        };
        let dark = |kind| Style {
            background: Color::hex(0x12151C),
            face: Color::hex(0x2A2F3A),
            ink: Color::hex(0xC7CEDB),
            color: signal_color,
            texture: smooth,
            thickness: 6.0,
            retract: false,
            kind,
        };
        let body: AnyView = match tab.get() {
            0 => Box::new(
                FlowCanvas::new(flow.binding()).cables(true).label("Fluxo com cabos").size(900.0, 420.0),
            ),
            1 => Box::new(
                FlowCanvas::new(process.binding())
                    .cables(true)
                    .label("Processo com cabos")
                    .size(900.0, 420.0),
            ),
            2 => Box::new(Patch {
                title: "Traseira do amplificador",
                jacks: Rc::new(rack()),
                links: rack_links,
                said: None,
                size: Size::new(460.0, 300.0),
                style: dark("rack"),
            }),
            3 => Box::new(Patch {
                title: "Central telefônica: o cordão volta ao seu furo",
                jacks: Rc::new(switchboard()),
                links: board_links,
                said: None,
                size: Size::new(460.0, 320.0),
                style: Style {
                    background: Color::hex(0x2A1A12),
                    face: Color::hex(0x4A2A1A),
                    ink: Color::hex(0xE8DCC2),
                    color: cords,
                    texture: braided,
                    thickness: 5.0,
                    retract: true,
                    kind: "central",
                },
            }),
            4 => Box::new(Patch {
                title: "Sintetizador modular",
                jacks: Rc::new(synthesizer()),
                links: synth_links,
                said: None,
                size: Size::new(480.0, 300.0),
                style: Style { texture: synth_texture, ..dark("synth") },
            }),
            5 => Box::new(Patch {
                title: "Rack de rede",
                jacks: Rc::new(network()),
                links: net_links,
                said: Some(net_said),
                size: Size::new(460.0, 300.0),
                style: Style { texture: network_texture, thickness: 4.0, ..dark("network") },
            }),
            6 => {
                let links = column_links.clone();
                Box::new(VStack::new((
                    Patch {
                        title: "Ligue cada cidade ao seu país",
                        jacks: Rc::new(columns()),
                        links: column_links,
                        said: None,
                        size: Size::new(420.0, 290.0),
                        style: Style { color: |_| Color::hex(0x4C66EE), ..dark("columns") },
                    },
                    // The wrong ones come loose and fall.
                    Button::new("Conferir")
                        .on_click(move || links.update(|l| l.retain(|p| ANSWERS.contains(p)))),
                )))
            }
            7 => Box::new(Tip { shown, slack }),
            _ => Box::new(lab),
        };
        cx.child(
            &VStack::new((SegmentedControl::new(tab.binding(), USES), Keyed::new(tab.get() as u64, body)))
                .spacing(12.0)
                .padding(16.0)
                .align(Align::Start),
        )
    }
}
