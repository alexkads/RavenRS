//! A calendar of due dates.

use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Memo, Store, heading};
use crate::model::{Bill, BillStatus, date_label, month_label};
use crate::money::format_brl;

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct AgendaState {
    day: State<Option<Date>>,
    bills: Memo<u64, Rc<Vec<Bill>>>,
}

impl AgendaState {
    pub fn new(today: Date) -> Self {
        Self { day: State::new(Some(today)), bills: Memo::new() }
    }
}

fn bill_rows(bills: &[&Bill], store: &Store, forms: &Forms) -> Vec<AnyView> {
    bills
        .iter()
        .map(|bill| {
            let status = bill.status(store.today);
            let (id, pay_store, pay) = (bill.id, store.clone(), forms.pay.clone());
            let is_open = bill.paid_on.is_none();
            Box::new(
                HStack::new((
                    VStack::new((
                        Text::new(bill.title()).truncate(),
                        Text::new(format!("{} · {}", date_label(bill.due), bill.category))
                            .style(TextStyle::Caption)
                            .foreground(Semantic::InkMuted),
                    ))
                    .spacing(2.0)
                    .grow(1.0),
                    Badge::new(status.label()).tone(match status {
                        BillStatus::Overdue => Tone::Danger,
                        BillStatus::DueToday => Tone::Warning,
                        BillStatus::Paid => Tone::Success,
                        BillStatus::Upcoming => Tone::Neutral,
                    }),
                    Text::new(format_brl(bill.amount_cents)),
                    is_open.then(|| {
                        Button::new("Pagar")
                            .accessible_name(format!("Pagar {}", bill.title()))
                            .size(ButtonSize::Small)
                            .style(ButtonStyle::Secondary)
                            .on_click(move || pay.start(&pay_store, id))
                    }),
                ))
                .spacing(12.0)
                .align(Align::Center),
            ) as AnyView
        })
        .collect()
}

pub(crate) fn agenda(store: &Store, state: &AgendaState, forms: &Forms) -> impl View + use<> {
    let bills = state.bills.get(store.version(), || Rc::new(store.read(|db| db.bills())));
    let day = state.day.get().unwrap_or(store.today);
    let today_bills: Vec<&Bill> = bills.iter().filter(|b| b.due == day).collect();
    let (from, to) = (day.first_of_month(), day.first_of_month().add_months(1));
    let month_bills: Vec<&Bill> = bills.iter().filter(|b| b.due >= from && b.due < to).collect();
    let month_total: i64 = month_bills.iter().map(|b| b.amount_cents).sum();

    let mut day_rows = bill_rows(&today_bills, store, forms);
    if day_rows.is_empty() {
        day_rows.push(Box::new(Text::new("Nenhuma conta vence neste dia.").foreground(Semantic::InkMuted)));
    }
    let mut month_rows = bill_rows(&month_bills, store, forms);
    if month_rows.is_empty() {
        month_rows.push(Box::new(Text::new("Nenhuma conta neste mês.").foreground(Semantic::InkMuted)));
    }
    ScrollView::new((VStack::new((
        heading("Agenda", "Escolha um dia para ver o que vence."),
        HStack::new((
            Card::new(Calendar::new(state.day.binding(), store.today).today(store.today)),
            VStack::new((
                Card::new(
                    VStack::new((
                        Text::new(format!("Dia {}", date_label(day))).style(TextStyle::Headline),
                        VStack::new(day_rows).spacing(12.0).align(Align::Stretch),
                    ))
                    .spacing(12.0)
                    .align(Align::Stretch),
                ),
                Card::new(
                    VStack::new((
                        Text::new(format!("Em {}: {}", month_label(day), format_brl(month_total)))
                            .style(TextStyle::Headline),
                        VStack::new(month_rows).spacing(12.0).align(Align::Stretch),
                    ))
                    .spacing(12.0)
                    .align(Align::Stretch),
                ),
            ))
            .spacing(16.0)
            .align(Align::Stretch)
            .grow(1.0),
        ))
        .spacing(16.0)
        .align(Align::Start),
    ))
    .spacing(16.0)
    .align(Align::Stretch),))
    .padding(24.0)
}
