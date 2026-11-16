//! `Link`, `Anchor`, `NinePointAnchor` and `QuarterTurn`, every feature: links that leave the app
//! and links that jump to a place on the page, the point of reference of a box being resized, and
//! names turned down the side of a strip. A line says which point is chosen.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "LinksDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Ligações e posição";

/// The anchor the links on the page jump to.
pub const SUMMARY: &str = "resumo";
/// The address of the documentation, which a link opens outside the app.
pub const DOCS: &str = "https://alexkads.github.io/RavenRS/components/";

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn point_name(point: AnchorPoint) -> &'static str {
    match point {
        AnchorPoint::TopLeft => "acima à esquerda",
        AnchorPoint::Top => "acima",
        AnchorPoint::TopRight => "acima à direita",
        AnchorPoint::Left => "à esquerda",
        AnchorPoint::Center => "no centro",
        AnchorPoint::Right => "à direita",
        AnchorPoint::BottomLeft => "abaixo à esquerda",
        AnchorPoint::Bottom => "abaixo",
        AnchorPoint::BottomRight => "abaixo à direita",
    }
}

/// The page: links, the nine points, the turned names and the place the links jump to.
#[derive(Debug, Default)]
pub struct LinksPage;

impl View for LinksPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let point = cx.use_state(|| AnchorPoint::Center);
        let corner = cx.use_state(|| AnchorPoint::TopLeft);
        let jump = format!("#{SUMMARY}");

        let links = VStack::new((
            Link::new("Documentação dos componentes", DOCS).icon(IconName::ExternalLink),
            Link::new("Ir para o resumo", jump.clone()).style(TextStyle::Callout),
            HStack::new((
                Link::new("Topo", "#topo").muted(),
                Link::new("Site", "https://alexkads.github.io/RavenRS/").muted(),
                Link::new("Resumo", jump).muted(),
            ))
            .spacing(16.0),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let points = HStack::new((
            NinePointAnchor::new(point.binding()).label("Ponto de referência"),
            NinePointAnchor::new(corner.binding()).label("Canto da marca d'água").cell_size(16.0),
        ))
        .spacing(24.0)
        .align(Align::Center);
        let point_line = format!(
            "A caixa cresce a partir do ponto {} · a marca d'água fica {}",
            point_name(point.get()),
            point_name(corner.get())
        );

        let strip = HStack::new((
            QuarterTurn::new(Text::new("Arquivos"), Turn::Clockwise),
            QuarterTurn::new(Text::new("Busca").style(TextStyle::Caption), Turn::CounterClockwise),
        ))
        .spacing(12.0)
        .align(Align::Center);
        let strip_note = Text::new("Os nomes descem pela faixa, e o leitor de tela lê o mesmo texto.")
            .foreground(Semantic::InkMuted);

        let summary = Anchor::new(
            SUMMARY,
            VStack::new((
                heading("Resumo"),
                Text::new("Os links com # trazem você até aqui, logo abaixo da barra do topo."),
            ))
            .spacing(4.0)
            .align(Align::Start),
        )
        .offset(56.0);

        cx.child(
            &VStack::new((
                Anchor::new("topo", VStack::new((heading("Links"), links)).spacing(8.0)),
                VStack::new((heading("Nove pontos"), points, Text::new(point_line))).spacing(8.0),
                VStack::new((heading("Um quarto de volta"), strip, strip_note)).spacing(8.0),
                summary,
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
