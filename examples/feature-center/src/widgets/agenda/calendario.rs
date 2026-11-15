//! `EventCalendar` and `RecurrenceEditor`, every feature: the bills and meetings of a household
//! by month, week and day; events with a time that move by dragging and are created on an empty
//! hour; a cap of events per day with the rest as a count; and the rule of a bill that repeats,
//! edited in place, its days showing on the calendar and its iCalendar text below.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "EventCalendarDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Calendário de eventos";

/// The id the days of the bill that repeats start from.
const SERIES: u64 = 1_000;

/// Today, fixed so the page reads the same every time.
pub fn today() -> Date {
    Date::new(2026, 11, 16).unwrap_or_else(|| unreachable!("a real date"))
}

fn day(n: i32) -> Date {
    today().first_of_month().add_days(n - 1)
}

/// The first day of the bill that repeats.
pub fn bill_start() -> Date {
    day(10)
}

/// The events the page starts with: whole days, timed ones, one locked and a crowded day.
fn first_events() -> Vec<CalendarEvent> {
    let at = |d: i32, h: u32, m: u32| TimePoint::at(day(d), h, m);
    vec![
        CalendarEvent::new(1, day(5), "Aluguel").tone(Tone::Warning).done(true),
        CalendarEvent::new(2, day(5), "Salário").tone(Tone::Success).done(true),
        CalendarEvent::new(3, day(20), "Feriado").tone(Tone::Danger).locked(true),
        CalendarEvent::timed(4, at(16, 9, 0), at(16, 10, 30), "Dentista").tone(Tone::Accent),
        CalendarEvent::timed(5, at(17, 14, 0), at(17, 15, 0), "Reunião da escola"),
        CalendarEvent::timed(6, at(18, 19, 0), at(18, 21, 0), "Aniversário da Bia").tone(Tone::Accent),
        CalendarEvent::timed(7, at(16, 12, 0), at(16, 13, 0), "Almoço com a equipe").locked(true),
        CalendarEvent::new(8, day(25), "Seguro do carro").tone(Tone::Warning),
        CalendarEvent::new(9, day(25), "Condomínio").tone(Tone::Warning),
        CalendarEvent::new(10, day(25), "Internet").tone(Tone::Warning),
        CalendarEvent::new(11, day(25), "Cartão de crédito").tone(Tone::Danger),
    ]
}

fn when(time: TimePoint) -> String {
    let (h, m) = time.hour_minute();
    let date = time.date();
    format!("{:02}/{:02} {h:02}:{m:02}", date.day(), date.month())
}

/// The page.
#[derive(Debug, Default)]
pub struct EventCalendarPage;

impl View for EventCalendarPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let month = cx.use_state(today);
        let picked = cx.use_state(|| Some(today()));
        let view = cx.use_state(|| CalendarView::Month);
        let events = cx.use_state(first_events);
        let next = cx.use_state(|| 100_u64);
        let per_day = cx.use_state(|| 3_i32);
        let working = cx.use_state(|| false);
        let rule = cx.use_state(|| RecurrenceRule::new(Frequency::Monthly).end(RecurrenceEnd::After(12)));
        let said = cx.use_state(|| "Clique num evento, troque a visão ou crie numa hora vazia.".to_string());

        // The bill's days that fall in what is shown, as the app expands a series.
        let (from, to) = view.get().range(month.get());
        let series = rule.get().occurrences(bill_start(), from, to);
        let mut shown = events.get();
        shown.extend(
            series
                .iter()
                .zip(SERIES..)
                .map(|(d, id)| CalendarEvent::new(id, *d, "Conta de luz").tone(Tone::Neutral)),
        );
        let title_of = {
            let all = shown.clone();
            move |id: u64| all.iter().find(|e| e.id == id).map(|e| e.title.clone()).unwrap_or_default()
        };
        let (event_line, event_title) = (said.clone(), title_of.clone());
        let (moved, move_line, move_title) = (events.clone(), said.clone(), title_of.clone());
        let (created, create_line, counter) = (events.clone(), said.clone(), next.clone());
        let (from_hour, to_hour) = if working.get() { (8, 18) } else { (0, 24) };
        let calendar = EventCalendar::new(month.binding(), picked.binding())
            .today(today())
            .events(shown)
            .events_per_day(usize::try_from(per_day.get()).unwrap_or(3))
            .view(view.binding())
            .hours(from_hour, to_hour)
            .on_event(move |id| event_line.set(format!("Clicou em {}", event_title(id))))
            .on_move(move |id, start, end| {
                moved.update(|all| {
                    if let Some(event) = all.iter_mut().find(|e| e.id == id) {
                        event.date = start.date();
                        event.time = Some((start, end));
                    }
                });
                move_line.set(format!("{} foi para {}", move_title(id), when(start)));
            })
            .on_create(move |start, end| {
                let id = counter.get();
                counter.set(id + 1);
                created.update(|all| all.push(CalendarEvent::timed(id, start, end, "Novo compromisso")));
                create_line.set(format!("Criou um compromisso em {} até {}", when(start), when(end)));
            });

        let (to_week, to_day) = (view.clone(), view.clone());
        let options = HStack::new((
            Button::new("Ver a semana")
                .style(ButtonStyle::Secondary)
                .on_click(move || to_week.set(CalendarView::Week)),
            Button::new("Ver o dia")
                .style(ButtonStyle::Secondary)
                .on_click(move || to_day.set(CalendarView::Day)),
            Toggle::new(working.binding()).label("Só o horário comercial"),
            FormField::new(
                "Eventos por dia",
                Stepper::new(per_day.binding()).range(1, 5).label("Eventos por dia"),
            ),
        ))
        .spacing(12.0)
        .align(Align::Center)
        .wrap();
        let shown_view = match view.get() {
            CalendarView::Week => "semana",
            CalendarView::Day => "dia",
            _ => "mês",
        };
        let left = VStack::new((
            options,
            calendar,
            Text::new(format!(
                "Visão: {shown_view} · dia escolhido: {}",
                picked
                    .get()
                    .map_or_else(|| "nenhum".to_string(), |d| format!("{:02}/{:02}", d.day(), d.month()))
            )),
        ))
        .spacing(10.0)
        .align(Align::Stretch);
        let current = rule.get();
        let right = VStack::new((
            Text::new("Conta de luz que se repete").style(TextStyle::Headline),
            Text::new(format!(
                "Primeira em {:02}/{:02}/{}",
                bill_start().day(),
                bill_start().month(),
                bill_start().year()
            ))
            .style(TextStyle::Caption)
            .foreground(Semantic::InkMuted),
            RecurrenceEditor::new(rule.binding(), bill_start()),
            Text::new(format!("{} vez(es) no que está à vista", series.len())),
            Text::new(format!("iCalendar: {}", current.to_text()))
                .style(TextStyle::Caption)
                .foreground(Semantic::InkMuted),
        ))
        .spacing(8.0)
        .align(Align::Stretch);
        let body: AnyView = if narrow {
            Box::new(VStack::new((left, right)).spacing(16.0).align(Align::Stretch))
        } else {
            Box::new(
                HStack::new((
                    VStack::new((left,)).grow(1.0).align(Align::Stretch),
                    VStack::new((right,)).width(340.0).align(Align::Stretch),
                ))
                .spacing(24.0)
                .align(Align::Start),
            )
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                body,
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
