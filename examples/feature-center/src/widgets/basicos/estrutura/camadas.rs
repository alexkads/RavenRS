//! `Grid`, `ZStack`, `Spacer` and `Canvas`, every feature: a grid whose columns change with a
//! click, layers aligned each way, the room a spacer takes and keeps, and drawing by hand on a
//! canvas of fixed size or one that fills the row. A line says what changed.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "GridLayersDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Grade e camadas";

/// The column counts the grid offers.
pub const COLUMNS: [u16; 3] = [2, 3, 4];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn cell(n: usize) -> Card<Text> {
    Card::new(Text::new(format!("Item {n}")).align(TextAlign::Center)).padding(12.0).no_shadow()
}

/// A painted block for the layers, the size of what sits on it.
fn block(width: f32, height: f32) -> Canvas {
    Canvas::new(|frame, scene| {
        // ravenrs-lint: allow(raw_color) a drawing on a canvas, which has no theme
        scene.push_quad(Quad::new(frame, Color::hex(0x2b3550)).radius(12.0));
    })
    .size(width, height)
}

/// The page: the grid, the layers, the spacer and the canvas.
#[derive(Debug, Default)]
pub struct GridLayersPage;

impl View for GridLayersPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let columns = cx.use_state(|| 1_usize);
        let bars = cx.use_state(|| vec![0.4_f32, 0.7, 0.5]);
        let event = cx.use_state(|| String::from("Nada mudou ainda"));

        // The grid, with its column count picked by buttons.
        let count = if narrow { 2 } else { COLUMNS[columns.get()] };
        let pickers: Vec<AnyView> = COLUMNS
            .iter()
            .enumerate()
            .map(|(index, n)| {
                let (pick, said) = (columns.clone(), event.clone());
                let style =
                    if index == columns.get() { ButtonStyle::Primary } else { ButtonStyle::Secondary };
                Box::new(Button::new(format!("{n} colunas")).style(style).size(ButtonSize::Small).on_click(
                    move || {
                        pick.set(index);
                        said.set(format!("Grade com {n} colunas"));
                    },
                )) as AnyView
            })
            .collect();
        let cells: Vec<AnyView> = (1..=8).map(|n| Box::new(cell(n)) as AnyView).collect();
        let grid = VStack::new((
            HStack::new(pickers).spacing(8.0),
            Grid::new(count, cells),
            Text::new("Espaço de 4 entre as células:").foreground(Semantic::InkMuted),
            Grid::new(4, (cell(1), cell(2), cell(3), cell(4))).spacing(4.0),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // Layers: the first at the back, the smaller ones placed by the alignment.
        let side = if narrow { 96.0 } else { 140.0 };
        let layered = |align: Align, label: &str| {
            VStack::new((
                ZStack::new((block(side, side * 0.7), Badge::new(label.to_owned()).tone(Tone::Accent)))
                    .align(align),
                Text::new(label.to_owned()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0)
            .align(Align::Center)
        };
        let layers = HStack::new((
            layered(Align::Start, "Início"),
            layered(Align::Center, "Centro"),
            layered(Align::End, "Fim"),
            VStack::new((
                ZStack::new((block(side, side * 0.7), Text::new("Esticado").padding(8.0))),
                Text::new("Esticado").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0),
        ))
        .spacing(12.0);

        // The spacer pushes the buttons to the end and keeps at least 48 of room.
        let toolbar = VStack::new((
            HStack::new((
                Text::new("Relatório"),
                Spacer::new(),
                Button::new("Exportar").size(ButtonSize::Small),
            ))
            .align(Align::Center),
            HStack::new((
                Text::new("Mínimo de 48"),
                Spacer::new().min(48.0),
                Button::new("Imprimir").size(ButtonSize::Small),
            ))
            .align(Align::Center)
            .width(if narrow { 220.0 } else { 280.0 }),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // A canvas draws what the state holds; a second one fills the rest of its row.
        let heights = bars.get();
        let total = heights.len();
        let chart = Canvas::new(move |frame, scene| {
            let step = frame.width() / heights.len().max(1) as f32;
            for (i, h) in heights.iter().enumerate() {
                let tall = frame.height() * h;
                let bar = Rect::new(
                    frame.x() + i as f32 * step + 4.0,
                    frame.y() + frame.height() - tall,
                    step - 8.0,
                    tall,
                );
                // ravenrs-lint: allow(raw_color) a drawing on a canvas, which has no theme
                scene.push_quad(Quad::new(bar, Color::hex(0x4c66ee)).radius(4.0));
            }
        })
        .size(if narrow { 160.0 } else { 240.0 }, 120.0);
        let (add, said_add) = (bars.clone(), event.clone());
        let (clear, said_clear) = (bars.clone(), event.clone());
        let canvas = VStack::new((
            chart,
            HStack::new((
                Button::new("Mais uma barra").on_click(move || {
                    add.update(|bars| {
                        let next = 0.25 + 0.15 * (bars.len() % 5) as f32;
                        bars.push(next);
                    });
                    said_add.set(String::from("Barra desenhada"));
                }),
                Button::new("Limpar").style(ButtonStyle::Secondary).on_click(move || {
                    clear.set(Vec::new());
                    said_clear.set(String::from("Desenho limpo"));
                }),
            ))
            .spacing(8.0),
            Text::new(format!("Barras no desenho: {total}")),
            HStack::new((
                Text::new("Faixa que cresce:"),
                Canvas::new(|frame, scene| {
                    // ravenrs-lint: allow(raw_color) a drawing on a canvas, which has no theme
                    scene.push_quad(Quad::new(frame, Color::hex(0x2fb27a)).radius(6.0));
                })
                .size(0.0, 12.0)
                .grow(1.0),
            ))
            .spacing(8.0)
            .align(Align::Center),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        cx.child(
            &VStack::new((
                VStack::new((heading("Grade"), grid)).spacing(8.0),
                VStack::new((heading("Camadas"), layers)).spacing(8.0),
                VStack::new((heading("Espaçador"), toolbar)).spacing(8.0),
                VStack::new((heading("Desenho livre"), canvas)).spacing(8.0),
                Text::bound(&event, |said| format!("Evento: {said}")),
            ))
            .spacing(20.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
