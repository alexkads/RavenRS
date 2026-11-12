//! The pickers and forms page.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, first_of_plan, page_header};

impl Gallery {
    pub(crate) fn pickers(&self, width: f32) -> impl View + use<> {
        let otp_toasts = self.toasts.clone();
        let submit = self.submitted.clone();
        let (email, company) = (self.email.clone(), self.company.clone());
        let form_toasts = self.toasts.clone();
        let today = Date::new(2026, 10, 31).unwrap_or_else(|| unreachable!());
        let show = self.submitted.get();
        VStack::new((
            page_header("Pickers & forms", "Dates, searchable selects, one-time codes and validated forms."),
            Grid::new(
                columns(width, 2),
                (
                    demo(
                        "Calendar",
                        "Pick a day; the ring marks today.",
                        Calendar::new(self.date.binding(), today).today(today),
                    ),
                    demo(
                        "Event calendar",
                        "A month with what is on each day.",
                        EventCalendar::new(self.agenda_month.binding(), self.date.binding()).today(today).events([
                            CalendarEvent::new(1, first_of_plan().add_days(3), "Rent").done(true),
                            CalendarEvent::new(2, first_of_plan().add_days(9), "Review").tone(Tone::Warning),
                            CalendarEvent::new(3, first_of_plan().add_days(14), "Salary").tone(Tone::Success),
                        ]),
                    ),
                    demo(
                        "Scheduler",
                        "Drag events along the time and between rows, pull their ends, draw on an empty stretch, zoom.",
                        {
                            let (moved, drawn) = (self.plan_events.clone(), self.plan_events.clone());
                            VStack::new((Scheduler::new(self.plan_axis.binding())
                                .resources([
                                    Resource::new(1, "Ana").subtitle("Design").group("Team"),
                                    Resource::new(2, "Bruno").subtitle("Engineering").group("Team"),
                                    Resource::new(3, "Meeting room").group("Rooms"),
                                ])
                                .events(self.plan_events.get())
                                .on_change(move |change| {
                                    moved.update(|events| {
                                        if let Some(event) = events.iter_mut().find(|e| Some(e.id) == change.id) {
                                            event.resource = change.resource;
                                            event.start = change.start;
                                            event.end = change.end;
                                        }
                                    });
                                })
                                .on_create(move |change| {
                                    drawn.update(|events| {
                                        let id = events.len() as u64 + 1;
                                        events.push(SchedulerEvent::new(
                                            id,
                                            change.resource,
                                            change.start,
                                            change.end,
                                            "New event",
                                        ));
                                    });
                                }),))
                            .align(Align::Stretch)
                            .height(380.0)
                        },
                    ),
                    demo(
                        "Recurrence editor",
                        "How often, on which days, until when; it says the result in words.",
                        RecurrenceEditor::new(self.plan_rule.binding(), first_of_plan()),
                    ),
                    demo(
                        "Date picker and combobox",
                        "A calendar in a popover, and a select you can type into.",
                        VStack::new((
                            DatePicker::new(self.date.binding(), today).today(today).label("Date"),
                            Combobox::new(
                                self.country.binding(),
                                [
                                    "Argentina",
                                    "Brazil",
                                    "Canada",
                                    "Chile",
                                    "Germany",
                                    "Japan",
                                    "Mexico",
                                    "Portugal",
                                    "Spain",
                                    "United States",
                                ],
                            )
                            .placeholder("Choose a country")
                            .label("Country"),
                            Text::bound(&self.date, |d| {
                                d.map_or("No date selected".into(), |d| format!("Selected: {d}"))
                            })
                            .style(TextStyle::Callout)
                            .foreground(Semantic::InkMuted),
                        ))
                        .spacing(12.0),
                    ),
                    demo(
                        "One-time code",
                        "Digits only; completes automatically.",
                        InputOtp::new(self.otp.binding(), 6).on_complete(move |code| {
                            otp_toasts.show("Code accepted", format!("Verified {code}."), Tone::Success)
                        }),
                    ),
                    demo(
                        "Form with validation",
                        "Errors appear as you type, or for every field after submitting.",
                        VStack::new((
                            FormField::new(
                                "Company",
                                TextField::new(self.company.binding()).placeholder("Northwind Labs"),
                            )
                            .required()
                            .validate(self.company.binding(), validate::min_length(3))
                            .show_errors(show),
                            FormField::new(
                                "Work email",
                                TextField::new(self.email.binding())
                                    .placeholder("you@company.com")
                                    .icon(IconName::Mail),
                            )
                            .required()
                            .hint("We send the invoice here.")
                            .validate(self.email.binding(), validate::email)
                            .show_errors(show),
                            Button::new("Create account").on_click(move || {
                                submit.set(true);
                                let ok = validate::required(&company.get()).is_ok()
                                    && validate::min_length(3)(&company.get()).is_ok()
                                    && validate::required(&email.get()).is_ok()
                                    && validate::email(&email.get()).is_ok();
                                if ok {
                                    form_toasts.show("Account created", "Welcome aboard.", Tone::Success);
                                }
                            }),
                        ))
                        .spacing(16.0)
                        .align(Align::Stretch),
                    ),
                ),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}
