//! `ScrollView`, `ScrollLinked`, `ScrollScene`, `ScrollEdgeArrow`, `Sticky` and `Carousel`, every
//! feature: lists that scroll up and down or sideways, effects that follow the scroll, a scene that
//! holds while its steps change, strips with arrows, titles that stay put and slides that turn by
//! themselves. A line says what was chosen.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ScrollingDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Rolagem";

/// The categories of the strip with arrows.
pub const CATEGORIES: [&str; 10] = [
    "Bebidas",
    "Padaria",
    "Frios",
    "Hortifrúti",
    "Limpeza",
    "Higiene",
    "Congelados",
    "Mercearia",
    "Pet",
    "Papelaria",
];

/// The slides of the carousel, by title.
pub const SLIDES: [&str; 3] = ["Frete grátis", "Cupom de boas-vindas", "Clube de vantagens"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn rows(prefix: &str, count: usize) -> Vec<AnyView> {
    (1..=count).map(|n| Box::new(Text::new(format!("{prefix} {n}"))) as AnyView).collect()
}

/// Room below the content, so that a scene has something to scroll past.
fn room(height: f32) -> Canvas {
    Canvas::new(|_, _| {}).size(1.0, height)
}

/// The page: the scroll views, the scene, the strips, the pinned titles and the carousel.
#[derive(Debug, Default)]
pub struct ScrollingPage;

impl View for ScrollingPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let event = cx.use_state(|| String::from("Nada escolhido ainda"));

        // Up and down, sideways, and with the bar outside a padded surface.
        let lists = Grid::new(
            if narrow { 1 } else { 3 },
            (
                ScrollView::new(rows("Linha", 30)).spacing(8.0).height(160.0),
                ScrollView::new((
                    HStack::new(rows("Coluna", 16)).spacing(24.0),
                    Text::new("Este conteúdo é mais largo que a vista."),
                ))
                .horizontal()
                .spacing(8.0)
                .height(160.0),
                Card::new(
                    ScrollView::new(rows("Item", 20))
                        .scrollbar_outside(12.0)
                        .padding(4.0)
                        .spacing(4.0)
                        .align(Align::Center)
                        .height(136.0),
                )
                .padding(12.0)
                .no_shadow(),
            ),
        )
        .spacing(12.0);

        // Cards that fade and rise as they come into view, following the scroll and not a clock.
        let linked = ScrollView::new((
            ScrollLinked::new(
                Card::new(Text::new("Cada cartão aparece")),
                ScrollLink::fade_in().range(ScrollRange::Enter),
            ),
            ScrollLinked::new(
                Card::new(Text::new("conforme entra na vista,")),
                ScrollLink::fade_in().range(ScrollRange::Enter),
            ),
            ScrollLinked::new(Card::new(Text::new("seguindo a rolagem")), ScrollLink::parallax(24.0)),
            ScrollLinked::new(Card::new(Text::new("e não um relógio.")), ScrollLink::grow_from(0.8)),
        ))
        .spacing(64.0)
        .height(180.0);

        // A scene that holds its steps on screen while the reader scrolls 600 pixels.
        let step = |title: &'static str, from: f32, to: f32| {
            let away = Effect::hidden().then(Effect::moved(0.0, 16.0));
            ScrollLinked::new(
                Card::new(Text::new(title).style(TextStyle::Title)),
                ScrollLink::keyframes([
                    (from - 0.08, away),
                    (from + 0.02, Effect::IDENTITY),
                    (to - 0.02, Effect::IDENTITY),
                    (to + 0.08, away),
                ]),
            )
        };
        let scene = ScrollView::new((
            Text::new("Role aqui dentro: a cena para enquanto os passos mudam.")
                .foreground(Semantic::InkMuted),
            ScrollScene::new(ZStack::new((
                step("1. Estado", -1.0, 0.33),
                step("2. Layout", 0.33, 0.66),
                step("3. Pintura", 0.66, 2.0),
            )))
            .length(600.0)
            .top(8.0),
            Text::new("Aqui a cena se solta.").foreground(Semantic::InkMuted),
            room(200.0),
        ))
        .spacing(12.0)
        .height(220.0);

        // Strips with arrows at the ends that are cut, sideways and up and down.
        let chips: Vec<AnyView> = CATEGORIES
            .iter()
            .map(|name| {
                let said = event.clone();
                Box::new(
                    Button::new(*name)
                        .style(ButtonStyle::Secondary)
                        .size(ButtonSize::Small)
                        .on_click(move || said.set(format!("Categoria: {name}"))),
                ) as AnyView
            })
            .collect();
        let strip = ScrollEdgeArrow::new(chips).spacing(8.0).padding(4.0).fade(Semantic::Canvas);
        let column = ScrollEdgeArrow::new(rows("Mensagem", 12)).vertical().spacing(6.0).height(140.0);

        // Titles that stay on top of their section, and a first column that stays at the left.
        let section = |name: &'static str| {
            VStack::new((
                Sticky::new(Badge::new(name).tone(Tone::Accent)).top(4.0),
                VStack::new(rows(name, 6)).spacing(8.0),
            ))
            .spacing(8.0)
        };
        let pinned = ScrollView::new((section("Entradas"), section("Saídas"), section("Ajustes")))
            .spacing(16.0)
            .height(180.0);
        let timeline = ScrollView::new((HStack::new((
            Sticky::new(Card::new(Text::new("Sala 1")).padding(8.0)).left(0.0),
            HStack::new(rows("09h", 14)).spacing(32.0),
        ))
        .spacing(16.0),))
        .horizontal()
        .height(72.0);

        // Slides with arrows and dots; the second turns by itself every 4 seconds.
        let slide = |title: &str, text: &str| -> AnyView {
            Box::new(
                VStack::new((
                    Text::new(title.to_owned()).style(TextStyle::Title),
                    Text::new(text.to_owned()).foreground(Semantic::InkMuted),
                ))
                .spacing(6.0)
                .padding(24.0)
                .align(Align::Center),
            )
        };
        let carousel = Carousel::new(vec![
            slide(SLIDES[0], "Nas compras acima de R$ 200."),
            slide(SLIDES[1], "10% na primeira compra."),
            slide(SLIDES[2], "Pontos em cada pedido."),
        ])
        .height(if narrow { 140.0 } else { 180.0 });
        let rotating = Carousel::new(vec![
            slide("Novidade", "Passa sozinho a cada 4 segundos."),
            slide("Outra novidade", "E volta ao começo no fim."),
        ])
        .autoplay(4.0)
        .height(120.0);

        cx.child(
            &VStack::new((
                VStack::new((heading("Vistas que rolam"), lists)).spacing(8.0),
                VStack::new((heading("Efeitos ligados à rolagem"), linked)).spacing(8.0),
                VStack::new((heading("Cena que para na tela"), scene)).spacing(8.0),
                VStack::new((heading("Faixas com setas"), strip, column)).spacing(8.0),
                VStack::new((heading("Títulos que ficam"), pinned, timeline)).spacing(8.0),
                VStack::new((heading("Carrossel"), carousel, rotating)).spacing(8.0),
                Text::bound(&event, |said| format!("Evento: {said}")),
            ))
            .spacing(20.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
