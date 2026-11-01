//! Bills to pay: what is due, what is late, paying and undoing.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Memo, Store, date_column, heading, money_column, row_id, screen};
use crate::model::{Bill, BillStatus};
use crate::money::format_brl;

struct BillRows {
    data: GridData,
    overdue: i64,
    open: i64,
    open_count: usize,
    bills: Vec<Bill>,
}

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct BillsState {
    view: State<usize>,
    selection: State<BTreeSet<usize>>,
    rows: Memo<(u64, usize), Rc<BillRows>>,
}

impl BillsState {
    pub fn new() -> Self {
        Self { view: State::new(0), selection: State::new(BTreeSet::new()), rows: Memo::new() }
    }
}

fn load(store: &Store, view: usize) -> BillRows {
    let today = store.today;
    let all = store.read(|db| db.bills());
    let open: Vec<&Bill> = all.iter().filter(|b| b.paid_on.is_none()).collect();
    let overdue =
        open.iter().filter(|b| b.status(today) == BillStatus::Overdue).map(|b| b.amount_cents).sum();
    let open_total = open.iter().map(|b| b.amount_cents).sum();
    let open_count = open.len();
    let mut shown: Vec<Bill> = all
        .iter()
        .filter(|bill| match view {
            0 => bill.paid_on.is_none(),
            1 => bill.status(today) == BillStatus::Overdue,
            2 => bill.paid_on.is_some(),
            _ => true,
        })
        .cloned()
        .collect();
    if view == 2 {
        shown.reverse();
    }
    let cells = shown
        .iter()
        .map(|bill| {
            vec![
                Value::Number(bill.id as f64),
                bill.due.to_string().into(),
                bill.title().into(),
                bill.category.clone().into(),
                bill.member.clone().into(),
                bill.recurrence.label().into(),
                bill.status(today).label().into(),
                Value::Number(bill.amount_cents as f64 / 100.0),
                bill.paid_on.map_or(Value::Empty, |d| d.to_string().into()),
            ]
        })
        .collect();
    BillRows { data: GridData::new(cells), overdue, open: open_total, open_count, bills: shown }
}

pub(crate) fn bills(store: &Store, state: &BillsState, forms: &Forms) -> impl View + use<> {
    let view = state.view.get();
    let rows = state.rows.get((store.version(), view), || Rc::new(load(store, view)));
    let data = rows.data.clone();
    let selected = state.selection.get().iter().next().and_then(|row| row_id(&data, *row));
    let selected_bill = selected.and_then(|id| rows.bills.iter().find(|b| b.id == id));
    let is_open = selected_bill.is_some_and(|b| b.paid_on.is_none());
    let is_paid = selected_bill.is_some_and(|b| b.paid_on.is_some());

    let (pay_store, pay) = (store.clone(), forms.pay.clone());
    let (edit_store, edit) = (store.clone(), forms.bill.clone());
    let (new_store, new) = (store.clone(), forms.bill.clone());
    let (undo_store, undo_confirm, undo_selection) =
        (store.clone(), forms.confirm.clone(), state.selection.clone());
    let (delete_store, delete_confirm, delete_selection) =
        (store.clone(), forms.confirm.clone(), state.selection.clone());
    let clear = state.selection.clone();
    let source = state.view.clone();
    let view_binding = Binding::new(move || source.get(), {
        let target = state.view.clone();
        move |value| {
            clear.set(BTreeSet::new());
            target.set(value);
        }
    });
    let toolbar = HStack::new((
        SegmentedControl::new(view_binding, ["Em aberto", "Vencidas", "Pagas", "Todas"]).label("Mostrar"),
        Spacer::new(),
        is_paid.then(|| {
            Button::new("Desfazer pagamento").icon(IconName::Undo2).style(ButtonStyle::Ghost).on_click(
                move || {
                    let Some(id) = selected else { return };
                    let (store, selection) = (undo_store.clone(), undo_selection.clone());
                    undo_confirm.ask(
                        "Desfazer este pagamento?",
                        "A conta volta a ficar em aberto e o lançamento da despesa é apagado.",
                        "Desfazer pagamento",
                        move || {
                            selection.set(BTreeSet::new());
                            store.write("Pagamento desfeito", |db| db.reopen_bill(id));
                        },
                    );
                },
            )
        }),
        Button::new("Alterar").icon(IconName::Pencil).style(ButtonStyle::Ghost).disabled(!is_open).on_click(
            move || {
                if let Some(id) = selected {
                    edit.start_edit(&edit_store, id);
                }
            },
        ),
        Button::new("Excluir").icon(IconName::Trash2).style(ButtonStyle::Ghost).disabled(!is_open).on_click(
            move || {
                let Some(id) = selected else { return };
                let (store, selection) = (delete_store.clone(), delete_selection.clone());
                delete_confirm.ask(
                    "Excluir esta conta?",
                    "Esta ação não pode ser desfeita.",
                    "Excluir conta",
                    move || {
                        selection.set(BTreeSet::new());
                        store.write("Conta excluída", |db| db.delete_bill(id));
                    },
                );
            },
        ),
        Button::new("Pagar").icon(IconName::Check).style(ButtonStyle::Secondary).disabled(!is_open).on_click(
            move || {
                if let Some(id) = selected {
                    pay.start(&pay_store, id);
                }
            },
        ),
        Button::new("Nova conta").icon(IconName::Plus).on_click(move || new.start_new(&new_store)),
    ))
    .spacing(8.0)
    .align(Align::Center);

    let (activate_store, activate, activate_data) = (store.clone(), forms.pay.clone(), data.clone());
    let activate_bills = rows.clone();
    let grid = DataGrid::new(
        data,
        [
            GridColumn::number("Id").hidden(),
            date_column("Vencimento"),
            GridColumn::text("Conta").width(240.0),
            GridColumn::text("Categoria").width(130.0),
            GridColumn::text("De quem").width(100.0),
            GridColumn::text("Repete").width(110.0),
            GridColumn::text("Situação").width(116.0).template(|value| {
                let text = if let Value::Text(text) = value { text.as_str() } else { "" };
                let tone = match text {
                    t if t == BillStatus::Overdue.label() => Tone::Danger,
                    t if t == BillStatus::DueToday.label() => Tone::Warning,
                    t if t == BillStatus::Paid.label() => Tone::Success,
                    _ => Tone::Neutral,
                };
                Box::new(Badge::new(text).tone(tone)) as AnyView
            }),
            money_column("Valor").summary(Summary::Sum),
            date_column("Paga em"),
        ],
    )
    .selection(state.selection.binding())
    .on_row_activate(move |row| {
        let Some(id) = row_id(&activate_data, row) else { return };
        if activate_bills.bills.iter().any(|b| b.id == id && b.paid_on.is_none()) {
            activate.start(&activate_store, id);
        }
    });

    screen((
        heading(
            "Contas a pagar",
            format!(
                "{} em aberto somando {}, das quais {} em atraso",
                rows.open_count,
                format_brl(rows.open),
                format_brl(rows.overdue)
            ),
        ),
        toolbar,
        grid,
    ))
}
