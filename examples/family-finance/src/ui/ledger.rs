//! Transactions: a month in a list generated from what they are, with quick filters and an
//! expression filter that runs in SQLite through `ravenrs-criteria`.

use std::cell::{Cell, OnceCell};
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    Action, ActionContext, Application, ApplicationBuilder, Criteria, Module, Query, Selection,
    Value as Field, ViewController,
};
use ravenrs_app_ui::{ActionButton, ListView, Session, Words};

use super::entities;
use super::forms::Forms;
use super::help;
use super::store::{
    Memo, Page, Store, content_width, date_column, filling, follow_store, id_of_field, money_column,
    month_nav, remembered_scenarios, screen,
};
use crate::filter;
use crate::model::{Flow, month_range};
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

/// What the month's list asks for, and what it adds up to.
struct LedgerRows {
    filter: Option<Criteria>,
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
    scenarios: Binding<Vec<GridScenario>>,
    rows: Memo<(u64, Date, usize, String), Rc<LedgerRows>>,
    session: Rc<OnceCell<Option<Session>>>,
    seen: Rc<Cell<u64>>,
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
            rows: Memo::new(),
            session: Rc::default(),
            seen: Rc::default(),
        }
    }
}

const NEW_EXPENSE: &str = "NewExpense";
const NEW_INCOME: &str = "NewIncome";
const EDIT: &str = "EditEntry";
const DELETE: &str = "DeleteEntry";

fn selected_id(cx: &ActionContext) -> Option<i64> {
    cx.selected.first().and_then(|entry| id_of_field(&entry.get("Id")))
}

/// What can be done to the month's transactions.
struct LedgerCommands {
    store: Store,
    forms: Forms,
}

impl ViewController for LedgerCommands {
    fn id(&self) -> &str {
        "LedgerCommands"
    }

    fn actions(&self) -> Vec<Action> {
        let (store, forms) = (self.store.clone(), self.forms.clone());
        let new = |flow: Flow| {
            let (store, form) = (store.clone(), forms.transaction.clone());
            move |_: &ActionContext| {
                form.start_new(&store, flow);
                Ok(())
            }
        };
        let (edit_store, edit_form, gone) = (store.clone(), forms.transaction.clone(), store.clone());
        vec![
            Action::simple(NEW_EXPENSE, "Nova despesa").icon("plus").on_execute(new(Flow::Expense)),
            Action::simple(NEW_INCOME, "Nova receita").icon("trending-up").on_execute(new(Flow::Income)),
            Action::simple(EDIT, "Alterar").icon("pencil").selection(Selection::RequireSingle).on_execute(
                move |cx| {
                    if let Some(id) = selected_id(cx) {
                        edit_form.start_edit(&edit_store, id);
                    }
                    Ok(())
                },
            ),
            Action::simple(DELETE, "Excluir")
                .icon("trash-2")
                .selection(Selection::RequireSingle)
                .confirmation("Excluir este lançamento?")
                .confirmation_words(
                    "Se ele pagou uma conta, a conta volta a ficar em aberto.",
                    "Excluir lançamento",
                )
                .on_execute(move |cx| {
                    if let Some(id) = selected_id(cx) {
                        gone.write("Lançamento excluído", |db| db.delete_transaction(id));
                    }
                    Ok(())
                }),
        ]
    }
}

struct LedgerModule {
    store: Store,
    forms: Forms,
}

impl Module for LedgerModule {
    fn name(&self) -> &str {
        "Ledger"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(LedgerCommands { store: self.store.clone(), forms: self.forms.clone() })]
    }
}

fn make_session(store: &Store, forms: &Forms) -> Option<Session> {
    let database = entities::database(store)?;
    let application = database
        .entities()
        .into_iter()
        .fold(Application::builder("Lançamentos"), ApplicationBuilder::entity)
        .module(LedgerModule { store: store.clone(), forms: forms.clone() })
        .build()
        .map_err(|e| tracing::error!(error = %e, "the ledger could not be put together"))
        .ok()?;
    let session = Session::embedded(application, move || database.space(), Words::pt_br());
    session.show_list("LedgerEntry_ListView");
    Some(session)
}

/// Reads the month's list as it will show, for what it adds up to.
fn load(session: &Session, month: Date, kind: usize, text: &str) -> LedgerRows {
    let typed = match kind {
        1 => combine(text, "[Tipo] = 'Despesa'"),
        2 => combine(text, "[Tipo] = 'Receita'"),
        _ => text.to_owned(),
    };
    // What cannot run says why, and the month shows whole.
    let (typed, error) = match filter::compile(&typed) {
        Ok(_) => (typed, None),
        Err(message) => (String::new(), Some(message)),
    };
    let (from, to) = month_range(month);
    let period = format!("[Data] >= '{from}' And [Data] < '{to}'");
    let filter = Criteria::parse(&combine(&typed, &period)).ok();
    let mut query = Query::all();
    if let Some(filter) = &filter {
        query = query.filter(filter.clone());
    }
    let found = session.frame().space().borrow().query("LedgerEntry", &query);
    let (mut income, mut expense) = (0, 0);
    for entry in found.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "could not read the ledger");
        Vec::new()
    }) {
        let Field::Number(value) = entry.get("Valor") else { continue };
        let cents = (value * 100.0).round() as i64;
        if cents > 0 {
            income += cents;
        } else {
            expense -= cents;
        }
    }
    LedgerRows { filter, error, income, expense }
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
    let session = state.session.get_or_init(|| make_session(store, forms));
    let Some(session) = session else {
        return Page::new(
            help::LANCAMENTOS,
            "",
            Alert::new("Lançamentos", "Não foi possível abrir os lançamentos.").tone(Tone::Danger),
        );
    };
    follow_store(store, session, &state.seen);
    let (month, kind, applied) = (state.month.get(), state.kind.get(), state.applied.get());
    let rows = state.rows.get((store.version(), month, kind, applied.clone()), || {
        Rc::new(load(session, month, kind, &applied))
    });

    let toolbar = HStack::new((
        month_nav(&state.month, store.today),
        SegmentedControl::new(state.kind.binding(), ["Todos", "Despesas", "Receitas"]).label("Mostrar"),
        Spacer::new(),
        // With little room the main action stays on the line; Novo also has this one.
        (content >= ROOM_FOR_BOTH_ACTIONS)
            .then(|| ActionButton::new(session.clone(), NEW_INCOME).style(ButtonStyle::Secondary)),
        ActionButton::new(session.clone(), NEW_EXPENSE),
    ))
    .spacing(8.0)
    .align(Align::Center)
    .wrap();

    let (typed, apply_read, apply_write) = (state.typed.clone(), state.typed.clone(), state.applied.clone());
    let apply_submit = {
        let applied = state.applied.clone();
        move |text: String| applied.set(text)
    };
    let rows_error = rows.error.is_some();
    let (clear_typed, clear_applied) = (state.typed.clone(), state.applied.clone());
    let has_filter = !state.applied.get().is_empty();
    let mut presets = DropdownMenu::new("Filtrar")
        .icon(IconName::Filter)
        .size(ButtonSize::Small)
        .style(ButtonStyle::Secondary);
    for (label, expression) in PRESETS {
        let (t, a) = (state.typed.clone(), state.applied.clone());
        presets = presets.item(MenuItem::new(label).on_select(move || {
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
            Button::new("Aplicar")
                .size(ButtonSize::Small)
                .style(ButtonStyle::Secondary)
                .on_click(move || apply_write.set(apply_read.get())),
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
        ActionButton::new(session.clone(), EDIT).style(ButtonStyle::Ghost),
        ActionButton::new(session.clone(), DELETE).style(ButtonStyle::Ghost),
    ))
    .spacing(8.0)
    .align(Align::Center)
    .wrap();

    let error =
        rows.error.clone().map(|message| Alert::new("Filtro não aplicado", message).tone(Tone::Warning));
    let (open_store, open_form) = (store.clone(), forms.transaction.clone());
    let export_store = store.clone();
    let scenarios = state.scenarios.clone();
    let view = session.frame().view();
    let list = view.map(|view| {
        ListView::new(session.clone(), session.frame(), view)
            .filter(rows.filter.clone())
            .sort("Data", false)
            .sort("Id", false)
            .single_selection()
            .plain_toolbar()
            .on_open(Rc::new(move |key: &Field| {
                if let Some(id) = id_of_field(key) {
                    open_form.start_edit(&open_store, id);
                }
            }))
            .column_shape(move |field, column| match field.name.as_str() {
                "Id" => GridColumn::number("Id").hidden(),
                "Data" => date_column("Data"),
                "Descrição" => GridColumn::text("Descrição").width(220.0).fill(),
                "Categoria" if category => GridColumn::text("Categoria").width(140.0),
                "Categoria" => GridColumn::text("Categoria").width(140.0).hidden(),
                "Conta" if all_columns => GridColumn::text("Conta").width(150.0),
                "Conta" => GridColumn::text("Conta").width(150.0).hidden(),
                "Membro" if all_columns => GridColumn::text("De quem").width(100.0),
                "Membro" => GridColumn::text("De quem").width(100.0).hidden(),
                // The sign and the color of the value already say it; the column chooser brings it back.
                "Tipo" => GridColumn::text("Tipo").width(96.0).hidden(),
                "Valor" => money_column("Valor").summary(Summary::Sum).template(signed_amount),
                _ => column,
            })
            .grid_shape(move |grid: DataGrid| {
                let saver = export_store.clone();
                grid.filter_row(false)
                    .empty_text("Nenhum lançamento neste mês. Comece em Nova despesa.")
                    .scenarios(scenarios.clone())
                    .on_export(move |csv| {
                        saver.save_file("lancamentos.csv", ("CSV", &["csv"]), csv.as_bytes());
                    })
            })
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
            Keyed::new(u64::from(all_columns) + u64::from(category) * 2, filling(list)),
            session.confirmations(),
        )),
    )
}
