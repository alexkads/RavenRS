//! Dates, times, steps and files.

use ravenrs::prelude::*;

samples! {
    "DatePicker" => date_picker, "Calendar" => calendar, "EventCalendar" => event_calendar,
    "Scheduler" => scheduler, "RecurrenceEditor" => recurrence_editor, "TimePicker" => time_picker,
    "DateTimePicker" => date_time_picker, "DateRangePicker" => date_range_picker, "Wizard" => wizard,
    "StepIndicator" => step_indicator, "StepBadge" => step_badge, "FileUpload" => file_upload,
    "ImageField" => image_field, "DropZone" => drop_zone, "YearView" => year_view,
}

// sample: TimePicker, 90
fn time_picker() -> impl View {
    let at = State::new(Time::new(9, 30));
    TimePicker::new(at.binding()).step_minutes(15).label("Starts at")
}
// end

// sample: DateTimePicker, 90
fn date_time_picker() -> impl View {
    let month = Date::new(2026, 11, 1).unwrap_or_else(|| Date::from_days(0));
    let at = State::new(Time::new(14, 0).map(|time| DateTime { date: month, time }));
    DateTimePicker::new(at.binding(), month).label("Starts")
}
// end

// sample: DateRangePicker, 420
fn date_range_picker() -> impl View {
    let today = Date::new(2026, 11, 2).unwrap_or_else(|| unreachable!("a valid date"));
    let period = State::new(Some(DateRange::new(today, today.add_days(6))));
    DateRangePicker::new(period.binding(), today)
        .today(today)
        .preset("Last 7 days", DateRange::new(today.add_days(-6), today))
        .preset("Last 30 days", DateRange::new(today.add_days(-29), today))
        .label("Period")
}
// end

// sample: StepIndicator, 180
fn step_indicator() -> impl View {
    StepIndicator::new(vec![
        IndicatorStep::new("Client", StepState::Done),
        IndicatorStep::new("Photos", StepState::Error).note("Two are missing"),
        IndicatorStep::new("Review", StepState::Current),
        IndicatorStep::new("Send", StepState::Upcoming),
    ])
    .vertical()
    .on_select(|_step| {})
}
// end

// sample: StepBadge, 80
fn step_badge() -> impl View {
    HStack::new((
        StepBadge::new(1).state(StepState::Done),
        StepBadge::new(2).state(StepState::Error),
        StepBadge::new(3).state(StepState::Current),
        StepBadge::new(4),
    ))
    .spacing(8.0)
}
// end

// sample: Wizard, 280
fn wizard() -> impl View {
    let step = State::new(0);
    let name = State::new(String::new());
    let filled = name.clone();
    Wizard::new(
        step.binding(),
        vec![
            WizardStep::new("Account", FormField::new("Name", TextField::new(name.binding()))).check(
                move || {
                    if filled.get().trim().is_empty() { Err("Fill in the name".to_owned()) } else { Ok(()) }
                },
            ),
            WizardStep::new("Opening balance", Text::new("How much it holds today")),
            WizardStep::new("Review", Text::new("Everything is ready")),
        ],
    )
    .on_finish(|| {})
}
// end

// sample: FileUpload, 240
fn file_upload() -> impl View {
    let files = State::new(Vec::<PickedFile>::new());
    FileUpload::new(files.binding()).accept(["pdf", "png", "jpg"]).most_files(5).label("Receipts")
}
// end

// sample: ImageField, 220
fn image_field() -> impl View {
    let photo = State::new(None::<PickedFile>);
    ImageField::new(photo.binding()).size(180.0, 120.0).label("Photo")
}
// end

// sample: DatePicker, 420
fn date_picker() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(None);
    DatePicker::new(date.binding(), today).today(today).label("Date")
}
// end

// sample: Calendar, 380
fn calendar() -> impl View {
    let today = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let date = State::new(Some(today));
    Calendar::new(date.binding(), today).today(today)
}
// end

// sample: EventCalendar, 620
fn event_calendar() -> impl View {
    let today = Date::new(2026, 11, 16).unwrap_or_else(|| unreachable!("a real date"));
    let (month, day) = (State::new(today.first_of_month()), State::new(Some(today)));
    EventCalendar::new(month.binding(), day.binding()).today(today).events([
        CalendarEvent::new(1, today.add_days(-4), "Rent").done(true),
        CalendarEvent::new(2, today, "Review").tone(Tone::Warning),
        CalendarEvent::new(3, today.add_days(3), "Salary").tone(Tone::Success),
    ])
}
// end

// sample: Scheduler, 400
fn scheduler() -> impl View {
    let first = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
    let at = |day: i64| TimePoint::from_date(first).plus_days(day);
    let axis = State::new(TimeAxis::days(first, 30));
    Scheduler::new(axis.binding())
        .resources([Resource::new(1, "Ana"), Resource::new(2, "Bruno"), Resource::new(3, "Carla")])
        .events([
            SchedulerEvent::new(1, 1, at(1), at(5), "Vacation").tone(Tone::Success),
            SchedulerEvent::new(2, 1, at(3), at(8), "Workshop"),
            SchedulerEvent::new(3, 2, at(2), at(6), "On call").tone(Tone::Warning),
        ])
        .on_change(|change| println!("{change:?}"))
}
// end

// sample: RecurrenceEditor, 280
fn recurrence_editor() -> impl View {
    let start = Date::new(2026, 11, 5).unwrap_or_else(|| unreachable!("a real date"));
    let rule = State::new(RecurrenceRule::new(Frequency::Weekly).on_weekdays([1, 4]));
    RecurrenceEditor::new(rule.binding(), start)
}
// end

// sample: YearView, 700
fn year_view() -> impl View {
    let year = State::new(2026);
    let chosen = State::new(None::<Date>);
    YearView::new(year.binding())
        .selected(chosen.binding())
        .today(Date::new(2026, 11, 5).unwrap_or_else(|| Date::from_days(0)))
        .marks(|day| match day.day() % 11 {
            0 => Some(DayMark::new().tone(Tone::Accent).label("Shoot")),
            5 => Some(DayMark::new().tone(Tone::Success).label("Delivery")),
            _ => None,
        })
}
// end

// sample: DropZone, 140
fn drop_zone() -> impl View {
    let count = State::new(0_usize);
    let (shown, add) = (count.clone(), count);
    DropZone::new(
        Text::bound(&shown, |n| format!("{n} files received. Drop more, or press Enter here."))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    )
    .label("Files of the session")
    .min_height(96.0)
    .choosable()
    .on_files(move |files| add.update(|n| *n += files.len()))
}
// end
