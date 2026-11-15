//! `HeroBanner`, `IconTile`, `EmptyState`, `ShimmerText`, `Dot` and `OverlayBadge`, every
//! feature (#579): what a counter shows from across the room, tinted icons, a day with no orders
//! yet, a search under way, marks of state and badges on the corner of an avatar and of a bell.
//! A line says what happened last.

use ravenrs::core::Color;
use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "HighlightsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Destaques";

/// The tones of the banner, in the order of `Tone`.
pub const TONES: [&str; 5] = ["Neutro", "Destaque", "Sucesso", "Atenção", "Perigo"];

fn tone(index: usize) -> Tone {
    match index {
        0 => Tone::Neutral,
        1 => Tone::Accent,
        2 => Tone::Success,
        3 => Tone::Warning,
        _ => Tone::Danger,
    }
}

/// A heading and what goes under it, closer together than the sections are.
fn section<C: ViewSeq>(children: C) -> VStack<C> {
    VStack::new(children).spacing(8.0).align(Align::Stretch)
}

/// The page: the highlights and a line with the last event.
#[derive(Debug, Default)]
pub struct HighlightsPage;

impl View for HighlightsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_string());
        let tone_at = cx.use_state(|| 2_usize);
        let orders = cx.use_state(|| 0_u32);
        let searching = cx.use_state(|| true);
        let online = cx.use_state(|| true);
        let unread = cx.use_state(|| 3_u32);

        let (sale, sale_line) = (orders.clone(), said.clone());
        let banner = HeroBanner::new("Pode retirar o cartão")
            .description("Pagamento aprovado · R$ 318,40")
            .icon(IconName::CircleCheck)
            .tone(tone(tone_at.get()))
            .action(Button::new("Nova venda").style(ButtonStyle::Secondary).on_click(move || {
                sale.set(0);
                sale_line.set("Caixa pronto para a próxima venda".to_string());
            }));
        let branded = HeroBanner::new("Semana do café")
            .description("Faixa na cor da marca, com o texto no contraste que ela pede")
            .icon(IconName::Gift)
            .color(Color::hex(0x6B3E26));

        let tiles = HStack::new((
            IconTile::new(IconName::Info).label("Informação"),
            IconTile::new(IconName::CircleCheck).tone(Tone::Success).label("Tudo certo"),
            IconTile::new(IconName::TriangleAlert).tone(Tone::Warning).size(40.0).label("Atenção"),
            IconTile::new(IconName::Flag).tone(Tone::Danger).size(32.0).label("Urgente"),
            IconTile::new(IconName::Star).color(Color::hex(0xC9A46A)).size(56.0).label("Favorito"),
        ))
        .spacing(if narrow { 8.0 } else { 16.0 })
        .align(Align::Center);

        let today: AnyView = if orders.get() == 0 {
            let (first, first_line) = (orders.clone(), said.clone());
            let import_line = said.clone();
            Box::new(
                EmptyState::new("Nenhum pedido hoje")
                    .icon(IconName::Inbox)
                    .description("Os pedidos do dia aparecem aqui assim que chegam.")
                    .action(Button::new("Criar pedido").icon(IconName::Plus).on_click(move || {
                        first.set(1);
                        first_line.set("Pedido criado".to_string());
                    }))
                    .action(
                        Button::new("Importar")
                            .style(ButtonStyle::Secondary)
                            .on_click(move || import_line.set("Nada para importar".to_string())),
                    ),
            )
        } else {
            Box::new(Text::new(format!("{} pedido hoje", orders.get())))
        };

        let shimmer =
            ShimmerText::new("Procurando atualizações").style(TextStyle::Headline).active(searching.get());

        let dots = HStack::new((
            Dot::new().label("Rascunho"),
            Dot::new().tone(Tone::Warning).label("Não salvo"),
            Dot::new().tone(Tone::Success).size(12.0).label("Salvo"),
            Dot::new().color(Color::hex(0x3B82F6)).size(16.0).label("Sincronizando"),
        ))
        .spacing(16.0)
        .align(Align::Center);

        let reader = (unread.clone(), said.clone());
        let badges = HStack::new((
            OverlayBadge::new(
                Avatar::new("Ana Lima"),
                Dot::new().tone(Tone::Success).size(12.0).label("Disponível"),
            )
            .corner(FloatingCorner::BottomRight)
            .inset(4.0)
            .visible(online.get()),
            OverlayBadge::new(
                IconButton::new(IconName::Bell).label("Avisos da loja").on_click(move || {
                    reader.0.set(0);
                    reader.1.set("Avisos lidos".to_string());
                }),
                Badge::new(unread.get().to_string()).tone(Tone::Danger),
            )
            .corner(FloatingCorner::TopRight)
            .visible(unread.get() > 0),
        ))
        .spacing(32.0)
        .align(Align::Center);

        let tones: AnyView = if narrow {
            Box::new(Select::new(tone_at.binding(), TONES).label("Tom da faixa"))
        } else {
            Box::new(SegmentedControl::new(tone_at.binding(), TONES))
        };

        cx.child(
            &VStack::new((
                section((Text::new("Faixas").style(TextStyle::Headline), tones, banner, branded)),
                section((Text::new("Ícones com cor").style(TextStyle::Headline), tiles)),
                section((Text::new("Sem nada ainda").style(TextStyle::Headline), today)),
                section((
                    Text::new("Em andamento").style(TextStyle::Headline),
                    shimmer,
                    Checkbox::new("Procurando", searching.binding()),
                )),
                section((
                    Text::new("Marcas").style(TextStyle::Headline),
                    dots,
                    badges,
                    Checkbox::new("Ana disponível", online.binding()),
                    Text::new(format!("Avisos não lidos: {}", unread.get())),
                )),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
