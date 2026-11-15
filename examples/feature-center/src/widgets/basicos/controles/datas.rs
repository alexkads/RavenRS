//! `Calendar`, `DatePicker` and `DateRangePicker`, every feature: a booking whose days before today
//! cannot be picked, the due date of a bill, a report period with ready-made ranges, and fields
//! that are off or only read. A line says what was picked.

use std::rc::Rc;

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "DatesDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Datas";

/// The day the page treats as today, fixed so that the page reads the same every day.
pub fn today() -> Date {
    Date::new(2026, 11, 15).unwrap_or_else(|| Date::from_days(0))
}

/// A day as people in Brazil write it.
fn br(day: Date) -> String {
    format!("{:02}/{:02}/{}", day.day(), day.month(), day.year())
}

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// The page: a calendar, the pickers, and a line with what was picked.
#[derive(Debug, Default)]
pub struct DatesPage;

impl View for DatesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let today = today();
        let said = cx.use_state(|| "Escolha um dia no calendário.".to_string());
        let booked = cx.use_state(|| None::<Date>);
        let due = cx.use_state(|| None::<Date>);
        let issued = cx.use_state(|| Some(today.add_days(-10)));
        let period = cx.use_state(|| None::<DateRange>);
        let closed = cx.use_state(|| {
            Some(DateRange::new(today.add_months(-1).first_of_month(), today.first_of_month().add_days(-1)))
        });
        let width = if narrow { 240.0 } else { 340.0 };

        let picked = said.clone();
        let calendar = Calendar::new(booked.binding(), today)
            .today(today)
            .range(Some(today), Some(today.add_days(60)))
            .on_pick(Rc::new(move |day: Date| picked.set(format!("Reserva para {}", br(day)))));

        let single = VStack::new((
            DatePicker::new(due.binding(), today)
                .label("Vencimento")
                .placeholder("Escolha o vencimento")
                .today(today)
                .width(if narrow { 200.0 } else { 220.0 }),
            VStack::new((DatePicker::new(due.binding(), today)
                .label("Vencimento, largura do formulário")
                .fill(),))
            .width(width)
            .align(Align::Stretch),
            DatePicker::new(issued.binding(), today).label("Emissão (só leitura)").read_only(true),
            DatePicker::new(issued.binding(), today).label("Entrega (desligada)").disabled(true),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let ranges = VStack::new((
            DateRangePicker::new(period.binding(), today)
                .label("Período do relatório")
                .placeholder("Escolha o período")
                .preset("Últimos 7 dias", DateRange::new(today.add_days(-6), today))
                .preset("Últimos 30 dias", DateRange::new(today.add_days(-29), today))
                .preset("Este mês", DateRange::new(today.first_of_month(), today))
                .today(today)
                .range(Some(today.add_months(-12)), Some(today))
                .width(width),
            DateRangePicker::new(closed.binding(), today)
                .label("Mês fechado (só leitura)")
                .read_only(true)
                .width(width),
            DateRangePicker::new(closed.binding(), today)
                .label("Período anterior (desligado)")
                .disabled(true)
                .width(width),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let summary = format!(
            "Reserva: {} · vencimento: {} · período: {}",
            booked.get().map_or("nenhuma".to_string(), br),
            due.get().map_or("nenhum".to_string(), br),
            period.get().map_or("nenhum".to_string(), |r| format!(
                "{} a {} ({} dias)",
                br(r.start),
                br(r.end),
                r.days()
            )),
        );

        cx.child(
            &VStack::new((
                VStack::new((heading("Calendário"), calendar)).spacing(8.0).align(Align::Start),
                VStack::new((heading("Um dia"), single)).spacing(8.0),
                VStack::new((heading("Um período"), ranges)).spacing(8.0),
                Text::new(summary),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
