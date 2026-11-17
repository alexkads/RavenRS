//! Text: the full type scale, custom appearance, alignment, truncation and reactive content.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "TextDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Texto";
/// The full content preserved for assistive technology despite visual truncation.
pub const LONG: &str =
    "Este texto é longo e fica visualmente cortado, mas continua inteiro para quem usa leitor de tela.";

/// Every public feature of Text on one page.
#[derive(Debug, Default)]
pub struct TextPage;

impl View for TextPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let count = cx.use_state(|| 0_u32);
        let dynamic = count.clone();
        let increment = count.clone();
        let scale: Vec<AnyView> = [
            ("Métrica", TextStyle::Metric),
            ("Destaque", TextStyle::Display),
            ("Título", TextStyle::Title),
            ("Cabeçalho", TextStyle::Headline),
            ("Corpo", TextStyle::Body),
            ("Chamada", TextStyle::Callout),
            ("Legenda", TextStyle::Caption),
        ]
        .into_iter()
        .map(|(text, style)| Box::new(Text::new(text).style(style)) as AnyView)
        .collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                VStack::new(scale).spacing(4.0).align(Align::Start),
                Text::new("Aparência personalizada").style(TextStyle::Headline),
                Text::new("Tamanho, peso e cor")
                    .font_size(20.0)
                    .font_weight(Weight::SEMIBOLD)
                    .foreground(Semantic::InkMuted),
                Text::new("Alinhamento").style(TextStyle::Headline),
                VStack::new((
                    Text::new("À esquerda").align(TextAlign::Start),
                    Text::new("Ao centro").align(TextAlign::Center),
                    Text::new("À direita").align(TextAlign::End),
                ))
                .spacing(4.0)
                .width(320.0),
                Text::new("Uma linha, texto completo no leitor de tela").style(TextStyle::Headline),
                Text::new(LONG).truncate().width(280.0),
                Text::new("Conteúdo que acompanha o estado").style(TextStyle::Headline),
                Text::bound(&count, |n| format!("Vinculado: {n}")),
                Text::dynamic(move || format!("Calculado: {}", dynamic.get() * 2)),
                Button::new("Alterar conteúdo").on_click(move || increment.update(|n| *n += 1)),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
