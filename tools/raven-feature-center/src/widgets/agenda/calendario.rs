//! EventCalendar: a month with its events inside each day, the hours of a week or a day, the
//! events in a list, the year and the season, with the views the app offers and the hemisphere
//! the seasons are named from.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "EventCalendarDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Calendário de eventos";

fn day(d: u8) -> Date {
    Date::new(2026, 11, d).unwrap_or_else(|| unreachable!("a real date"))
}

/// Every public feature of EventCalendar on one page.
#[derive(Debug, Default)]
pub struct EventCalendarPage;

impl View for EventCalendarPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let month = cx.use_state(|| day(15));
        let chosen: State<Option<Date>> = cx.use_state(|| Some(day(15)));
        let view = cx.use_state(|| CalendarView::Season);
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_owned());
        let north = cx.use_state(|| false);
        let (opened, created, moved) = (said.clone(), said.clone(), said.clone());
        let flip = north.clone();
        let at = |d: u8, hour: u32| TimePoint::at(day(d), hour, 0);
        let calendar = EventCalendar::new(month.binding(), chosen.binding())
            .today(day(15))
            .views(CalendarView::ALL)
            .view(view.binding())
            .hemisphere(if north.get() { Hemisphere::Northern } else { Hemisphere::Southern })
            .events_per_day(2)
            .hours(8, 18)
            .events([
                CalendarEvent::new(1, day(15), "Ensaio de gestante").tone(Tone::Accent),
                CalendarEvent::timed(2, at(18, 9), at(18, 10), "Café com cliente"),
                CalendarEvent::timed(3, at(18, 14), at(18, 16), "Entrega de álbum").tone(Tone::Success),
                CalendarEvent::new(4, day(20), "Revelação").done(true),
                CalendarEvent::new(5, day(20), "Aluguel").tone(Tone::Warning).locked(true),
                CalendarEvent::new(6, day(20), "Luz"),
            ])
            .on_event(move |id| opened.set(format!("Aberto o evento {id}")))
            .on_create(move |start, _| created.set(format!("Novo evento às {:02}h", start.hour_minute().0)))
            .on_move(move |id, start, _| {
                moved.set(format!("Evento {id} para as {:02}h", start.hour_minute().0))
            });
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Mês, semana, dia, agenda, ano e estação; o app escolhe as visões e o hemisfério das estações.")
                    .foreground(Semantic::InkMuted),
                Button::new(if north.get() { "Estações do norte" } else { "Estações do sul" })
                    .style(ButtonStyle::Secondary)
                    .on_click(move || flip.set(!flip.get())),
                calendar,
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(16.0)
            .align(Align::Stretch),
        )
    }
}
