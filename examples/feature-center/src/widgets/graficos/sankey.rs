//! `SankeyChart`, every feature: where a family's money goes, with each way of putting nodes in
//! columns, ribbons in one color or going to their target's, colors of the theme for some nodes,
//! a period that draws the flows in again, and a diagram that says nothing under the pointer.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "SankeyDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Diagrama de Sankey";

/// The ways of putting nodes in columns, in the order of their switch.
pub const ALIGNS: [&str; 4] = ["Justificado", "À esquerda", "À direita", "Ao centro"];
/// The months the money comes from.
pub const MONTHS: [&str; 2] = ["Outubro", "Novembro"];

fn reais(v: f64) -> String {
    format!("R$ {v:.0}")
}

/// The page.
#[derive(Debug, Default)]
pub struct SankeyPage;

impl View for SankeyPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let align = cx.use_state(|| 0_usize);
        let month = cx.use_state(|| 0_usize);
        let gradient = cx.use_state(|| true);
        let narrow = cx.viewport().width < 720.0;
        let wide = if narrow { (cx.viewport().width - 32.0).max(240.0) } else { 820.0 };
        let how = [SankeyAlign::Justify, SankeyAlign::Left, SankeyAlign::Right, SankeyAlign::Center]
            [align.get().min(3)];
        let extra = if month.get() == 0 { 0.0 } else { 600.0 };

        let money = SankeyChart::new()
            .title(format!("Para onde foi o dinheiro em {}", MONTHS[month.get().min(1)]))
            .flow("Salário", "Casa", 2400.0)
            .flow("Salário", "Mercado", 1300.0)
            .flow("Salário", "Poupança", 900.0 + extra)
            .flow("Freelas", "Poupança", 600.0)
            .flow("Freelas", "Lazer", 400.0)
            .flow("Poupança", "Fundos", 1000.0 + extra)
            .flow("Poupança", "Reserva", 500.0)
            .flow("Casa", "Aluguel", 1800.0)
            .flow("Casa", "Contas", 600.0)
            .node_color("Poupança", Semantic::Success)
            .node_color("Lazer", Semantic::Warning)
            .format(reais)
            .align(how)
            .appear_key(month.get())
            .size(wide, if narrow { 360.0 } else { 420.0 });
        let money = if gradient.get() { money } else { money.no_gradient() };

        // Whole from the first frame, silent under the pointer: a picture in a report.
        let quiet = SankeyChart::new()
            .alt("Energia: da usina às casas e às fábricas")
            .flow("Usina", "Casas", 60.0)
            .flow("Usina", "Fábricas", 40.0)
            .flow("Fábricas", "Perdas", 8.0)
            .no_appear()
            .hide_tooltip()
            .height(200.0);

        let align_picker: AnyView = if narrow {
            Box::new(Select::new(align.binding(), ALIGNS).label("Colunas"))
        } else {
            Box::new(SegmentedControl::new(align.binding(), ALIGNS))
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                align_picker,
                HStack::new((
                    SegmentedControl::new(month.binding(), MONTHS),
                    Toggle::new(gradient.binding()).label("Fitas em degradê"),
                ))
                .spacing(12.0)
                .wrap(),
                money,
                Text::new(format!("Colunas: {}", ALIGNS[align.get().min(3)])),
                Text::new("Sem dica nem animação").style(TextStyle::Headline),
                quiet,
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Start),
        )
    }
}
