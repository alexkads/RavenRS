//! `StepIndicator`, `StepBadge` and `MultiProgress`, every feature (#579): where the delivery of
//! a photo shoot stands, across and down, with a step that can go wrong; the marks of each state
//! in three sizes; and the copy of the photos in several bars. A line says what happened last.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "StepsAndMetersDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Etapas e medidores";

/// The steps of the delivery.
pub const STEPS: [&str; 4] = ["Cliente", "Fotos", "Revisão", "Entrega"];
/// How many photos there are to copy.
pub const PHOTOS: u64 = 120;

/// The steps, from where the delivery is and whether the photos are missing.
fn steps(current: usize, missing: bool) -> Vec<IndicatorStep> {
    STEPS
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let state = if i == 1 && missing {
                StepState::Error
            } else if i < current {
                StepState::Done
            } else if i == current {
                StepState::Current
            } else {
                StepState::Upcoming
            };
            let step = IndicatorStep::new(*title, state);
            if i == 1 && missing { step.note("Faltam duas fotos") } else { step }
        })
        .collect()
}

/// The page: the steps, the marks, the copy and a line with the last event.
#[derive(Debug, Default)]
pub struct StepsAndMetersPage;

impl View for StepsAndMetersPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Clique numa etapa para ir até ela.".to_string());
        let current = cx.use_state(|| 2_usize);
        let missing = cx.use_state(|| false);
        let copied = cx.use_state(|| 40_u64);

        let (across, across_line) = (current.clone(), said.clone());
        let row = StepIndicator::new(steps(current.get(), missing.get())).on_select(move |i| {
            across.set(i);
            across_line.set(format!("Etapa atual: {}", STEPS[i]));
        });
        let (down, down_line) = (current.clone(), said.clone());
        let column = StepIndicator::new(steps(current.get(), missing.get())).vertical().on_select(move |i| {
            down.set(i);
            down_line.set(format!("Etapa atual pela coluna: {}", STEPS[i]));
        });

        let badges = [
            (StepState::Done, "Feita"),
            (StepState::Current, "Atual"),
            (StepState::Upcoming, "Próxima"),
            (StepState::Error, "Com erro"),
        ];
        let marks: Vec<AnyView> = [20.0_f32, 28.0, 36.0]
            .into_iter()
            .map(|size| {
                let row: Vec<AnyView> = badges
                    .iter()
                    .enumerate()
                    .map(|(i, (state, word))| {
                        Box::new(
                            VStack::new((
                                StepBadge::new(i + 1).state(*state).size(size),
                                Text::new(*word).style(TextStyle::Caption),
                            ))
                            .spacing(4.0)
                            .align(Align::Center),
                        ) as AnyView
                    })
                    .collect();
                Box::new(HStack::new(row).spacing(if narrow { 12.0 } else { 24.0 }).align(Align::End))
                    as AnyView
            })
            .collect();

        let done = copied.get();
        let copy = MultiProgress::new()
            .label("Cópia das fotos")
            .meter("Cópia local", done, PHOTOS)
            .meter("Miniaturas", done / 3, PHOTOS / 3)
            .bar(
                ProgressBar::new(done as f32 / PHOTOS as f32)
                    .tone(Tone::Success)
                    .height(10.0)
                    .label("Envio para a nuvem")
                    .percent(),
            );
        let (more, more_line) = (copied.clone(), said.clone());
        let step = Button::new("Avançar a cópia").icon(IconName::Download).on_click(move || {
            let now = (more.get() + 20).min(PHOTOS);
            more.set(now);
            more_line.set(format!("Cópia local: {now} de {PHOTOS}"));
        });

        let steps_view: AnyView = if narrow {
            Box::new(column)
        } else {
            Box::new(HStack::new((row, column)).spacing(32.0).align(Align::Start))
        };
        cx.child(
            &VStack::new((
                Text::new("Entrega do ensaio").style(TextStyle::Headline),
                steps_view,
                Checkbox::new("Faltam fotos", missing.binding()),
                Text::new("Marcas de etapa").style(TextStyle::Headline),
                VStack::new(marks).spacing(12.0),
                Text::new("Cópia").style(TextStyle::Headline),
                copy,
                step,
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
