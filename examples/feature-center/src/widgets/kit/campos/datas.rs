//! Dates and times, every feature (#579): a meeting with its day and time in steps of 15
//! minutes, the hours of a shop in steps of 30, and the same fields only to read or off.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "DateTimeDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Datas e horas";

/// A day of 2026; the days asked for here all exist.
fn day(month: u8, day: u8) -> Date {
    Date::new(2026, month, day).unwrap_or_else(|| Date::from_days(0))
}

fn shown(at: Option<DateTime>) -> String {
    at.map_or_else(|| "sem data".to_string(), |at| format!("{} às {}", at.date, at.time))
}

fn hour(at: Option<Time>) -> String {
    at.map_or_else(|| "--:--".to_string(), |t| t.to_string())
}

/// The page: a meeting, the hours of a shop and a line with what is chosen.
#[derive(Debug, Default)]
pub struct DatesPage;

impl View for DatesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let november = day(11, 1);
        let meeting =
            cx.use_state(|| Some(DateTime { date: day(11, 20), time: Time::new(9, 30).unwrap_or_default() }));
        let opens = cx.use_state(|| Time::new(8, 0));
        let closes = cx.use_state(|| Time::new(18, 0));
        let width = if narrow { 120.0 } else { 140.0 };
        let fixed = Some(DateTime { date: day(12, 24), time: Time::new(20, 0).unwrap_or_default() });

        let meetings = VStack::new((
            Text::new("Reunião").style(TextStyle::Headline),
            DateTimePicker::new(meeting.binding(), november).label("Reunião").step_minutes(15),
            DateTimePicker::new(Binding::constant(fixed), november)
                .label("Ceia, só para ler")
                .read_only(true),
            DateTimePicker::new(Binding::constant(None), november).label("Retorno, desligado").disabled(true),
        ))
        .spacing(8.0)
        .align(Align::Start);
        let shop = VStack::new((
            Text::new("Horário da loja").style(TextStyle::Headline),
            HStack::new((
                TimePicker::new(opens.binding()).label("Abre às").step_minutes(30).width(width),
                TimePicker::new(closes.binding()).label("Fecha às").step_minutes(30).width(width),
            ))
            .spacing(12.0)
            .wrap(),
            HStack::new((
                TimePicker::new(Binding::constant(Time::new(12, 0)))
                    .label("Almoço, só para ler")
                    .read_only(true),
                TimePicker::new(Binding::constant(None)).label("Plantão, desligado").disabled(true),
            ))
            .spacing(12.0)
            .wrap(),
        ))
        .spacing(8.0)
        .align(Align::Start);
        let line = format!(
            "Reunião: {} · loja das {} às {}",
            shown(meeting.get()),
            hour(opens.get()),
            hour(closes.get())
        );
        cx.child(
            &VStack::new((meetings, shop, Text::new(line)))
                .spacing(16.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
