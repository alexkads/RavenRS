//! What is ahead, on a timeline or a month: bills, and receipts and payments planned on a rule.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs::core::Key;
use ravenrs::prelude::*;

use super::forms::Forms;
use super::help;
use super::store::{Memo, Page, Store, content_width, screen};
use crate::model::{Bill, Flow, Member, Occurrence, date_label};
use crate::money::format_brl;

const FILTER_WIDTH: f32 = 220.0;
/// Below this width the chosen day goes under the timeline.
const SIDE_BY_SIDE_FROM: f32 = 860.0;
const DAY_PANEL_WIDTH: f32 = 300.0;
const PANEL_GAP: f32 = 16.0;
const DAY_PANEL_HEIGHT: f32 = 170.0;
const RESOURCE_WIDTH: f32 = 170.0;
/// How many days the timeline shows, unless someone zooms.
const DAYS_IN_VIEW: f32 = 7.0;
const FIRST_DAY_WIDTH: f32 = 96.0;
/// A day narrower than this has no room for an amount.
const NARROWEST_DAY: f32 = 84.0;

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
    /// Only what has this in its name shows on the timeline and the month.
    filter: State<String>,
    history: MoveHistory,
    /// The width of a day last fitted to the window. Once zoom changes it, it stays.
    fitted: Rc<Cell<f32>>,
    data: Memo<(u64, Date, Date), Rc<AgendaData>>,
}

impl AgendaState {
    pub fn new(today: Date) -> Self {
        Self {
            view: State::new(0),
            // Wide enough to read what a one-day bar says.
            axis: State::new(TimeAxis {
                tick_width: FIRST_DAY_WIDTH,
                ..TimeAxis::days(today.first_of_month(), 62)
            }),
            month: State::new(today.first_of_month()),
            day: State::new(Some(today)),
            filter: State::new(String::new()),
            history: MoveHistory::new(),
            fitted: Rc::new(Cell::new(FIRST_DAY_WIDTH)),
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

/// A move on the timeline, which can be taken back.
#[derive(Clone, Copy)]
enum Moved {
    Bill { id: i64, from: (Date, Option<i64>), to: (Date, Option<i64>) },
    Planned { schedule: i64, day: Date, from: Date, to: Date },
}

impl Moved {
    /// The same move the other way.
    fn reversed(self) -> Self {
        match self {
            Self::Bill { id, from, to } => Self::Bill { id, from: to, to: from },
            Self::Planned { schedule, day, from, to } => Self::Planned { schedule, day, from: to, to: from },
        }
    }

    fn apply(self, store: &Store, done: &str) -> bool {
        match self {
            Self::Bill { id, to, .. } => store.write(done, |db| db.move_bill(id, to.0, to.1)),
            Self::Planned { schedule, day, to, .. } => {
                store.write(done, |db| db.move_occurrence(schedule, day, to))
            }
        }
        .is_some()
    }
}

/// The moves that can be undone and those that can be done again.
#[derive(Clone)]
struct MoveHistory {
    undo: State<Vec<Moved>>,
    redo: State<Vec<Moved>>,
}

impl MoveHistory {
    fn new() -> Self {
        Self { undo: State::new(Vec::new()), redo: State::new(Vec::new()) }
    }

    /// Makes a new move, after which nothing undone can be done again.
    fn make(&self, store: &Store, moved: Moved, done: &str) {
        if moved.apply(store, done) {
            self.undo.update(|moves| moves.push(moved));
            self.redo.set(Vec::new());
        }
    }

    fn undo(&self, store: &Store) {
        let Some(moved) = self.undo.get().last().copied() else { return };
        if moved.reversed().apply(store, "Remarcação desfeita") {
            self.undo.update(|moves| {
                moves.pop();
            });
            self.redo.update(|moves| moves.push(moved));
        }
    }

    fn redo(&self, store: &Store) {
        let Some(moved) = self.redo.get().last().copied() else { return };
        if moved.apply(store, "Remarcação refeita") {
            self.redo.update(|moves| {
                moves.pop();
            });
            self.undo.update(|moves| moves.push(moved));
        }
    }
}

/// Undo and redo from the keyboard, on this screen.
struct HistoryKeys {
    history: MoveHistory,
    store: Store,
}

impl View for HistoryKeys {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let z = || Key::Character("z".into());
        let (redo, redo_store) = (self.history.clone(), self.store.clone());
        cx.shortcut_with_shift(z(), true, Rc::new(move || redo.redo(&redo_store)));
        let (again, again_store) = (self.history.clone(), self.store.clone());
        cx.shortcut(Key::Character("y".into()), true, Rc::new(move || again.redo(&again_store)));
        let (undo, undo_store) = (self.history.clone(), self.store.clone());
        cx.shortcut(z(), true, Rc::new(move || undo.undo(&undo_store)));
        cx.empty()
    }
}

/// Places a change from the timeline in the database.
fn apply_move(store: &Store, history: &MoveHistory, item: &Item, change: SchedulerChange) {
    let day = change.start.date();
    match item {
        Item::Bill(bill) => {
            let moved = Moved::Bill {
                id: bill.id,
                from: (bill.due, bill.member_id),
                to: (day, member_of(change.resource)),
            };
            history.make(store, moved, "Conta remarcada");
        }
        Item::Planned(occurrence) => {
            let moved = Moved::Planned {
                schedule: occurrence.schedule_id,
                day: occurrence.day,
                from: occurrence.when,
                to: day,
            };
            history.make(store, moved, "Dia remarcado");
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

/// What the colors on the agenda mean.
fn legend() -> impl View {
    HStack::new((
        Badge::new("A pagar").tone(Tone::Accent),
        Badge::new("A receber").tone(Tone::Success),
        Badge::new("Atrasado").tone(Tone::Danger),
    ))
    .spacing(6.0)
    .wrap()
}

/// What a right click on something of the agenda offers, as the day beside it does.
fn item_menu(item: &Item, store: &Store, forms: &Forms) -> Vec<MenuItem> {
    let mut menu = Vec::new();
    match item {
        Item::Bill(bill) if bill.paid_on.is_none() => {
            let (id, pay_store, pay) = (bill.id, store.clone(), forms.pay.clone());
            menu.push(
                MenuItem::new("Pagar").icon(IconName::Check).on_select(move || pay.start(&pay_store, id)),
            );
            let (edit_store, edit) = (store.clone(), forms.bill.clone());
            menu.push(
                MenuItem::new("Alterar conta")
                    .icon(IconName::Pencil)
                    .on_select(move || edit.start_edit(&edit_store, id)),
            );
            let (delete_store, confirm) = (store.clone(), forms.confirm.clone());
            menu.push(MenuItem::new("Excluir conta").icon(IconName::Trash2).destructive().on_select(
                move || {
                    let store = delete_store.clone();
                    confirm.ask(
                        "Excluir esta conta?",
                        "Esta ação não pode ser desfeita.",
                        "Excluir conta",
                        move || {
                            store.write("Conta excluída", |db| db.delete_bill(id));
                        },
                    );
                },
            ));
        }
        Item::Bill(bill) => {
            let (id, undo_store, confirm) = (bill.id, store.clone(), forms.confirm.clone());
            menu.push(MenuItem::new("Desfazer pagamento").icon(IconName::Undo2).on_select(move || {
                let store = undo_store.clone();
                confirm.ask(
                    "Desfazer este pagamento?",
                    "A conta volta a ficar em aberto e o lançamento da despesa é apagado.",
                    "Desfazer pagamento",
                    move || {
                        store.write("Pagamento desfeito", |db| db.reopen_bill(id));
                    },
                );
            }));
        }
        Item::Planned(occurrence) => {
            if let Some(transaction) = occurrence.transaction_id {
                let (undo_store, confirm) = (store.clone(), forms.confirm.clone());
                menu.push(MenuItem::new("Desfazer confirmação").icon(IconName::Undo2).on_select(move || {
                    let store = undo_store.clone();
                    confirm.ask(
                        "Desfazer esta confirmação?",
                        "O lançamento é apagado e o dia volta a ficar previsto.",
                        "Desfazer",
                        move || {
                            store.write("Confirmação desfeita", |db| db.undo_occurrence(transaction));
                        },
                    );
                }));
            } else {
                let (form_store, form, found) = (store.clone(), forms.occurrence.clone(), occurrence.clone());
                menu.push(
                    MenuItem::new("Confirmar")
                        .icon(IconName::Check)
                        .on_select(move || form.start(&form_store, &found)),
                );
                let (skip_store, id, day) = (store.clone(), occurrence.schedule_id, occurrence.day);
                menu.push(MenuItem::new("Pular este dia").on_select(move || {
                    skip_store.write("Dia pulado", |db| db.skip_occurrence(id, day));
                }));
            }
            let (edit_store, edit, id) = (store.clone(), forms.schedule.clone(), occurrence.schedule_id);
            menu.push(
                MenuItem::new("Alterar programação")
                    .icon(IconName::Pencil)
                    .on_select(move || edit.start_edit(&edit_store, id)),
            );
            let (stop_store, stop) = (store.clone(), forms.confirm.clone());
            menu.push(MenuItem::new("Encerrar programação").icon(IconName::Trash2).destructive().on_select(
                move || {
                    let store = stop_store.clone();
                    stop.ask(
                        "Encerrar esta programação?",
                        "Os dias futuros somem. O que já foi confirmado continua nos lançamentos.",
                        "Encerrar",
                        move || {
                            store.write("Programação encerrada", |db| db.delete_schedule(id));
                        },
                    );
                },
            ));
        }
    }
    menu
}

/// The chosen day, on the side of the timeline or the month.
fn day_panel(data: &AgendaData, day: Date, store: &Store, forms: &Forms, beside: bool) -> AnyView {
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
    let card = Card::new(
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
    );
    if beside { Box::new(card.width(DAY_PANEL_WIDTH)) } else { Box::new(card.height(DAY_PANEL_HEIGHT)) }
}

pub(crate) fn agenda(store: &Store, state: &AgendaState, forms: &Forms, window: Size) -> Page {
    let content = content_width(window);
    let timeline = state.view.get() == 0;
    // A week and a day fill the timeline, however big the window is.
    let beside = content >= SIDE_BY_SIDE_FROM;
    let timeline_width = if beside { content - DAY_PANEL_WIDTH - PANEL_GAP } else { content };
    let day_width = ((timeline_width - RESOURCE_WIDTH) / DAYS_IN_VIEW).max(NARROWEST_DAY);
    let current = *state.axis.peek();
    if current.tick_width == state.fitted.get() && day_width != current.tick_width {
        state.axis.set(TimeAxis { tick_width: day_width, ..current });
        state.fitted.set(day_width);
    }
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

    let (undo_history, undo_store) = (state.history.clone(), store.clone());
    let mut undo_button =
        IconButton::new(IconName::Undo2).label("Desfazer remarcação").size(ButtonSize::Small);
    if !state.history.undo.get().is_empty() {
        undo_button = undo_button.on_click(move || undo_history.undo(&undo_store));
    }
    let (redo_history, redo_store) = (state.history.clone(), store.clone());
    let mut redo_button =
        IconButton::new(IconName::Redo2).label("Refazer remarcação").size(ButtonSize::Small);
    if !state.history.redo.get().is_empty() {
        redo_button = redo_button.on_click(move || redo_history.redo(&redo_store));
    }
    let (plan_store, plan_form, plan_day) = (store.clone(), forms.schedule.clone(), state.day.clone());
    let toolbar = HStack::new((
        SegmentedControl::with_icons(
            state.view.binding(),
            [(IconName::Rows3, "Linha do tempo"), (IconName::CalendarDays, "Mês")],
        )
        .label("Exibição"),
        TextField::new(state.filter.binding())
            .placeholder("Filtrar por nome")
            .icon(IconName::Search)
            .compact()
            .width(FILTER_WIDTH),
        Spacer::new(),
        Tooltip::new(undo_button, "Desfazer remarcação (⌘Z)"),
        Tooltip::new(redo_button, "Refazer remarcação (⌘⇧Z)"),
        Button::new("Programar").icon(IconName::Plus).size(ButtonSize::Small).on_click(move || {
            let start = plan_day.get().unwrap_or(plan_store.today);
            plan_form.start_new(&plan_store, start, None);
        }),
    ))
    .spacing(12.0)
    .align(Align::Center)
    .wrap();

    let main: AnyView = if timeline {
        let items = Rc::new(data.items.clone());
        // What each row has in this stretch of time, beside its name.
        let row = |id: u64, name: String| {
            let open = data.items.iter().filter(|item| !item.done() && row_of(item.member()) == id).count();
            let said = match open {
                0 => "Nada previsto".to_owned(),
                1 => "1 previsto".to_owned(),
                n => format!("{n} previstos"),
            };
            Resource::new(id, name).subtitle(said)
        };
        let resources = std::iter::once(row(0, "Família toda".to_owned()))
            .chain(data.members.iter().map(|m| row(m.id as u64, m.name.clone())))
            .collect::<Vec<_>>();
        // A day of the agenda has no length to change, so bars only move.
        let wanted = state.filter.get().to_lowercase();
        let shows = |item: &Item| wanted.is_empty() || item.title().to_lowercase().contains(&wanted);
        let events = data.items.iter().enumerate().filter(|(_, item)| shows(item)).map(|(i, item)| {
            let start = TimePoint::from_date(item.day());
            SchedulerEvent::new(i as u64, row_of(item.member()), start, start.plus_days(1), item.title())
                .tone(item.tone(today))
                .done(item.done())
                .locked(item.done())
                .resizable(false)
        });
        let (change_store, change_items, change_history) =
            (store.clone(), items.clone(), state.history.clone());
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
        let detail_items = items.clone();
        let (menu_items, menu_store, menu_forms) = (items.clone(), store.clone(), forms.clone());
        let (lane_store, lane_form) = (store.clone(), forms.schedule.clone());
        let (new_store, new_form, new_day) = (store.clone(), forms.schedule.clone(), state.day.clone());
        Box::new(
            Scheduler::new(state.axis.binding())
                .resources(resources)
                .events(events)
                .now(TimePoint::from_date(today))
                .resource_width(RESOURCE_WIDTH)
                .card(true)
                .avatars(true)
                .event_detail(move |id| detail_items.get(id as usize).map(|item| format_brl(item.cents())))
                .event_menu(move |id| {
                    menu_items
                        .get(id as usize)
                        .map_or_else(Vec::new, |item| item_menu(item, &menu_store, &menu_forms))
                })
                .lane_menu(move |change| {
                    let (store, form) = (lane_store.clone(), lane_form.clone());
                    vec![MenuItem::new("Programar aqui").icon(IconName::Plus).on_select(move || {
                        form.start_new(&store, change.start.date(), member_of(change.resource));
                    })]
                })
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
                        apply_move(&change_store, &change_history, item, change);
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
        let wanted = state.filter.get().to_lowercase();
        let events: Vec<CalendarEvent> = data
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| wanted.is_empty() || item.title().to_lowercase().contains(&wanted))
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

    Page::new(
        help::AGENDA,
        format!("Previsto no período: entra {} · sai {}", format_brl(incoming), format_brl(outgoing)),
        screen((
            HistoryKeys { history: state.history.clone(), store: store.clone() },
            toolbar,
            Fill2 {
                main,
                side: Box::new(
                    VStack::new((day_panel(&data, day, store, forms, beside), legend()))
                        .spacing(PANEL_GAP)
                        .align(Align::Stretch),
                ),
                beside,
            },
        )),
    )
}

/// The timeline or the month, taking the room left, beside the chosen day.
struct Fill2 {
    main: AnyView,
    side: AnyView,
    beside: bool,
}

impl View for Fill2 {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let main = cx.child(self.main.as_ref());
        let side = cx.child(self.side.as_ref());
        cx.container(
            ravenrs::layout::LayoutStyle {
                grow: 1.0,
                min_height: Some(0.0),
                spacing: PANEL_GAP,
                align: Align::Stretch,
                overflow_hidden: true,
                ..ravenrs::layout::LayoutStyle::stack(if self.beside {
                    ravenrs::layout::Axis::Horizontal
                } else {
                    ravenrs::layout::Axis::Vertical
                })
            },
            &[main, side],
            ravenrs::widgets::Paint::None,
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "test helpers fail loudly")]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::seed;
    use crate::update::Updater;

    #[test]
    fn a_move_can_be_undone_and_done_again_and_a_new_one_clears_what_was_undone() {
        let today = Date::new(2026, 11, 15).expect("valid date");
        let db = Database::in_memory().expect("database");
        seed::load_demo(&db, today).expect("seed");
        let store = Store::new(db, today, None, Updater::idle());
        let history = MoveHistory::new();
        let bill = store.read(|db| db.open_bills()).into_iter().next().expect("a bill");
        let due = |store: &Store| {
            store.read(|db| db.open_bills()).iter().find(|b| b.id == bill.id).expect("the bill").due
        };
        let (from, to) = (bill.due, bill.due.add_days(3));
        let moved = Moved::Bill { id: bill.id, from: (from, bill.member_id), to: (to, bill.member_id) };

        history.make(&store, moved, "");
        assert_eq!(due(&store), to);
        history.undo(&store);
        assert_eq!(due(&store), from);
        history.redo(&store);
        assert_eq!(due(&store), to);
        history.undo(&store);
        assert_eq!(history.redo.get().len(), 1);
        history.make(&store, moved, "");
        assert!(history.redo.get().is_empty());
        assert_eq!(history.undo.get().len(), 1);
    }
}
