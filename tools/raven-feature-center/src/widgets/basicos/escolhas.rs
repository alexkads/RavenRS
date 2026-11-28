//! RadioGroup, SegmentedControl and Slider: one choice among a few, a few segments with or without
//! icons, and a value along a range, each off, read only, and with options that cannot be chosen.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "ChoicesDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Escolhas";

const PLANOS: [&str; 3] = ["Mensal", "Anual", "Vitalício"];
const VISOES: [&str; 3] = ["Dia", "Semana", "Mês"];

/// Every public feature of RadioGroup, SegmentedControl and Slider on one page.
#[derive(Debug, Default)]
pub struct ChoicesPage;

impl View for ChoicesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let plan = cx.use_state(|| 0_usize);
        let view = cx.use_state(|| 0_usize);
        let density = cx.use_state(|| 1_usize);
        let volume = cx.use_state(|| 40.0_f32);
        let warmth = cx.use_state(|| 0.0_f32);
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Uma escolha entre poucas, segmentos e um valor ao longo de uma faixa. Desligados, só leitura ou com opções que não dá para escolher.")
                    .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Opções").style(TextStyle::Headline),
                    RadioGroup::new(plan.binding(), PLANOS).label("Plano").disable_option(2),
                    Text::new(format!("Plano: {}", PLANOS[plan.get().min(2)])),
                    RadioGroup::new(plan.binding(), PLANOS).label("Plano travado").read_only(true),
                    RadioGroup::new(plan.binding(), PLANOS).label("Plano desligado").disabled(true),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Segmentos").style(TextStyle::Headline),
                    SegmentedControl::new(view.binding(), VISOES).label("Visão").disable_option(1),
                    Text::new(format!("Visão: {}", VISOES[view.get().min(2)])),
                    SegmentedControl::with_icons(
                        density.binding(),
                        [(IconName::List, "Compacta"), (IconName::LayoutDashboard, "Confortável")],
                    )
                    .label("Densidade"),
                    SegmentedControl::new(view.binding(), VISOES).label("Visão travada").read_only(true),
                    SegmentedControl::new(view.binding(), VISOES).label("Visão desligada").disabled(true),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Faixa de valores").style(TextStyle::Headline),
                    Slider::new(volume.binding())
                        .range(0.0, 100.0)
                        .step(5.0)
                        .marks([0.0, 50.0, 100.0])
                        .label("Volume")
                        .width(280.0)
                        .show_value()
                        .value_format(|v| format!("{v:.0}%")),
                    Slider::new(warmth.binding())
                        .range(-50.0, 50.0)
                        .neutral(0.0)
                        .track_colors([Color::hex(0x4A7BD8), Color::WHITE, Color::hex(0xE08A3C)])
                        .label("Temperatura")
                        .width(280.0),
                    Slider::new(volume.binding()).range(0.0, 100.0).label("Volume travado").width(280.0).read_only(true),
                    Slider::new(volume.binding()).range(0.0, 100.0).label("Volume desligado").width(280.0).disabled(true),
                    Text::new(format!("Volume: {:.0}", volume.get())),
                ))
                .spacing(8.0)
                .align(Align::Start),
            ))
            .spacing(16.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
