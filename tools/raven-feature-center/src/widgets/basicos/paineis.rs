//! SplitPanes: panes side by side or stacked, a bar to drag between each two with limits, sizes the
//! app keeps, a pane hidden and shown again, and panes that fold to their edge with an arrow.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "SplitPanesDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Painéis divididos";

/// A pane's content: a title and a line under it.
fn column(title: &'static str, detail: &'static str) -> impl View + 'static {
    VStack::new((
        Text::new(title).style(TextStyle::Headline),
        Text::new(detail).foreground(Semantic::InkMuted),
    ))
    .spacing(4.0)
    .padding(16.0)
    .align(Align::Start)
}

/// Every public feature of SplitPanes on one page.
#[derive(Debug, Default)]
pub struct SplitPanesPage;

impl View for SplitPanesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let sessions_folded = cx.use_state(|| false);
        let sizes = cx.use_state(|| vec![Some(220.0_f32), None, Some(220.0)]);
        let saved = cx.use_state(|| 0_u32);
        let details = cx.use_state(|| true);
        let (count, shown) = (saved.clone(), details.clone());
        let folded = sessions_folded.get();
        let widths: Vec<String> = sizes
            .get()
            .iter()
            .map(|s| s.map_or_else(|| "o resto".to_owned(), |w| format!("{w:.0}")))
            .collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Painéis lado a lado ou empilhados, com uma barra de arrastar entre cada dois. Um painel pode recolher até a borda e voltar no mesmo tamanho.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Lado a lado, com as colunas que recolhem").style(TextStyle::Headline),
                    SplitPanes::horizontal()
                        .pane(
                            SplitPane::new(column("Sessões", "Caixa 1 e caixa 2"))
                                .size(220.0)
                                .range(160.0, 320.0)
                                .label("Sessões")
                                .collapsed(sessions_folded.binding()),
                        )
                        .pane(SplitPane::new(column("Cupom", "Ocupa o que sobra")).range(240.0, f32::MAX))
                        .pane(
                            SplitPane::new(column("Movimento", "Entradas e saídas do dia"))
                                .size(220.0)
                                .range(160.0, 320.0)
                                .label("Movimento")
                                .collapsible(true),
                        )
                        .sizes(sizes.binding())
                        .on_end(move || count.update(|n| *n += 1))
                        .height(200.0),
                    Text::new(if folded { "Sessões: recolhida" } else { "Sessões: aberta" }),
                    Text::new(format!("Larguras guardadas: {}", widths.join(", "))),
                    Text::new(format!("Tamanhos salvos: {}", saved.get())),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
                VStack::new((
                    Text::new("Empilhados, com um painel que some").style(TextStyle::Headline),
                    Button::new(if details.get() { "Esconder detalhes" } else { "Mostrar detalhes" })
                        .style(ButtonStyle::Secondary)
                        .on_click(move || shown.set(!shown.get())),
                    SplitPanes::vertical()
                        .pane(SplitPane::new(column("Conversas", "A lista em cima")).range(60.0, f32::MAX))
                        .pane(
                            SplitPane::new(column("Detalhes", "Embaixo, do tamanho de antes ao voltar"))
                                .size(90.0)
                                .range(60.0, 160.0)
                                .visible(details.get()),
                        )
                        .height(240.0),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
            ))
            .spacing(16.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Stretch),
        )
    }
}
