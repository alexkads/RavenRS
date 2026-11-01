//! Transactions: a month in a grid, with quick filters and an expression filter
//! that runs in SQLite through `ravenrs-criteria`.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Memo, Store, date_column, heading, money_column, month_nav, row_id, screen};
use crate::filter;
use crate::model::Flow;
use crate::money::format_brl;
use crate::paths;

/// Ready-made expressions, which also teach the language.
const PRESETS: [(&str, &str); 5] = [
    ("Acima de R$ 500", "[Valor] > 500"),
    ("Mercado e lazer", "[Categoria] In ('Mercado', 'Lazer')"),
    ("Da Lia", "[Membro] = 'Lia'"),
    ("Na carteira", "[Conta] = 'Carteira'"),
    ("Sem responsável", "[Membro] = ''"),
];

struct LedgerRows {
    data: GridData,
    error: Option<String>,
    income: i64,
    expense: i64,
}

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct LedgerState {
    month: State<Date>,
    kind: State<usize>,
    typed: State<String>,
    applied: State<String>,
    selection: State<BTreeSet<usize>>,
    rows: Memo<(u64, Date, usize, String), Rc<LedgerRows>>,
}

impl LedgerState {
    pub fn new(today: Date) -> Self {
        Self {
            month: State::new(today.first_of_month()),
            kind: State::new(0),
            typed: State::new(String::new()),
            applied: State::new(String::new()),
            selection: State::new(BTreeSet::new()),
            rows: Memo::new(),
        }
    }
}

fn load(store: &Store, month: Date, kind: usize, text: &str) -> LedgerRows {
    let compiled = match kind {
        1 => filter::compile(&combine(text, "[Tipo] = 'Despesa'")),
        2 => filter::compile(&combine(text, "[Tipo] = 'Receita'")),
        _ => filter::compile(text),
    };
    let (compiled, error) = match compiled {
        Ok(compiled) => (compiled, None),
        Err(message) => (None, Some(message)),
    };
    let rows = store.read(|db| db.ledger(month, compiled.as_ref()));
    let (mut income, mut expense) = (0, 0);
    let cells = rows
        .into_iter()
        .map(|row| {
            let cents = (row.value * 100.0).round() as i64;
            let signed = if row.kind == Flow::Income.label() {
                income += cents;
                row.value
            } else {
                expense += cents;
                -row.value
            };
            vec![
                Value::Number(row.id as f64),
                row.date.into(),
                row.description.into(),
                row.category.into(),
                row.account.into(),
                row.member.into(),
                row.kind.into(),
                Value::Number(signed),
            ]
        })
        .collect();
    LedgerRows { data: GridData::new(cells), error, income, expense }
}

fn combine(typed: &str, condition: &str) -> String {
    if typed.trim().is_empty() { condition.to_owned() } else { format!("({typed}) And {condition}") }
}

pub(crate) fn ledger(store: &Store, state: &LedgerState, forms: &Forms) -> impl View + use<> {
    let (month, kind, applied) = (state.month.get(), state.kind.get(), state.applied.get());
    let rows = state
        .rows
        .get((store.version(), month, kind, applied.clone()), || Rc::new(load(store, month, kind, &applied)));
    let data = rows.data.clone();

    let selected = state.selection.get().iter().next().and_then(|row| row_id(&data, *row));

    let (new_store, new_expense) = (store.clone(), forms.transaction.clone());
    let (new_store_2, new_income) = (store.clone(), forms.transaction.clone());
    let (edit_store, edit) = (store.clone(), forms.transaction.clone());
    let (delete_store, confirm, delete_selection) =
        (store.clone(), forms.confirm.clone(), state.selection.clone());
    let toolbar = HStack::new((
        month_nav(&state.month, store.today),
        SegmentedControl::new(state.kind.binding(), ["Todos", "Despesas", "Receitas"]).label("Mostrar"),
        Spacer::new(),
        Button::new("Nova receita")
            .icon(IconName::TrendingUp)
            .style(ButtonStyle::Secondary)
            .on_click(move || new_income.start_new(&new_store_2, Flow::Income)),
        Button::new("Nova despesa")
            .icon(IconName::Plus)
            .on_click(move || new_expense.start_new(&new_store, Flow::Expense)),
    ))
    .spacing(8.0)
    .align(Align::Center);

    let (typed, apply_read, apply_write, sel_1) =
        (state.typed.clone(), state.typed.clone(), state.applied.clone(), state.selection.clone());
    let apply_submit = {
        let applied = state.applied.clone();
        let selection = state.selection.clone();
        move |text: String| {
            selection.set(BTreeSet::new());
            applied.set(text);
        }
    };
    let (clear_typed, clear_applied) = (state.typed.clone(), state.applied.clone());
    let has_filter = !state.applied.get().is_empty();
    let mut presets =
        DropdownMenu::new("Filtros prontos").icon(IconName::Filter).style(ButtonStyle::Secondary);
    for (label, expression) in PRESETS {
        let (t, a, s) = (state.typed.clone(), state.applied.clone(), state.selection.clone());
        presets = presets.item(MenuItem::new(label).on_select(move || {
            s.set(BTreeSet::new());
            t.set(expression.to_owned());
            a.set(expression.to_owned());
        }));
    }
    let filter_bar = HStack::new((
        TextField::new(typed.binding())
            .placeholder("Filtro avançado, ex.: [Valor] > 100 And [Categoria] = 'Mercado'")
            .icon(IconName::SlidersHorizontal)
            .on_submit(apply_submit)
            .stretch()
            .grow(1.0),
        Button::new("Aplicar").style(ButtonStyle::Secondary).on_click(move || {
            sel_1.set(BTreeSet::new());
            apply_write.set(apply_read.get());
        }),
        has_filter.then(|| {
            Button::new("Limpar").icon(IconName::FilterX).style(ButtonStyle::Ghost).on_click(move || {
                clear_typed.set(String::new());
                clear_applied.set(String::new());
            })
        }),
        presets,
        Button::new("Alterar")
            .icon(IconName::Pencil)
            .style(ButtonStyle::Ghost)
            .disabled(selected.is_none())
            .on_click(move || {
                if let Some(id) = selected {
                    edit.start_edit(&edit_store, id);
                }
            }),
        Button::new("Excluir")
            .icon(IconName::Trash2)
            .style(ButtonStyle::Ghost)
            .disabled(selected.is_none())
            .on_click(move || {
                let Some(id) = selected else { return };
                let (store, selection) = (delete_store.clone(), delete_selection.clone());
                confirm.ask(
                    "Excluir este lançamento?",
                    "Se ele pagou uma conta, a conta volta a ficar em aberto.",
                    "Excluir lançamento",
                    move || {
                        selection.set(BTreeSet::new());
                        store.write("Lançamento excluído", |db| db.delete_transaction(id));
                    },
                );
            }),
    ))
    .spacing(8.0)
    .align(Align::Center);

    let error =
        rows.error.clone().map(|message| Alert::new("Filtro não aplicado", message).tone(Tone::Warning));
    let (activate_store, activate, activate_data) = (store.clone(), forms.transaction.clone(), data.clone());
    let export_store = store.clone();
    let grid = DataGrid::new(
        data,
        [
            GridColumn::number("Id").hidden(),
            date_column("Data"),
            GridColumn::text("Descrição").width(250.0),
            GridColumn::text("Categoria").width(140.0),
            GridColumn::text("Conta").width(140.0),
            GridColumn::text("De quem").width(100.0),
            GridColumn::text("Tipo").width(96.0).template(|value| {
                let text = if let Value::Text(text) = value { text.as_str() } else { "" };
                let tone = if text == Flow::Income.label() { Tone::Success } else { Tone::Neutral };
                Box::new(Badge::new(text).tone(tone)) as AnyView
            }),
            money_column("Valor").summary(Summary::Sum),
        ],
    )
    .selection(state.selection.binding())
    .on_row_activate(move |row| {
        if let Some(id) = row_id(&activate_data, row) {
            activate.start_edit(&activate_store, id);
        }
    })
    .on_export(move |csv| match paths::save_export("lancamentos.csv", csv.as_bytes()) {
        Ok(path) => {
            export_store.toasts.show("CSV exportado", path.display().to_string(), Tone::Success);
            paths::open_with_system(&path);
        }
        Err(e) => export_store.toasts.show("Não foi possível exportar", e.to_string(), Tone::Danger),
    });

    screen((
        heading(
            "Lançamentos",
            format!(
                "Entrou {} · saiu {} · sobrou {}",
                format_brl(rows.income),
                format_brl(rows.expense),
                format_brl(rows.income - rows.expense)
            ),
        ),
        toolbar,
        filter_bar,
        error,
        grid,
    ))
}
