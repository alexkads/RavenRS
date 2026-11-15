//! `Stat`, `CountUp` and `Ticker`, every feature: the figures of a panel with their change, where
//! going up is good and where it is not, the one big number of the screen, numbers that count to
//! the value of the chosen period in their own format and pace, and strips that keep moving either
//! way. A line says which period is shown.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "LiveNumbersDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Números ao vivo";

/// The periods of the panel, in order.
pub const PERIODS: [&str; 3] = ["Semana", "Mês", "Ano"];

/// Revenue, spending and clients for each period.
const FIGURES: [(f64, f64, f64); 3] =
    [(12_400.0, 3_100.0, 18.0), (48_200.0, 12_900.0, 64.0), (571_000.0, 150_300.0, 702.0)];

/// The banks of the moving strip.
const BANKS: [&str; 5] = ["Banco do Norte", "Caixa Sul", "Coop Serra", "Banco Litoral", "Crédito Centro"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// Writes a value in reais, with thousands separated by dots.
fn reais(value: f64) -> String {
    let whole = value.round() as i64;
    let digits = whole.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    format!("R$ {out}")
}

/// The page: stats, counting numbers and moving strips, with a period that changes the values.
#[derive(Debug, Default)]
pub struct LiveNumbersPage;

impl View for LiveNumbersPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let period = cx.use_state(|| 1_usize);
        let index = period.get().min(PERIODS.len() - 1);
        let (revenue, spending, clients) = FIGURES[index];

        let picker: AnyView = if narrow {
            Box::new(Select::new(period.binding(), PERIODS).label("Período").fill())
        } else {
            Box::new(SegmentedControl::new(period.binding(), PERIODS).label("Período"))
        };

        let stats = HStack::new((
            Stat::new("Receita", reais(revenue)).large().delta("+12,4%", true),
            Stat::new("Despesas", reais(spending)).delta_toned("+8,1%", true, false),
            Stat::new("Inadimplência", "2,3%").delta_toned("-0,6 p.p.", false, true),
            Stat::new("Ticket médio", "R$ 312").delta("-3,0%", false),
        ))
        .spacing(if narrow { 16.0 } else { 32.0 })
        .wrap()
        .align(Align::Start);

        let counts = HStack::new((
            VStack::new((
                Text::new("Receita").style(TextStyle::Caption),
                CountUp::new(revenue).format(reais),
            ))
            .spacing(2.0)
            .align(Align::Start),
            VStack::new((
                Text::new("Clientes novos").style(TextStyle::Caption),
                CountUp::new(clients).format(|v| format!("{v:.0}+")).seconds(0.8).easing(Easing::Linear),
            ))
            .spacing(2.0)
            .align(Align::Start),
            VStack::new((
                Text::new("Despesas, sem contar ao abrir").style(TextStyle::Caption),
                CountUp::new(spending).format(reais).style(TextStyle::Title).start_at_value().seconds(0.6),
            ))
            .spacing(2.0)
            .align(Align::Start),
        ))
        .spacing(if narrow { 16.0 } else { 40.0 })
        .wrap()
        .align(Align::End);

        let banks =
            HStack::new(BANKS.map(|bank| Badge::new(bank).icon(IconName::CreditCard)).to_vec()).spacing(8.0);
        let quotes = HStack::new((
            Text::new("USD 5,12 ▲"),
            Text::new("EUR 5,48 ▼"),
            Text::new("IBOV 128.400 ▲"),
            Text::new("SELIC 10,75%"),
        ))
        .spacing(24.0);
        let strips = VStack::new((
            Ticker::new(banks),
            Ticker::new(quotes).speed(if narrow { 24.0 } else { 60.0 }).gap(48.0).reverse(),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        cx.child(
            &VStack::new((
                VStack::new((heading("Painel do período"), picker, stats)).spacing(8.0).align(Align::Start),
                VStack::new((heading("Contando até o valor"), counts)).spacing(8.0),
                VStack::new((heading("Faixas que andam"), strips)).spacing(8.0).align(Align::Stretch),
                Text::new(format!(
                    "Período: {} · receita {} · {clients:.0} clientes novos",
                    PERIODS[index],
                    reais(revenue)
                )),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
