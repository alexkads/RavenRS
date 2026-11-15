//! `RichText` and `FloatingPanel`, every feature: text with the marks chat apps use, in other
//! sizes and colors, taken literally, reduced to a preview, and with links the app opens; and a
//! panel over the app, in its corner or loose, reduced to its header with a summary.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "RichTextDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Texto rico";

/// The panel's name, which screen readers give its sphere.
pub const PANEL: &str = "Assistente da loja";

/// A message with every mark.
pub const MARKED: &str = "*Pedido 102 pronto.* Chega _amanhã_, não ~hoje~. Código `RV-102`.\n\
* Nota fiscal anexada\n\
- Troca em até 7 dias\n\
> Guarde a caixa até lá.\n\
Detalhes em https://ravenrs.dev/pedidos/102\n\
```\nrastreio: BR123456789\n```";

/// The page.
#[derive(Debug, Default)]
pub struct RichTextPage;

impl View for RichTextPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let open = cx.use_state(|| false);
        let minimized = cx.use_state(|| false);
        let loose = cx.use_state(|| false);
        let place = cx.use_state(|| None::<Point>);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_string());
        let narrow = cx.viewport().width < 720.0;
        let wide = if narrow { (cx.viewport().width - 32.0).max(240.0) } else { 560.0 };

        let opened = said.clone();
        let marked = RichText::new(MARKED).on_link(move |address| opened.set(format!("Abriu {address}")));
        let headline = RichText::new("*Resumo* do dia: `12` pedidos").style(TextStyle::Headline);
        let accent = RichText::new("_Frete grátis_ acima de *R$ 200*").foreground(Semantic::Accent);
        let literal =
            RichText::new("Marcas ao pé da letra: *não* fica _negrito_; o link https://ravenrs.dev fica")
                .links_only();

        let (shut, minimized_now) = (open.clone(), minimized.get());
        let panel = FloatingPanel::new(open.binding(), PANEL, move || {
            let close = shut.clone();
            Box::new(
                VStack::new((
                    RichText::new("*Olá!* Posso ajudar com _trocas_ e `rastreio`."),
                    Button::new("Fechar o assistente").on_click(move || close.set(false)),
                ))
                .spacing(8.0)
                .padding(12.0),
            )
        })
        .size(if narrow { 300.0 } else { 360.0 }, 320.0)
        .bottom_inset(24.0)
        .minimized(minimized.binding())
        .summary(|| Box::new(Text::new("2 mensagens novas")));
        let panel = if loose.get() { panel.movable(place.binding()) } else { panel };
        let opener = open.clone();

        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Todas as marcas").style(TextStyle::Headline),
                VStack::new((marked,)).width(wide),
                VStack::new((Text::new("Tamanho e cor").style(TextStyle::Headline), headline, accent))
                    .spacing(12.0)
                    .align(Align::Start),
                Text::new("Só os links").style(TextStyle::Headline),
                VStack::new((literal,)).width(wide),
                Text::new(format!("Prévia: {}", RichText::plain_text(MARKED))).style(TextStyle::Caption),
                Text::new("Painel flutuante").style(TextStyle::Headline),
                HStack::new((
                    Toggle::new(loose.binding()).label("Painel solto"),
                    Button::new("Abrir o assistente").on_click(move || opener.set(true)),
                ))
                .spacing(12.0)
                .wrap(),
                Text::new(format!(
                    "Painel: {}{}",
                    if open.get() { "aberto" } else { "fechado" },
                    if minimized_now { ", minimizado" } else { "" }
                )),
                Text::new(said.get()),
                panel,
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Start),
        )
    }
}
