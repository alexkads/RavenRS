//! A design board, made only with `FlowCanvas`, `FlowToolbox` and `FlowInspector`: frames
//! hold the shapes of a screen, shapes are placed freely, resized, turned and lined up with
//! the guides, and arrows between frames say where a tap leads, as in a clickable mock-up.
//!
//! It validates the component as a board of this kind: everything here is the library's,
//! and what it could not do is written in the README.

use std::rc::Rc;

use ravenrs::core::{MemoryStorage, Storage};
use ravenrs::prelude::*;

/// The kinds of piece, with the word the inspector shows for each.
pub const KINDS: [(&str, &str); 3] = [("shape", "Forma"), ("text", "Texto"), ("screen", "Tela")];

fn piece(id: &str, title: &str, shape: FlowShape, w: f32, h: f32) -> FlowNode {
    FlowNode::new(id, "shape", title, 0.0, 0.0).shape(shape).size(w, h)
}

fn at(node: FlowNode, x: f32, y: f32, frame: &str) -> FlowNode {
    FlowNode { x, y, ..node }.parent(frame)
}

/// Two screens of a sign-in, their pieces inside each frame, and where the button leads.
pub fn sample() -> FlowGraph {
    let login = Rect::new(0.0, 0.0, 360.0, 600.0);
    let home = Rect::new(480.0, 0.0, 360.0, 600.0);
    FlowGraph::new()
        .node(FlowNode::new_group("login", "Entrar", login))
        .node(at(piece("logo", "Logo", FlowShape::Ellipse, 96.0, 96.0), 132.0, 72.0, "login"))
        .node(at(piece("email", "E-mail", FlowShape::Rectangle, 264.0, 48.0), 48.0, 240.0, "login"))
        .node(at(piece("password", "Senha", FlowShape::Rectangle, 264.0, 48.0), 48.0, 312.0, "login"))
        .node(
            at(piece("enter", "Entrar", FlowShape::RoundedRectangle, 264.0, 48.0), 48.0, 408.0, "login")
                .port(FlowPort::output("tap").side(PortSide::Right)),
        )
        .node(
            FlowNode::new("heading", "text", "Fluxo de entrada", 0.0, -96.0)
                .shape(FlowShape::Text)
                .size(480.0, 48.0),
        )
        .node(at(
            FlowNode::new("photo", "image", "Foto do produto", 0.0, 0.0).size(312.0, 160.0),
            504.0,
            288.0,
            "home",
        ))
        // A note beside the screen, outside its frame.
        .node(
            FlowNode::new("contrast", "text", "Revisar o contraste do botão", -240.0, 408.0)
                .shape(FlowShape::Note)
                .size(192.0, 96.0),
        )
        .node(FlowNode::new_group("home", "Início", home).port(FlowPort::input("in").side(PortSide::Left)))
        .node(at(piece("header", "Cabeçalho", FlowShape::Rectangle, 312.0, 64.0), 504.0, 48.0, "home"))
        .node(at(piece("card", "Cartão do pedido", FlowShape::Card, 312.0, 120.0), 504.0, 144.0, "home"))
        .node(at(
            piece("badge", "Novo", FlowShape::Ellipse, 64.0, 32.0).rotation(-12.0),
            744.0,
            120.0,
            "home",
        ))
        .edge(
            FlowEdge::new("tap", FlowEndpoint::new("enter", "tap"), FlowEndpoint::new("home", "in"))
                .label("Toque")
                .shape(EdgeShape::Curve),
        )
}

/// The pieces to drag in, by group.
pub fn toolbox() -> FlowToolbox {
    FlowToolbox::new()
        .group("Formas")
        .model("Retângulo", piece("rect", "Retângulo", FlowShape::Rectangle, 160.0, 96.0))
        .model("Arredondado", piece("round", "Botão", FlowShape::RoundedRectangle, 160.0, 48.0))
        .model("Elipse", piece("ellipse", "Elipse", FlowShape::Ellipse, 120.0, 120.0))
        .model("Losango", piece("diamond", "Losango", FlowShape::Diamond, 140.0, 100.0))
        .model("Cilindro", piece("cylinder", "Cilindro", FlowShape::Cylinder, 120.0, 120.0))
        .model("Paralelogramo", piece("slant", "Paralelogramo", FlowShape::Parallelogram, 160.0, 80.0))
        .group("Texto")
        .model(
            "Título",
            FlowNode::new("title", "text", "Título", 0.0, 0.0).shape(FlowShape::Text).size(320.0, 48.0),
        )
        .model(
            "Nota",
            FlowNode::new("note", "text", "Nota", 0.0, 0.0).shape(FlowShape::Note).size(192.0, 96.0),
        )
        .model("Cartão", piece("card", "Cartão", FlowShape::Card, 240.0, 96.0))
        .model("Imagem", FlowNode::new("image", "image", "Imagem", 0.0, 0.0).size(240.0, 160.0))
        .group("Telas")
        .model("Celular", FlowNode::new_group("phone", "Tela", Rect::new(0.0, 0.0, 360.0, 640.0)))
        .model("Computador", FlowNode::new_group("desktop", "Tela", Rect::new(0.0, 0.0, 960.0, 600.0)))
}

/// A picture for the image pieces of the sample: a soft gradient, made here so that the example
/// needs no file.
pub fn sample_picture() -> Option<ImageData> {
    ImageData::from_fn(240, 140, |x, y| {
        let (u, v) = (x as f32 / 240.0, y as f32 / 140.0);
        Color::rgba(0.25 + 0.5 * u, 0.4 + 0.3 * v, 0.85 - 0.3 * u, 1.0)
    })
}

/// The board: pieces on the left, the board in the middle, the selected piece's size, place
/// and words on the right.
pub struct Board {
    /// Where the diagrams are kept between runs.
    storage: Rc<dyn Storage>,
}

impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Board").finish_non_exhaustive()
    }
}

impl View for Board {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let graph = cx.use_state(sample);
        let selection = cx.use_state(FlowSelection::default);
        let history = cx.use_state(FlowHistory::new);
        let arrivals = cx.use_state(|| None::<FlowNode>);
        let view = cx.use_state(FlowViewport::fit);
        let canvas = FlowCanvas::new(graph.binding())
            .view(view.binding())
            .ports_on_hover(true)
            .label("Quadro")
            .selection(selection.binding())
            .history(history.binding())
            .arrivals(arrivals.binding())
            // A board is drawn by hand: no arrows to make the next piece, a fine grid.
            .quick_create(false)
            .grid(8.0)
            .kind("shape", FlowKind::new(IconName::Shapes, "Forma"))
            .kind("text", FlowKind::new(IconName::Type, "Texto"))
            .kind("image", FlowKind::new(IconName::Image, "Imagem"))
            // An image piece shows its picture: here one drawn for the sample, in an app the file.
            .content(|node| {
                if node.kind == "image" {
                    sample_picture().map(|p| Box::new(Image::new(p)) as AnyView)
                } else {
                    None
                }
            })
            .on_export(ravenrs_flow_export::saver(cx.file_picker(), "Quadro"));
        let mut inspector =
            FlowInspector::new(graph.binding(), selection.binding()).history(history.binding());
        for (kind, caption) in KINDS {
            inspector = inspector.kind(kind, caption);
        }
        cx.child(
            &HStack::new((
                VStack::new((
                    FlowLibrary::new(graph.binding())
                        .title("Quadros")
                        .key("boards")
                        .history(history.binding())
                        .selection(selection.binding())
                        .view(view.binding())
                        .storage(self.storage.clone()),
                    toolbox().title("Peças").arrivals(arrivals.binding()),
                    FlowLayers::new(graph.binding(), selection.binding()).history(history.binding()),
                ))
                .spacing(0.0)
                .align(Align::Stretch),
                canvas,
                inspector,
            ))
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

/// The whole app.
pub fn app() -> Localized<Board> {
    app_kept_in(Rc::new(MemoryStorage::default()))
}

/// The whole app, keeping its diagrams in `storage`.
pub fn app_kept_in(storage: Rc<dyn Storage>) -> Localized<Board> {
    Localized::new(Labels::pt_br(), Board { storage })
}
