//! `YearView`, every feature: the twelve months of a year with the holidays, the paydays and the
//! birthdays marked in their tones, today with a ring, the day picked filled, the name of each
//! month as a button, and as many months on a line as chosen. A line says what was picked.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "YearViewDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Ano inteiro";

/// How many months go on a line, by the option chosen: as many as fit, or a number.
pub const COLUMNS: [&str; 5] = ["Automático", "2", "3", "4", "6"];

/// The national holidays with a fixed date: month and day.
const HOLIDAYS: [(u8, u8, &str); 8] = [
    (1, 1, "Confraternização"),
    (4, 21, "Tiradentes"),
    (5, 1, "Dia do Trabalho"),
    (9, 7, "Independência"),
    (10, 12, "Nossa Senhora Aparecida"),
    (11, 2, "Finados"),
    (11, 15, "Proclamação da República"),
    (12, 25, "Natal"),
];

/// The birthdays of the team: month and day.
const BIRTHDAYS: [(u8, u8, &str); 3] = [(3, 14, "Ana"), (7, 22, "Bia"), (11, 20, "Caio")];

/// Today, fixed so the page reads the same every time.
pub fn today() -> Date {
    Date::new(2026, 11, 15).unwrap_or_else(|| unreachable!("a real date"))
}

/// What happens on a day, if anything, and its tone.
pub fn what_is_on(day: Date) -> Option<(String, Tone)> {
    if let Some((_, _, name)) = HOLIDAYS.iter().find(|(m, d, _)| *m == day.month() && *d == day.day()) {
        return Some((format!("Feriado: {name}"), Tone::Danger));
    }
    if let Some((_, _, name)) = BIRTHDAYS.iter().find(|(m, d, _)| *m == day.month() && *d == day.day()) {
        return Some((format!("Aniversário de {name}"), Tone::Accent));
    }
    (day.day() == 5).then(|| ("Dia do pagamento".to_string(), Tone::Success))
}

/// The page.
#[derive(Debug, Default)]
pub struct YearViewPage;

impl View for YearViewPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let year = cx.use_state(|| today().year());
        let picked = cx.use_state(|| None::<Date>);
        let columns = cx.use_state(|| 0_usize);
        let said = cx.use_state(|| "Escolha um dia ou o nome de um mês.".to_string());
        let (pick_line, month_line) = (said.clone(), said.clone());
        // The month in the language of the center, as the year itself names it.
        let labels = cx.labels();

        let mut view = YearView::new(year.binding())
            .today(today())
            .selected(picked.binding())
            .marks(|day| what_is_on(day).map(|(text, tone)| DayMark::new().tone(tone).label(text)))
            .on_pick(move |day| {
                let what = what_is_on(day).map_or_else(|| "nada marcado".to_string(), |(text, _)| text);
                pick_line.set(format!("Escolheu {:02}/{:02}/{}: {what}", day.day(), day.month(), day.year()));
            })
            .on_month(move |first| month_line.set(format!("Abriu o mês de {}", labels.month_title(first))));
        // "Automático" lets as many months as fit go on a line; the others fix how many. A phone
        // has room for one month, so there the year always fits itself.
        if !narrow && let Some(count) = COLUMNS.get(columns.get()).and_then(|text| text.parse::<usize>().ok())
        {
            view = view.columns(count);
        }
        let picker: AnyView = if narrow {
            Box::new(
                Text::new("Na tela estreita cabe um mês por linha.")
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
            )
        } else {
            Box::new(FormField::new("Meses por linha", SegmentedControl::new(columns.binding(), COLUMNS)))
        };
        let (back_to_today, to_next) = (year.clone(), year.clone());
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                HStack::new((
                    Badge::new("Feriado").tone(Tone::Danger),
                    Badge::new("Aniversário").tone(Tone::Accent),
                    Badge::new("Pagamento").tone(Tone::Success),
                ))
                .spacing(8.0)
                .wrap(),
                picker,
                HStack::new((
                    Button::new("Voltar a este ano")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || back_to_today.set(today().year())),
                    Button::new("Ano seguinte")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || to_next.update(|y| *y += 1)),
                ))
                .spacing(8.0)
                .wrap(),
                view,
                Text::new(format!(
                    "Ano {} · dia escolhido: {}",
                    year.get(),
                    picked
                        .get()
                        .map_or_else(|| "nenhum".to_string(), |d| format!("{:02}/{:02}", d.day(), d.month()))
                )),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
