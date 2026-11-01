//! What is ahead, on a timeline or a month: bills, and receipts and payments planned on a rule.

use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Memo, Store, heading, screen};
use crate::model::{Bill, Flow, Member, Occurrence, date_label};
use crate::money::format_brl;

/// Something on a day of the agenda.
#[derive(Clone)]
enum Item {
    Bill(Bill),
    Planned(Occurrence),
}

impl Item {
    fn day(&self) -> Date {
        match self {
            Self::Bill(bill) => bill.paid_on.unwrap_or(bill.due),
            Self::Planned(occurrence) => occurrence.when,
        }
    }

    fn title(&self) -> String {
        match self {
            Self::Bill(bill) => bill.title(),
            Self::Planned(occurrence) => occurrence.schedule.input.description.clone(),
        }
    }

    fn member(&self) -> Option<i64> {
        match self {
            Self::Bill(bill) => bill.member_id,
            Self::Planned(occurrence) => occurrence.schedule.input.member_id,
        }
    }

    fn done(&self) -> bool {
        match self {
            Self::Bill(bill) => bill.paid_on.is_some(),
            Self::Planned(occurrence) => occurrence.transaction_id.is_some(),
        }
    }

    fn cents(&self) -> i64 {
        match self {
            Self::Bill(bill) => bill.amount_cents,
            Self::Planned(occurrence) => occurrence.schedule.input.amount_cents,
        }
    }

    fn incoming(&self) -> bool {
        matches!(self, Self::Planned(o) if o.schedule.input.flow == Flow::Income)
    }

    /// Late things are red, what comes in is green, what goes out stands out without alarm.
    fn tone(&self, today: Date) -> Tone {
        if !self.done() && self.day() < today {
            Tone::Danger
        } else if self.incoming() {
            Tone::Success
        } else {
            Tone::Accent
        }
    }

    /// Said in words, since color alone does not.
    fn status(&self, today: Date) -> &'static str {
        match self {
            Self::Bill(bill) => bill.status(today).label(),
            Self::Planned(_) if self.done() => "Confirmado",
            Self::Planned(_) if self.day() < today => "Atrasado",
            Self::Planned(_) if self.incoming() => "A receber",
            Self::Planned(_) => "A pagar",
        }
    }
}

#[derive(Default)]
struct AgendaData {
    items: Vec<Item>,
    members: Vec<Member>,
}

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct AgendaState {
    view: State<usize>,
    axis: State<TimeAxis>,
    month: State<Date>,
    day: State<Option<Date>>,
    data: Memo<(u64, Date, Date), Rc<AgendaData>>,
}

impl AgendaState {
    pub fn new(today: Date) -> Self {
        Self {
            view: State::new(0),
            // Wide enough to read what a one-day bar says.
            axis: State::new(TimeAxis { tick_width: 80.0, ..TimeAxis::days(today.first_of_month(), 62) }),
            month: State::new(today.first_of_month()),
            day: State::new(Some(today)),
            data: Memo::new(),
        }
    }
}

fn load(store: &Store, from: Date, to: Date) -> AgendaData {
    let mut items: Vec<Item> = store
        .read(|db| db.bills())
        .into_iter()
        .filter(|bill| {
            let day = bill.paid_on.unwrap_or(bill.due);
            day >= from && day <= to
        })
        .map(Item::Bill)
        .collect();
    items.extend(store.read(|db| db.occurrences(from, to)).into_iter().map(Item::Planned));
    items.sort_by_key(Item::day);
    AgendaData { items, members: store.read(|db| db.members()) }
}

/// The row of someone; the family as a whole has the first row.
fn row_of(member: Option<i64>) -> u64 {
    member.map_or(0, |id| id as u64)
}

fn member_of(row: u64) -> Option<i64> {
    (row != 0).then_some(row as i64)
}

/// Places a change from the timeline in the database.
fn apply_move(store: &Store, item: &Item, change: SchedulerChange) {
    let day = change.start.date();
    match item {
        Item::Bill(bill) => {
            store.write("Conta remarcada", |db| db.move_bill(bill.id, day, member_of(change.resource)));
        }
        Item::Planned(occurrence) => {
            store
                .write("Dia remarcado", |db| db.move_occurrence(occurrence.schedule_id, occurrence.day, day));
        }
    }
}

fn item_actions(item: &Item, store: &Store, forms: &Forms) -> Vec<AnyView> {
    let mut actions: Vec<AnyView> = Vec::new();
    let title = item.title();
    match item {
        Item::Bill(bill) if bill.paid_on.is_none() => {
            let (id, store, pay) = (bill.id, store.clone(), forms.pay.clone());
            actions.push(Box::new(
                Button::new("Pagar")
                    .accessible_name(format!("Pagar {title}"))
                    .size(ButtonSize::Small)
                    .on_click(move || pay.start(&store, id)),
            ));
        }
        Item::Bill(_) => {}
        Item::Planned(occurrence) => {
            if let Some(transaction) = occurrence.transaction_id {
                let (store, confirm) = (store.clone(), forms.confirm.clone());
                actions.push(Box::new(
                    Button::new("Desfazer")
                        .accessible_name(format!("Desfazer {title}"))
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            let store = store.clone();
                            confirm.ask(
                                "Desfazer esta confirmação?",
                                "O lançamento é apagado e o dia volta a ficar previsto.",
                                "Desfazer",
                                move || {
                                    store.write("Confirmação desfeita", |db| {
                                        db.undo_occurrence(transaction)
                                    });
                                },
                            );
                        }),
                ));
            } else {
                let (form_store, form, found) = (store.clone(), forms.occurrence.clone(), occurrence.clone());
                actions.push(Box::new(
                    Button::new("Confirmar")
                        .accessible_name(format!("Confirmar {title}"))
                        .size(ButtonSize::Small)
                        .on_click(move || form.start(&form_store, &found)),
                ));
                let (skip_store, id, day) = (store.clone(), occurrence.schedule_id, occurrence.day);
                actions.push(Box::new(
                    Button::new("Pular")
                        .accessible_name(format!("Pular {title}"))
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || {
                            skip_store.write("Dia pulado", |db| db.skip_occurrence(id, day));
                        }),
                ));
            }
            let (edit_store, edit, id) = (store.clone(), forms.schedule.clone(), occurrence.schedule_id);
            actions.push(Box::new(
                IconButton::new(IconName::Pencil)
                    .label(format!("Alterar programação de {title}"))
                    .size(ButtonSize::Small)
                    .on_click(move || edit.start_edit(&edit_store, id)),
            ));
            let (stop_store, stop, id) = (store.clone(), forms.confirm.clone(), occurrence.schedule_id);
            actions.push(Box::new(
                IconButton::new(IconName::Trash2)
                    .label(format!("Encerrar programação de {title}"))
                    .size(ButtonSize::Small)
                    .on_click(move || {
                        let store = stop_store.clone();
                        stop.ask(
                            "Encerrar esta programação?",
                            "Os dias futuros somem. O que já foi confirmado continua nos lançamentos.",
                            "Encerrar",
                            move || {
                                store.write("Programação encerrada", |db| db.delete_schedule(id));
                            },
                        );
                    }),
            ));
        }
    }
    actions
}

/// The chosen day, on the side of the timeline or the month.
fn day_panel(data: &AgendaData, day: Date, store: &Store, forms: &Forms) -> impl View + use<> {
    let rows: Vec<AnyView> = data
        .items
        .iter()
        .filter(|item| item.day() == day)
        .map(|item| {
            let sign = if item.incoming() { "+" } else { "-" };
            Box::new(
                VStack::new((
                    Text::new(item.title()).style(TextStyle::Body).truncate(),
                    HStack::new((
                        Badge::new(item.status(store.today)).tone(item.tone(store.today)),
                        Text::new(format!("{sign}{}", format_brl(item.cents()))).style(TextStyle::Callout),
                    ))
                    .spacing(8.0)
                    .align(Align::Center),
                    HStack::new(item_actions(item, store, forms)).spacing(6.0).align(Align::Center),
                ))
                .spacing(6.0)
                .align(Align::Stretch),
            ) as AnyView
        })
        .collect();
    let empty = rows.is_empty();
    Card::new(
        ScrollView::new((VStack::new((
            Text::new(date_label(day)).style(TextStyle::Headline),
            empty.then(|| {
                Text::new("Nada neste dia.").style(TextStyle::Callout).foreground(Semantic::InkMuted)
            }),
            VStack::new(rows).spacing(16.0).align(Align::Stretch),
        ))
        .spacing(12.0)
        .align(Align::Stretch),))
        .spacing(0.0),
    )
    .padding(16.0)
    .width(300.0)
}

pub(crate) fn agenda(store: &Store, state: &AgendaState, forms: &Forms) -> impl View + use<> {
    let timeline = state.view.get() == 0;
    let axis = state.axis.get();
    let month = state.month.get().first_of_month();
    let (from, to) = if timeline {
        (axis.start.date(), axis.end.plus_minutes(-1).date())
    } else {
        // The weeks of a month reach into the months around it.
        (month.add_days(-7), month.add_months(1).add_days(7))
    };
    let data = state.data.get((store.version(), from, to), || Rc::new(load(store, from, to)));
    let today = store.today;
    let day = state.day.get().unwrap_or(today);
    let (incoming, outgoing) = data.items.iter().filter(|item| !item.done()).fold((0, 0), |(i, o), item| {
        if item.incoming() { (i + item.cents(), o) } else { (i, o + item.cents()) }
    });

    let (plan_store, plan_form, plan_day) = (store.clone(), forms.schedule.clone(), state.day.clone());
    let toolbar = HStack::new((
        SegmentedControl::with_icons(
            state.view.binding(),
            [(IconName::Rows3, "Linha do tempo"), (IconName::CalendarDays, "Mês")],
        )
        .label("Exibição"),
        Text::new(format!("Previsto aqui: entra {} · sai {}", format_brl(incoming), format_brl(outgoing)))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
        Spacer::new(),
        Button::new("Programar").icon(IconName::Plus).on_click(move || {
            let start = plan_day.get().unwrap_or(plan_store.today);
            plan_form.start_new(&plan_store, start, None);
        }),
    ))
    .spacing(12.0)
    .align(Align::Center);

    let main: AnyView = if timeline {
        let items = Rc::new(data.items.clone());
        let resources = std::iter::once(Resource::new(0, "Família toda"))
            .chain(data.members.iter().map(|m| Resource::new(m.id as u64, m.name.clone())));
        // A day of the agenda has no length to change, so bars only move.
        let events = data.items.iter().enumerate().map(|(i, item)| {
            let start = TimePoint::from_date(item.day());
            SchedulerEvent::new(i as u64, row_of(item.member()), start, start.plus_days(1), item.title())
                .tone(item.tone(today))
                .done(item.done())
                .locked(item.done())
                .resizable(false)
        });
        let (change_store, change_items) = (store.clone(), items.clone());
        let rule_items = items.clone();
        let (pick_items, pick_day) = (items.clone(), state.day.clone());
        let selected = {
            let (read, write) = (state.day.clone(), pick_day.clone());
            let finder = pick_items.clone();
            Binding::new(
                move || {
                    let day = read.get();
                    finder.iter().position(|item| Some(item.day()) == day).map(|i| i as u64)
                },
                move |id: Option<u64>| {
                    if let Some(item) = id.and_then(|id| pick_items.get(id as usize)) {
                        write.set(Some(item.day()));
                    }
                },
            )
        };
        let (open_store, open_forms, open_items) = (store.clone(), forms.clone(), items.clone());
        let (new_store, new_form, new_day) = (store.clone(), forms.schedule.clone(), state.day.clone());
        Box::new(
            Scheduler::new(state.axis.binding())
                .resources(resources)
                .events(events)
                .now(TimePoint::from_date(today))
                .resource_width(150.0)
                .selection(selected)
                .can_drop(move |change| {
                    // Changing whose it is belongs to the whole plan, so a single day stays on its row.
                    change.id.and_then(|id| rule_items.get(id as usize)).is_none_or(|item| match item {
                        Item::Bill(_) => true,
                        Item::Planned(_) => change.resource == row_of(item.member()),
                    })
                })
                .on_change(move |change| {
                    if let Some(item) = change.id.and_then(|id| change_items.get(id as usize)) {
                        apply_move(&change_store, item, change);
                    }
                })
                .on_create(move |change| {
                    let start = change.start.date();
                    new_day.set(Some(start));
                    new_form.start_new(&new_store, start, member_of(change.resource));
                })
                .on_open(move |id| {
                    let Some(item) = open_items.get(id as usize) else { return };
                    match item {
                        Item::Bill(bill) if bill.paid_on.is_none() => {
                            open_forms.bill.start_edit(&open_store, bill.id)
                        }
                        Item::Bill(_) => {}
                        Item::Planned(o) => open_forms.schedule.start_edit(&open_store, o.schedule_id),
                    }
                }),
        )
    } else {
        let events: Vec<CalendarEvent> = data
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                CalendarEvent::new(i as u64, item.day(), item.title())
                    .tone(item.tone(today))
                    .done(item.done())
            })
            .collect();
        Box::new(ScrollView::new((EventCalendar::new(state.month.binding(), state.day.binding())
            .today(today)
            .events(events),)))
    };

    screen((
        heading(
            "Agenda",
            "Arraste para remarcar. Arraste em um espaço vazio para programar um recebimento ou um pagamento.",
        ),
        toolbar,
        Fill2 { main, side: Box::new(day_panel(&data, day, store, forms)) },
    ))
}

/// The timeline or the month, taking the room left, beside the chosen day.
struct Fill2 {
    main: AnyView,
    side: AnyView,
}

impl View for Fill2 {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let main = cx.child(self.main.as_ref());
        let side = cx.child(self.side.as_ref());
        cx.container(
            ravenrs::layout::LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                spacing: 16.0,
                align: Align::Stretch,
                overflow_hidden: true,
                ..ravenrs::layout::LayoutStyle::stack(ravenrs::layout::Axis::Horizontal)
            },
            &[main, side],
            ravenrs::widgets::Paint::None,
        )
    }
}
