//! Bills to pay: what is due, what is late, paying and undoing.

use std::cell::{Cell, OnceCell};
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    Action, ActionContext, Application, ApplicationBuilder, Criteria, CriteriaMode, Module, Selection,
    Value as Field, ViewController,
};
use ravenrs_app_ui::{ActionButton, ListView, Session, Words};

use super::entities;
use super::forms::Forms;
use super::help;
use super::store::{
    Memo, Page, Store, content_width, date_column, filling, follow_store, id_of_field, money_column,
    remembered_scenarios, screen,
};
use crate::model::BillStatus;
use crate::money::format_brl;

/// What the bills add up to.
struct BillTotals {
    overdue: i64,
    open: i64,
    open_count: usize,
}

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct BillsState {
    pub(crate) view: State<usize>,
    scenarios: Binding<Vec<GridScenario>>,
    totals: Memo<u64, Rc<BillTotals>>,
    session: Rc<OnceCell<Option<Session>>>,
    seen: Rc<Cell<u64>>,
}

impl BillsState {
    pub fn new(path: Option<&std::path::Path>) -> Self {
        Self {
            scenarios: remembered_scenarios(path, "contas"),
            view: State::new(0),
            totals: Memo::new(),
            session: Rc::default(),
            seen: Rc::default(),
        }
    }
}

const NEW: &str = "NewBill";
const EDIT: &str = "EditBill";
const DELETE: &str = "DeleteBill";
const PAY: &str = "PayBill";
const UNDO: &str = "UndoPayment";

fn selected_id(cx: &ActionContext) -> Option<i64> {
    cx.selected.first().and_then(|bill| id_of_field(&bill.get("id")))
}

/// What can be done to a bill. What was paid changes only by undoing the payment.
struct BillCommands {
    store: Store,
    forms: Forms,
}

impl ViewController for BillCommands {
    fn id(&self) -> &str {
        "BillCommands"
    }

    fn actions(&self) -> Vec<Action> {
        let (store, forms) = (self.store.clone(), self.forms.clone());
        let unpaid = "IsNull([paid_on])";
        let (new_store, new_form) = (store.clone(), forms.bill.clone());
        let (edit_store, edit_form) = (store.clone(), forms.bill.clone());
        let (pay_store, pay_form) = (store.clone(), forms.pay.clone());
        let (undo_store, gone) = (store.clone(), store);
        vec![
            Action::simple(UNDO, "Desfazer pagamento")
                .icon("undo-2")
                .selection(Selection::RequireSingle)
                .target_criteria("Not IsNull([paid_on])", CriteriaMode::TrueForAll)
                .confirmation("Desfazer este pagamento?")
                .confirmation_words(
                    "A conta volta a ficar em aberto e o lançamento da despesa é apagado.",
                    "Desfazer pagamento",
                )
                .on_execute(move |cx| {
                    if let Some(id) = selected_id(cx) {
                        undo_store.write("Pagamento desfeito", |db| db.reopen_bill(id));
                    }
                    Ok(())
                }),
            Action::simple(EDIT, "Alterar")
                .icon("pencil")
                .selection(Selection::RequireSingle)
                .target_criteria(unpaid, CriteriaMode::TrueForAll)
                .on_execute(move |cx| {
                    if let Some(id) = selected_id(cx) {
                        edit_form.start_edit(&edit_store, id);
                    }
                    Ok(())
                }),
            Action::simple(DELETE, "Excluir")
                .icon("trash-2")
                .selection(Selection::RequireSingle)
                .target_criteria(unpaid, CriteriaMode::TrueForAll)
                .confirmation("Excluir esta conta?")
                .confirmation_words("Esta ação não pode ser desfeita.", "Excluir conta")
                .on_execute(move |cx| {
                    if let Some(id) = selected_id(cx) {
                        gone.write("Conta excluída", |db| db.delete_bill(id));
                    }
                    Ok(())
                }),
            Action::simple(PAY, "Pagar")
                .icon("check")
                .selection(Selection::RequireSingle)
                .target_criteria(unpaid, CriteriaMode::TrueForAll)
                .on_execute(move |cx| {
                    if let Some(id) = selected_id(cx) {
                        pay_form.start(&pay_store, id);
                    }
                    Ok(())
                }),
            Action::simple(NEW, "Nova conta").icon("plus").on_execute(move |_| {
                new_form.start_new(&new_store);
                Ok(())
            }),
        ]
    }
}

struct BillsModule {
    store: Store,
    forms: Forms,
}

impl Module for BillsModule {
    fn name(&self) -> &str {
        "Bills"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(BillCommands { store: self.store.clone(), forms: self.forms.clone() })]
    }
}

fn make_session(store: &Store, forms: &Forms) -> Option<Session> {
    let database = entities::database(store)?;
    let application = database
        .entities()
        .into_iter()
        .fold(Application::builder("Contas a pagar"), ApplicationBuilder::entity)
        .module(BillsModule { store: store.clone(), forms: forms.clone() })
        .build()
        .map_err(|e| tracing::error!(error = %e, "the bills could not be put together"))
        .ok()?;
    let session = Session::embedded(application, move || database.space(), Words::pt_br());
    session.show_list("BillRow_ListView");
    Some(session)
}

fn totals(store: &Store) -> BillTotals {
    let today = store.today;
    let open = store.read(|db| db.open_bills());
    BillTotals {
        overdue: open.iter().filter(|b| b.status(today) == BillStatus::Overdue).map(|b| b.amount_cents).sum(),
        open: open.iter().map(|b| b.amount_cents).sum(),
        open_count: open.len(),
    }
}

/// Which bills each choice at the top of the screen shows.
fn filter_of(view: usize, today: Date) -> Option<Criteria> {
    let text = match view {
        0 => "IsNull([paid_on])".to_owned(),
        1 => format!("IsNull([paid_on]) And [due] < '{today}'"),
        2 => "Not IsNull([paid_on])".to_owned(),
        _ => return None,
    };
    Criteria::parse(&text).ok()
}

/// What a window narrower than each of these leaves out, so that the amount stays.
const ROOM_FOR_ALL_COLUMNS: f32 = 900.0;
const ROOM_FOR_THE_CATEGORY: f32 = 700.0;

/// How a bill stands, as something that stands out.
fn status_badge(value: &Value) -> AnyView {
    let text = if let Value::Text(text) = value { text.as_str() } else { "" };
    let tone = match text {
        t if t == BillStatus::Overdue.label() => Tone::Danger,
        t if t == BillStatus::DueToday.label() => Tone::Warning,
        t if t == BillStatus::Paid.label() => Tone::Success,
        _ => Tone::Neutral,
    };
    Box::new(Badge::new(text).tone(tone))
}

pub(crate) fn bills(store: &Store, state: &BillsState, forms: &Forms, window: Size) -> Page {
    let content = content_width(window);
    let all_columns = content >= ROOM_FOR_ALL_COLUMNS;
    let category = content >= ROOM_FOR_THE_CATEGORY;
    let session = state.session.get_or_init(|| make_session(store, forms));
    let Some(session) = session else {
        return Page::new(
            help::CONTAS,
            "",
            Alert::new("Contas a pagar", "Não foi possível abrir as contas.").tone(Tone::Danger),
        );
    };
    follow_store(store, session, &state.seen);
    let view = state.view.get();
    let rows = state.totals.get(store.version(), || Rc::new(totals(store)));
    let toolbar = HStack::new((
        SegmentedControl::new(state.view.binding(), ["Em aberto", "Vencidas", "Pagas", "Todas"])
            .label("Mostrar"),
        Spacer::new(),
        // At the start of the row, so nothing moves under the pointer when it comes and goes.
        ActionButton::new(session.clone(), UNDO).style(ButtonStyle::Ghost).hide_when_off(),
        ActionButton::new(session.clone(), EDIT).style(ButtonStyle::Ghost),
        ActionButton::new(session.clone(), DELETE).style(ButtonStyle::Ghost),
        ActionButton::new(session.clone(), PAY).style(ButtonStyle::Secondary),
        ActionButton::new(session.clone(), NEW),
    ))
    .spacing(8.0)
    .align(Align::Center)
    .wrap();

    // Only what was paid has a payment date; each view opens with its own columns.
    let shows_paid = view >= 2;
    let (open_store, open_form) = (store.clone(), forms.pay.clone());
    let scenarios = state.scenarios.clone();
    let list = session.frame().view().map(|info| {
        let list = ListView::new(session.clone(), session.frame(), info)
            .filter(filter_of(view, store.today))
            .single_selection()
            .plain_toolbar()
            .on_open(Rc::new(move |key: &Field| {
                let Some(id) = id_of_field(key) else { return };
                // What was already paid has nothing to pay.
                if open_store.read(|db| db.bill(id)).is_some_and(|bill| bill.paid_on.is_none()) {
                    open_form.start(&open_store, id);
                }
            }))
            .column_shape(move |field, column| match field.name.as_str() {
                "id" => GridColumn::number("Id").hidden(),
                "due" => date_column("Vencimento"),
                "title" => GridColumn::text("Descrição").width(200.0).fill(),
                "category" if category => GridColumn::text("Categoria").width(130.0),
                "category" => GridColumn::text("Categoria").width(130.0).hidden(),
                "member" if all_columns => GridColumn::text("De quem").width(100.0),
                "member" => GridColumn::text("De quem").width(100.0).hidden(),
                "recurrence" if all_columns => GridColumn::text("Repete").width(110.0),
                "recurrence" => GridColumn::text("Repete").width(110.0).hidden(),
                "status" => GridColumn::text("Situação").width(116.0).template(status_badge),
                "amount" => money_column("Valor").summary(Summary::Sum),
                "paid_on" if shows_paid => date_column("Paga em"),
                "paid_on" => date_column("Paga em").hidden(),
                _ => column,
            })
            .grid_shape(move |grid: DataGrid| {
                grid.filter_row(false)
                    .empty_text(match view {
                        1 => "Nenhuma conta vencida.",
                        2 => "Nenhuma conta paga ainda.",
                        _ => "Nenhuma conta por aqui. Cadastre a primeira em Nova conta.",
                    })
                    .scenarios(scenarios.clone())
            });
        // What was paid last comes first.
        if view == 2 {
            list.sort("due", false).sort("id", false)
        } else {
            list.sort("due", true).sort("id", true)
        }
    });

    Page::new(
        help::CONTAS,
        if rows.open_count == 0 {
            "Nenhuma conta em aberto.".to_owned()
        } else {
            format!(
                "{} em aberto somando {}, das quais {} em atraso. Clique duas vezes em uma conta para pagar.",
                rows.open_count,
                format_brl(rows.open),
                format_brl(rows.overdue)
            )
        },
        screen((
            toolbar,
            Keyed::new(
                u64::from(shows_paid) + u64::from(all_columns) * 2 + u64::from(category) * 4,
                filling(list),
            ),
            session.confirmations(),
        )),
    )
}
