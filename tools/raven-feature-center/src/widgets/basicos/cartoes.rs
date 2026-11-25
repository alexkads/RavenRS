//! Card: the grouped surface, its inside space, corners, fill, hairline and shadow, a card that is
//! clicked and the one chosen of a group, and the tones that say by color what a card is about.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "CardDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Cartões";

const PLANS: [&str; 3] = ["Básico", "Estúdio", "Rede"];

fn content(title: &str, line: &str) -> VStack<(Text, Text)> {
    VStack::new((Text::new(title).style(TextStyle::Headline), Text::new(line).foreground(Semantic::InkMuted)))
        .spacing(4.0)
}

/// Every public feature of Card on one page.
#[derive(Debug, Default)]
pub struct CardsPage;

impl View for CardsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let opened = cx.use_state(|| 0_u32);
        let plan = cx.use_state(|| 1_usize);
        let raised = cx.theme().elevation.raised;
        let count = opened.clone();
        let plans: Vec<AnyView> = PLANS
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let plan = plan.clone();
                Box::new(
                    Card::new(content(name, "Plano mensal"))
                        .selected(plan.get() == i)
                        .on_click(move || plan.set(i)),
                ) as AnyView
            })
            .collect();
        let tones: Vec<AnyView> = [
            ("Neutro", Tone::Neutral),
            ("Destaque", Tone::Accent),
            ("Pós-venda", Tone::Success),
            ("A conferir", Tone::Warning),
            ("Estornado", Tone::Danger),
        ]
        .into_iter()
        .map(|(name, tone)| Box::new(Card::new(content(name, "R$ 0,00")).tone(tone).padding(16.0)) as AnyView)
        .collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Uma superfície que agrupa um assunto: preenchimento, cantos, filete e sombra do tema, ajustáveis um a um.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Forma").style(TextStyle::Headline),
                    HStack::new((
                        Card::new(content("Padrão", "Margem, cantos e sombra do tema.")),
                        Card::new(content("Compacto", "Margem de 16 e cantos de 10."))
                            .padding(16.0)
                            .corner_radius(10.0),
                        Card::new(content("Plano", "Sem sombra, dentro de outra superfície.")).no_shadow(),
                        Card::new(content("Elevado", "A sombra mais alta do tema.")).shadow(raised),
                        Card::new(content("Sem filete", "Fundo de painel e nenhuma borda."))
                            .fill(Semantic::Surface)
                            .border(false),
                    ))
                    .spacing(16.0)
                    .wrap(),
                ))
                .spacing(12.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Clicável e escolhido").style(TextStyle::Headline),
                    Card::new(content("Relatório do mês", "Clique para abrir.")).on_click(move || count.update(|n| *n += 1)),
                    Text::new(format!("Aberto {} vezes", opened.get())),
                    HStack::new(plans).spacing(16.0).wrap(),
                    Text::new(format!("Plano escolhido: {}", PLANS[plan.get()])),
                ))
                .spacing(12.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Tons").style(TextStyle::Headline),
                    Text::new("O tom diz pela cor do que o cartão trata: o fundo leva o tom a 10% e o filete a 30%, como no Alerta. Neutro é o cartão de sempre.")
                        .foreground(Semantic::InkMuted),
                    HStack::new(tones).spacing(16.0).wrap(),
                ))
                .spacing(12.0)
                .align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(960.0)
            .align(Align::Start),
        )
    }
}
