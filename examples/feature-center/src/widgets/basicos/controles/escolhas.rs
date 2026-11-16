//! `Toggle`, `Checkbox`, `RadioGroup`, `SegmentedControl` and `FavoriteToggle`, every feature: the
//! preferences of an account, with each state side by side (on, small, read only, off) and a line
//! that sums up what is chosen.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ChoicesDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Escolhas";

/// The plans of the radio group, in order.
pub const PLANS: [&str; 3] = ["Gratuito", "Pessoal", "Equipe"];
/// The densities of the segmented control, in order.
pub const DENSITIES: [&str; 3] = ["Compacta", "Normal", "Folgada"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

fn yes(on: bool) -> &'static str {
    if on { "sim" } else { "não" }
}

/// The page: switches, check boxes, radios, segments and marks, and a line with what is chosen.
#[derive(Debug, Default)]
pub struct ChoicesPage;

impl View for ChoicesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let mail = cx.use_state(|| true);
        let sound = cx.use_state(|| false);
        let locked = cx.use_state(|| true);
        let terms = cx.use_state(|| false);
        let news = cx.use_state(|| true);
        let backup = cx.use_state(|| true);
        let plan = cx.use_state(|| 1_usize);
        let density = cx.use_state(|| 1_usize);
        let view = cx.use_state(|| 0_usize);
        let liked = cx.use_state(|| false);
        let starred = cx.use_state(|| true);

        let switches = VStack::new((
            Toggle::new(mail.binding()).label("Avisos por e-mail"),
            Toggle::new(sound.binding()).label("Som nos avisos").small(),
            Toggle::new(locked.binding()).label("Conta verificada (só leitura)").read_only(true),
            Toggle::new(locked.binding()).label("Modo de manutenção (desligado)").disabled(true),
        ))
        .spacing(8.0)
        .align(Align::Start);

        // `leading(40.0)` lines the labels of the boxes up with the switches above them.
        let boxes = VStack::new((
            Checkbox::new("Aceito os termos de uso", terms.binding()).leading(40.0),
            Checkbox::new("Receber novidades", news.binding()).leading(40.0),
            Checkbox::new("Cópia de segurança diária (do plano)", backup.binding())
                .leading(40.0)
                .read_only(true),
            Checkbox::new("Recurso em teste (indisponível)", backup.binding()).leading(40.0).disabled(true),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let plans = RadioGroup::new(plan.binding(), PLANS).label("Plano");

        let densities: AnyView = if narrow {
            Box::new(Select::new(density.binding(), DENSITIES).label("Densidade"))
        } else {
            Box::new(SegmentedControl::new(density.binding(), DENSITIES).label("Densidade"))
        };
        let views: AnyView = if narrow {
            Box::new(Select::new(view.binding(), ["Lista", "Grade", "Agenda"]).label("Visão"))
        } else {
            Box::new(
                SegmentedControl::with_icons(
                    view.binding(),
                    [(IconName::List, "Lista"), (IconName::Grid2x2, "Grade"), (IconName::Calendar, "Agenda")],
                )
                .label("Visão"),
            )
        };

        let marks = HStack::new((
            FavoriteToggle::new(liked.binding()).label("Retrato quente"),
            FavoriteToggle::new(starred.binding()).label("Projeto principal").star(),
            FavoriteToggle::new(liked.binding()).label("Retrato quente, grande").size(24.0),
            FavoriteToggle::new(starred.binding())
                .label("Projeto principal, em azul")
                .star()
                .color(Color::hex(0x2563EB)),
        ))
        .spacing(8.0)
        .align(Align::Center);

        let summary = format!(
            "E-mail: {} · som: {} · termos: {} · novidades: {} · plano: {} · densidade: {} · favorito: {} · estrela: {}",
            yes(mail.get()),
            yes(sound.get()),
            yes(terms.get()),
            yes(news.get()),
            PLANS[plan.get().min(2)],
            DENSITIES[density.get().min(2)],
            yes(liked.get()),
            yes(starred.get()),
        );

        cx.child(
            &VStack::new((
                VStack::new((heading("Interruptores"), switches)).spacing(8.0),
                VStack::new((heading("Caixas de seleção"), boxes)).spacing(8.0),
                VStack::new((heading("Uma entre várias"), plans, densities, views))
                    .spacing(8.0)
                    .align(Align::Start),
                VStack::new((heading("Favoritos"), marks)).spacing(8.0),
                Text::new(summary),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
