//! `VirtualList` and `SortableList`, every feature: ten thousand rows that select, activate, jump
//! to a row and say which rows show; rows measured as they appear next to rows of known heights;
//! a conversation that stays at the end; a list across; rows wider than the view; and the
//! priorities of a team, put in order by dragging or with Alt and the arrows.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "VirtualListDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Listas longas";

/// The uses, in the order of the tabs.
pub const USES: [&str; 4] = ["Dez mil linhas", "Alturas", "Conversa e faixa", "Ordenar"];

/// How many rows the long list has.
pub const ROWS: usize = 10_000;

/// The priorities the team starts with.
pub const PRIORITIES: [&str; 5] =
    ["Fechar o caixa", "Responder clientes", "Conferir estoque", "Pagar fornecedores", "Revisar contratos"];

/// A paragraph whose length changes with the row, so measured rows differ.
fn note(index: usize) -> String {
    let words = ["Entrega", "conferida", "pelo", "depósito", "com", "nota", "e", "assinatura"];
    let length = 4 + index * 7 % 24;
    (0..length).map(|i| words[i % words.len()]).collect::<Vec<_>>().join(" ")
}

/// The page: a tab per use, and a line with what happened last.
#[derive(Debug, Default)]
pub struct VirtualListPage;

impl View for VirtualListPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let tab = cx.use_state(|| 0_usize);
        let said = cx.use_state(|| "Escolha uma linha, ou dê Enter numa delas.".to_string());
        let chosen = cx.use_state(|| None::<usize>);
        let jump = cx.use_state(|| None::<usize>);
        let showing = cx.use_state(|| 0..0_usize);
        // How far the long list is scrolled, as it told in its last build.
        let probe = cx.use_state(|| Rc::new(Cell::new(Default::default()))).get();
        let messages = cx.use_state(|| 12_usize);
        let order = cx.use_state(|| (0..PRIORITIES.len()).collect::<Vec<usize>>());
        let height = if narrow { 300.0 } else { 360.0 };

        let body: AnyView = match tab.get() {
            0 => {
                let (activate_line, seen) = (said.clone(), showing.clone());
                let list = VirtualList::new(ROWS, |index| {
                    HStack::new((
                        Text::new(format!("#{:05}", index + 1))
                            .style(TextStyle::Caption)
                            .foreground(Semantic::InkMuted),
                        Text::new(format!("Pedido {}", index + 1)),
                    ))
                    .spacing(12.0)
                    .padding(Insets::symmetric(6.0, 12.0))
                })
                .label("Dez mil pedidos")
                .key(|index| index as u64)
                .row_height(36.0)
                .spacing(2.0)
                .padding(4.0)
                .height(height)
                .selection(chosen.binding())
                .scroll_to(jump.binding())
                .row_name(|index| format!("Pedido {}", index + 1))
                .on_activate(move |index| activate_line.set(format!("Abriu o pedido {}", index + 1)))
                .on_visible(move |rows| seen.set(rows))
                .probe(probe.clone());
                let (to_middle, to_start) = (jump.clone(), jump.clone());
                let rows = showing.get();
                let place = probe.get();
                Box::new(
                    VStack::new((
                        HStack::new((
                            Button::new("Ir para o pedido 5000").on_click(move || to_middle.set(Some(4_999))),
                            Button::new("Voltar ao início")
                                .style(ButtonStyle::Secondary)
                                .on_click(move || to_start.set(Some(0))),
                        ))
                        .spacing(8.0)
                        .wrap(),
                        list,
                        Text::new(format!(
                            "Escolhido: {}",
                            chosen
                                .get()
                                .map_or_else(|| "nenhum".to_string(), |i| format!("pedido {}", i + 1))
                        )),
                        Text::new(format!("À vista: linhas {} a {}", rows.start + 1, rows.end)),
                        Text::new(format!("Rolagem: {:.0} de {:.0} px", place.offset, place.total))
                            .style(TextStyle::Caption)
                            .foreground(Semantic::InkMuted),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
            1 => {
                let measured = VirtualList::new(2_000, |index| {
                    VStack::new((
                        Text::new(format!("Entrega {}", index + 1)).style(TextStyle::Headline),
                        Text::new(note(index)).style(TextStyle::Callout),
                    ))
                    .spacing(4.0)
                    .padding(Insets::symmetric(8.0, 12.0))
                })
                .label("Entregas medidas")
                .estimated_height(64.0)
                .spacing(4.0)
                .height(height);
                let known = VirtualList::new(2_000, |index| {
                    Text::new(format!("Página {} · {} linhas", index + 1, 1 + index % 4))
                        .style(TextStyle::Callout)
                })
                .label("Páginas de altura conhecida")
                .row_heights(|index| 28.0 + (index % 4) as f32 * 14.0)
                .spacing(1.0)
                .height(height);
                let both: AnyView = if narrow {
                    Box::new(VStack::new((measured, known)).spacing(12.0).align(Align::Stretch))
                } else {
                    Box::new(
                        HStack::new((
                            VStack::new((
                                Text::new("Medidas ao aparecer").style(TextStyle::Headline),
                                measured,
                            ))
                            .spacing(6.0)
                            .width(380.0)
                            .align(Align::Stretch),
                            VStack::new((Text::new("Alturas conhecidas").style(TextStyle::Headline), known))
                                .spacing(6.0)
                                .width(300.0)
                                .align(Align::Stretch),
                        ))
                        .spacing(24.0)
                        .align(Align::Start),
                    )
                };
                Box::new(both)
            }
            2 => {
                let count = messages.get();
                let arrived = messages.clone();
                let arrive_line = said.clone();
                let chat = VirtualList::new(count, |index| {
                    Text::new(format!("Mensagem {}: {}", index + 1, note(index)))
                        .style(TextStyle::Callout)
                        .padding(Insets::symmetric(6.0, 10.0))
                })
                .label("Conversa")
                .key(|index| index as u64)
                .follow_end(true)
                .estimated_height(40.0)
                .height(if narrow { 220.0 } else { 260.0 });
                let strip = VirtualList::new(500, |index| {
                    Text::new(format!("{}", index + 1)).style(TextStyle::Headline).padding(12.0)
                })
                .label("Faixa de números")
                .horizontal()
                .row_height(64.0)
                .spacing(4.0)
                .row_align(Align::Center)
                .height(64.0);
                let wide = VirtualList::new(200, |index| {
                    Text::new(format!("Linha {} · {}", index + 1, note(index + 20)))
                        .style(TextStyle::Callout)
                        .width(900.0)
                })
                .bare()
                .row_height(28.0)
                .scroll_across(900.0)
                .height(160.0);
                Box::new(
                    VStack::new((
                        Text::new("Fica no fim enquanto chegam mensagens").style(TextStyle::Headline),
                        chat,
                        HStack::new((
                            Button::new("Chegar mensagem").on_click(move || {
                                arrived.update(|n| *n += 1);
                                arrive_line.set(format!("Chegou a mensagem {}", arrived.peek()));
                            }),
                            Text::new(format!("{count} mensagens")),
                        ))
                        .spacing(12.0)
                        .align(Align::Center),
                        Text::new("Da esquerda para a direita").style(TextStyle::Headline),
                        strip,
                        Text::new("Linhas mais largas que a vista, sem foco próprio")
                            .style(TextStyle::Headline),
                        Text::new("Embutida, a lista deixa o foco e os nomes para quem está em volta.")
                            .style(TextStyle::Caption)
                            .foreground(Semantic::InkMuted),
                        Card::new(wide).padding(4.0),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
            _ => {
                let now = order.get();
                let (shown, moved, line) = (now.clone(), order.clone(), said.clone());
                let names = now.clone();
                let tight = now.clone();
                let sortable = SortableList::new(now.len(), move |i| {
                    Text::new(format!("{}. {}", i + 1, PRIORITIES[shown[i]]))
                        .padding(Insets::symmetric(8.0, 12.0))
                })
                .label("Prioridades da equipe")
                .key({
                    let keys = now.clone();
                    move |i| keys[i] as u64
                })
                .row_name(move |i| PRIORITIES[names[i]].to_string())
                .spacing(4.0)
                .on_reorder(move |from, to| {
                    let item = PRIORITIES[moved.peek()[from]];
                    moved.update(|v| {
                        let it = v.remove(from);
                        v.insert(to, it);
                    });
                    line.set(format!("{item} foi da posição {} para a {}", from + 1, to + 1));
                });
                let plain = SortableList::new(tight.len(), move |i| {
                    Card::new(Text::new(PRIORITIES[tight[i]]).style(TextStyle::Callout)).padding(8.0)
                })
                .label("Prioridades sem alça")
                .grip(false)
                .spacing(8.0);
                Box::new(
                    VStack::new((
                        Text::new("Arraste uma linha, ou use Alt com as setas").style(TextStyle::Headline),
                        sortable,
                        Text::new("Sem a alça, em cartões").style(TextStyle::Headline),
                        plain,
                        Text::new(format!("Primeira: {}", PRIORITIES[order.get()[0]])),
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
        };
        let picker: AnyView = if narrow {
            Box::new(Select::new(tab.binding(), USES).label("Uso"))
        } else {
            Box::new(SegmentedControl::new(tab.binding(), USES))
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                picker,
                Keyed::new(tab.get() as u64, body),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
