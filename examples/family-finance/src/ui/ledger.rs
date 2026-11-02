//! Transactions: a month in a grid, with quick filters and an expression filter
//! that runs in SQLite through `ravenrs-criteria`.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs::prelude::*;

use super::forms::Forms;
use super::help;
use super::store::{
    Memo, Page, Store, content_width, date_column, filling, money_column, month_nav, remembered_scenarios,
    row_id, screen,
};
use crate::filter;
use crate::model::Flow;
use crate::money::format_brl;

/// What a window narrower than each of these leaves out, so that the amount stays.
const ROOM_FOR_ALL_COLUMNS: f32 = 860.0;
const ROOM_FOR_THE_CATEGORY: f32 = 640.0;
const ROOM_FOR_BOTH_ACTIONS: f32 = 860.0;

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
    advanced: State<bool>,
    typed: State<String>,
    applied: State<String>,
    selection: State<BTreeSet<usize>>,
    scenarios: Binding<Vec<GridScenario>>,
    rows: Memo<(u64, Date, usize, String), Rc<LedgerRows>>,
}

impl LedgerState {
    pub fn new(today: Date, path: Option<&std::path::Path>) -> Self {
        Self {
            scenarios: remembered_scenarios(path, "lancamentos"),
            month: State::new(today.first_of_month()),
            kind: State::new(0),
            advanced: State::new(false),
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

/// What came in stands out from what went out.
fn signed_amount(value: &Value) -> AnyView {
    let cents = (value.as_f64().unwrap_or(0.0) * 100.0).round() as i64;
    if cents > 0 {
        Box::new(
            Text::new(format!("+{}", format_brl(cents))).style(TextStyle::Body).foreground(Semantic::Success),
        )
    } else {
        Box::new(Text::new(format!("-{}", format_brl(-cents))).style(TextStyle::Body))
    }
}

fn combine(typed: &str, condition: &str) -> String {
    if typed.trim().is_empty() { condition.to_owned() } else { format!("({typed}) And {condition}") }
}

pub(crate) fn ledger(store: &Store, state: &LedgerState, forms: &Forms, window: Size) -> Page {
    let content = content_width(window);
    let all_columns = content >= ROOM_FOR_ALL_COLUMNS;
    let category = content >= ROOM_FOR_THE_CATEGORY;
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
        // With little room the main action stays on the line; Novo also has this one.
        (content >= ROOM_FOR_BOTH_ACTIONS).then(|| {
            Button::new("Nova receita")
                .icon(IconName::TrendingUp)
                .size(ButtonSize::Small)
                .style(ButtonStyle::Secondary)
                .on_click(move || new_income.start_new(&new_store_2, Flow::Income))
        }),
        Button::new("Nova despesa")
            .icon(IconName::Plus)
            .size(ButtonSize::Small)
            .on_click(move || new_expense.start_new(&new_store, Flow::Expense)),
    ))
    .spacing(8.0)
    .align(Align::Center)
    .wrap();

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
    let rows_error = rows.error.is_some();
    let (clear_typed, clear_applied) = (state.typed.clone(), state.applied.clone());
    let has_filter = !state.applied.get().is_empty();
    let mut presets = DropdownMenu::new("Filtrar")
        .icon(IconName::Filter)
        .size(ButtonSize::Small)
        .style(ButtonStyle::Secondary);
    for (label, expression) in PRESETS {
        let (t, a, s) = (state.typed.clone(), state.applied.clone(), state.selection.clone());
        presets = presets.item(MenuItem::new(label).on_select(move || {
            s.set(BTreeSet::new());
            t.set(expression.to_owned());
            a.set(expression.to_owned());
        }));
    }
    let advanced_open = state.advanced.get() || rows_error;
    let advanced = advanced_open.then(|| {
        HStack::new((
            TextField::new(typed.binding())
                .placeholder("Ex.: [Valor] > 100 And [Categoria] = 'Mercado'")
                .icon(IconName::SlidersHorizontal)
                .compact()
                .on_submit(apply_submit)
                .stretch()
                .grow(1.0),
            Button::new("Aplicar").size(ButtonSize::Small).style(ButtonStyle::Secondary).on_click(
                move || {
                    sel_1.set(BTreeSet::new());
                    apply_write.set(apply_read.get());
                },
            ),
        ))
        .spacing(8.0)
        .align(Align::Center)
    });
    let toggle_advanced = state.advanced.clone();
    let presets = presets.item(
        MenuItem::new("Filtro avançado…").on_select(move || toggle_advanced.update(|open| *open = !*open)),
    );
    let filter_bar = HStack::new((
        presets,
        help::tip(help::FILTROS),
        has_filter.then(|| {
            Button::new("Limpar")
                .icon(IconName::FilterX)
                .size(ButtonSize::Small)
                .style(ButtonStyle::Ghost)
                .on_click(move || {
                    clear_typed.set(String::new());
                    clear_applied.set(String::new());
                })
        }),
        Spacer::new(),
        Button::new("Alterar")
            .icon(IconName::Pencil)
            .size(ButtonSize::Small)
            .style(ButtonStyle::Ghost)
            .disabled(selected.is_none())
            .on_click(move || {
                if let Some(id) = selected {
                    edit.start_edit(&edit_store, id);
                }
            }),
        Button::new("Excluir")
            .icon(IconName::Trash2)
            .size(ButtonSize::Small)
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
    .align(Align::Center)
    .wrap();

    let error =
        rows.error.clone().map(|message| Alert::new("Filtro não aplicado", message).tone(Tone::Warning));
    let (activate_store, activate, activate_data) = (store.clone(), forms.transaction.clone(), data.clone());
    let export_store = store.clone();
    let grid = DataGrid::new(
        data,
        [
            GridColumn::number("Id").hidden(),
            date_column("Data"),
            GridColumn::text("Descrição").width(220.0).fill(),
            if category {
                GridColumn::text("Categoria").width(140.0)
            } else {
                GridColumn::text("Categoria").width(140.0).hidden()
            },
            if all_columns {
                GridColumn::text("Conta").width(150.0)
            } else {
                GridColumn::text("Conta").width(150.0).hidden()
            },
            if all_columns {
                GridColumn::text("De quem").width(100.0)
            } else {
                GridColumn::text("De quem").width(100.0).hidden()
            },
            // The sign and the color of the value already say it; the column chooser brings it back.
            GridColumn::text("Tipo").width(96.0).hidden(),
            money_column("Valor").summary(Summary::Sum).template(signed_amount),
        ],
    )
    .filter_row(false)
    .empty_text("Nenhum lançamento neste mês. Comece em Nova despesa.")
    .selection(state.selection.binding())
    .scenarios(state.scenarios.clone())
    .on_row_activate(move |row| {
        if let Some(id) = row_id(&activate_data, row) {
            activate.start_edit(&activate_store, id);
        }
    })
    .on_export(move |csv| {
        export_store.save_file("lancamentos.csv", ("CSV", &["csv"]), csv.as_bytes());
    });

    Page::new(
        help::LANCAMENTOS,
        format!(
            "Entrou {} · saiu {} · sobrou {}",
            format_brl(rows.income),
            format_brl(rows.expense),
            format_brl(rows.income - rows.expense)
        ),
        screen((
            toolbar,
            filter_bar,
            advanced,
            error,
            Keyed::new(u64::from(all_columns) + u64::from(category) * 2, filling(grid)),
        )),
    )
}
