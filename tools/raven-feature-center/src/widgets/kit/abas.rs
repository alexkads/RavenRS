//! Tabs: a bar with a view per tab or the bar alone, two looks, three sizes, the bar kept from
//! changing, and the tabs that do not fit going to a menu at the end of the bar.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "TabsDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Abas";

/// The channels of the chat, more than fit a narrow column.
pub const CANAIS: [&str; 6] = ["Todas", "WhatsApp", "Instagram", "Messenger", "Telegram", "Site"];

/// Every public feature of Tabs on one page.
#[derive(Debug, Default)]
pub struct TabsPage;

impl View for TabsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let section = cx.use_state(|| 0_usize);
        let channel = cx.use_state(|| 0_usize);
        let look = cx.use_state(|| 1_usize);
        let sizes = [ButtonSize::Small, ButtonSize::Medium, ButtonSize::Large];
        let bars: Vec<AnyView> = sizes
            .into_iter()
            .map(|size| {
                let picked = State::new(0_usize);
                Box::new(Tabs::new(picked.binding()).size(size).item("Hoje").item("Semana").item("Mês"))
                    as AnyView
            })
            .collect();
        let mut channels = Tabs::new(channel.binding()).size(ButtonSize::Small);
        let mut segmented = Tabs::new(look.binding()).variant(TabsVariant::Segmented).size(ButtonSize::Small);
        for name in CANAIS {
            channels = channels.item(name);
            segmented = segmented.item(name);
        }
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Troca entre vistas do mesmo lugar. As abas que não cabem vão para o botão no fim da faixa.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Com conteúdo").style(TextStyle::Headline),
                    Tabs::new(section.binding())
                        .tab("Atividade", Text::new("O que aconteceu hoje."))
                        .tab("Ajustes", Text::new("Preferências da conta."))
                        .tab("Cobrança", Text::new("Faturas e cartão.")),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
                VStack::new((
                    Text::new("Seis canais numa coluna estreita").style(TextStyle::Headline),
                    VStack::new((channels, segmented)).spacing(12.0).width(360.0).align(Align::Stretch),
                    Text::new(format!("Canal: {}", CANAIS[channel.get().min(CANAIS.len() - 1)])),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Tamanhos").style(TextStyle::Headline),
                    VStack::new(bars).spacing(12.0).width(360.0).align(Align::Stretch),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Travada enquanto espera").style(TextStyle::Headline),
                    Tabs::new(State::new(1_usize).binding()).item("Rascunho").item("Enviando").disabled(true),
                ))
                .spacing(8.0)
                .align(Align::Start),
            ))
            .spacing(16.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Stretch),
        )
    }
}
