//! Calendar, DatePicker, DateRangePicker, TimePicker and DateTimePicker: a day, a period and a
//! time, with limits, blocked days and why, ready-made periods, and the states that keep them.
//! DateTile: a day in a small block, in its tones and sizes, and outlined over a band.
use std::rc::Rc;

use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "DatesDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Datas e horas";

fn day(d: u8) -> Date {
    Date::new(2026, 11, d).unwrap_or_else(|| unreachable!("a real date"))
}

fn at(hour: u8, minute: u8) -> Time {
    Time::new(hour, minute).unwrap_or_else(|| unreachable!("a real time"))
}

/// The holiday of the page, with why it cannot be picked.
fn holiday(d: Date) -> Option<String> {
    (d == day(20)).then(|| "Feriado da Consciência Negra".to_owned())
}

/// Every public feature of the date and time pickers on one page.
#[derive(Debug, Default)]
pub struct DatesPage;

impl View for DatesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let picked = cx.use_state(|| None::<Date>);
        let delivery = cx.use_state(|| None::<Date>);
        let period = cx.use_state(|| None::<DateRange>);
        let start = cx.use_state(|| Some(at(9, 0)));
        let meeting = cx.use_state(|| None::<DateTime>);
        let said = cx.use_state(|| "Nenhum dia escolhido no calendário.".to_owned());
        let tell = said.clone();
        let fmt = |d: Option<Date>| d.map_or_else(|| "nenhuma".to_owned(), |d| d.to_string());
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Um dia, um período e uma hora, com limites e dias que não dá para escolher, e o motivo.",
                )
                .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Bloco de data").style(TextStyle::Headline),
                    HStack::new((
                        DateTile::new(day(14)),
                        DateTile::new(day(15)).tone(Tone::Accent),
                        DateTile::new(day(21)).tone(Tone::Success).size(72.0),
                        HStack::new((DateTile::new(day(28)).tone(Tone::Accent).outlined(),))
                            .padding(8.0)
                            .background(Semantic::Accent),
                    ))
                    .spacing(12.0)
                    .align(Align::Center),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Calendário").style(TextStyle::Headline),
                    Calendar::new(picked.binding(), day(1))
                        .today(day(27))
                        .range(Some(day(2)), Some(day(28)))
                        .blocked(holiday)
                        .on_pick(Rc::new(move |d| tell.set(format!("Escolhido no calendário: {d}")))),
                    Text::new(said.get()),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Um dia").style(TextStyle::Headline),
                    DatePicker::new(delivery.binding(), day(1))
                        .label("Entrega")
                        .placeholder("Escolha o dia da entrega")
                        .today(day(27))
                        .range(Some(day(2)), Some(day(28)))
                        .blocked(holiday)
                        .width(260.0),
                    Text::new(format!("Entrega: {}", fmt(delivery.get()))),
                    VStack::new((DatePicker::new(delivery.binding(), day(1))
                        .label("Entrega, largura da coluna")
                        .fill(),))
                    .width(360.0)
                    .align(Align::Stretch),
                    DatePicker::new(delivery.binding(), day(1)).label("Entrega travada").read_only(true),
                    DatePicker::new(delivery.binding(), day(1)).label("Entrega desligada").disabled(true),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Um período").style(TextStyle::Headline),
                    DateRangePicker::new(period.binding(), day(1))
                        .label("Férias")
                        .placeholder("Escolha o período")
                        .today(day(27))
                        .range(Some(day(1)), None)
                        .blocked(holiday)
                        .preset("Primeira semana", DateRange::new(day(1), day(7)))
                        .width(320.0),
                    Text::new(format!(
                        "Férias: {}",
                        period
                            .get()
                            .map_or_else(|| "nenhuma".to_owned(), |r| format!("{} a {}", r.start, r.end))
                    )),
                    DateRangePicker::new(period.binding(), day(1)).label("Férias travadas").read_only(true),
                    DateRangePicker::new(period.binding(), day(1)).label("Férias desligadas").disabled(true),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Uma hora").style(TextStyle::Headline),
                    TimePicker::new(start.binding())
                        .label("Início")
                        .step_minutes(30)
                        .range(Some(at(8, 0)), Some(at(18, 0)))
                        .width(140.0),
                    Text::new(format!(
                        "Início: {}",
                        start.get().map_or_else(|| "nenhum".to_owned(), |t| t.to_string())
                    )),
                    TimePicker::new(start.binding()).label("Início travado").read_only(true),
                    TimePicker::new(start.binding()).label("Início desligado").disabled(true),
                    DateTimePicker::new(meeting.binding(), day(1)).label("Reunião").step_minutes(60).range(
                        Some(DateTime { date: day(2), time: at(9, 0) }),
                        Some(DateTime { date: day(28), time: at(17, 0) }),
                    ),
                    DateTimePicker::new(meeting.binding(), day(1)).label("Reunião travada").read_only(true),
                    DateTimePicker::new(meeting.binding(), day(1)).label("Reunião desligada").disabled(true),
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
