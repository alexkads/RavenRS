//! Chronology: the moments of a record down a line, each with its date, a mark in its tone joined
//! to the next, a title, a detail and a badge; a click or Enter opens one, the arrows go between them.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "ChronologyDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Cronologia";

/// Every public feature of Chronology on one page.
#[derive(Debug, Default)]
pub struct ChronologyPage;

impl View for ChronologyPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let opened = cx.use_state(|| "Nenhum marco aberto.".to_owned());
        let tell = opened.clone();
        let marcos = vec![
            ChronologyItem::new(1, "02/03/2024", "Batismo").tone(Tone::Accent),
            ChronologyItem::new(2, "10/08/2025", "Membro ativo")
                .detail("Aprovado pela assembleia")
                .badge("Aprovado")
                .tone(Tone::Success),
            ChronologyItem::new(3, "01/02/2026", "Afastamento")
                .detail("A pedido")
                .badge("Pendente")
                .tone(Tone::Warning),
            ChronologyItem::new(4, "14/11/2026", "Transferência recusada")
                .badge("Recusado")
                .tone(Tone::Danger),
            ChronologyItem::new(5, "27/11/2026", "Visita marcada"),
        ];
        let names =
            ["", "Batismo", "Membro ativo", "Afastamento", "Transferência recusada", "Visita marcada"];
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Os marcos de um registro numa linha, do mais antigo ao mais novo. Clique ou Enter abre um; as setas andam entre eles.")
                    .foreground(Semantic::InkMuted),
                VStack::new((Chronology::new(marcos).label("Marcos de Lucas Oliveira").on_open(move |id| {
                    let name = names.get(id as usize).copied().unwrap_or_default();
                    tell.set(format!("Aberto: {name}"));
                }),))
                .width(520.0),
                Text::new(opened.get()),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
