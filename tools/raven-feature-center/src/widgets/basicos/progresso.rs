//! ProgressBar: how far a task is, in a tone or a color of its own, as thick as asked, with a
//! label, the percent and a detail under the bar or on the line of the label.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "ProgressBarDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Progresso";

/// Every public feature of ProgressBar on one page.
#[derive(Debug, Default)]
pub struct ProgressPage;

impl View for ProgressPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let done = cx.use_state(|| 3_u32);
        let inline = cx.use_state(|| true);
        let total = 5_u32;
        let share = done.get() as f32 / total as f32;
        let (more, less) = (done.clone(), done.clone());
        let detail = format!("{} de 1.360 testes", 272 * done.get());
        let mut stage = ProgressBar::new(share)
            .label(format!("Estágio {} de {total} · Testes", done.get()))
            .percent()
            .detail(detail.clone())
            .detail_inline(inline.get());
        if done.get() == total {
            stage = stage.tone(Tone::Success);
        }
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Quanto de uma tarefa já foi: com rótulo, percentual e um detalhe embaixo ou na mesma linha.")
                    .foreground(Semantic::InkMuted),
                Text::new("O build").style(TextStyle::Headline),
                stage,
                HStack::new((
                    Button::new("Avançar um estágio").style(ButtonStyle::Secondary).on_click(move || {
                        more.update(|n| *n = (*n + 1).min(total));
                    }),
                    Button::new("Voltar um estágio").style(ButtonStyle::Ghost).on_click(move || {
                        less.update(|n| *n = n.saturating_sub(1));
                    }),
                    Toggle::new(inline.binding()).label("Detalhe na linha do rótulo"),
                ))
                .spacing(12.0)
                .align(Align::Center)
                .wrap(),
                Text::new(format!("Detalhe: {detail}")).foreground(Semantic::InkMuted),
                Text::new("Tons, cor e espessura").style(TextStyle::Headline),
                ProgressBar::new(0.25),
                ProgressBar::new(0.9).tone(Tone::Danger).label("Disco").percent(),
                ProgressBar::new(0.5).color(Color::rgb(0.18, 0.83, 0.75)).height(10.0).label("Cor própria, 10 pt"),
                ProgressBar::new(0.4).label("Copiando").percent().detail("12 de 30 fotos"),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(720.0)
            .align(Align::Stretch),
        )
    }
}
