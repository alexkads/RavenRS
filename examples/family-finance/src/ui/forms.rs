//! The dialogs: transactions, bills, paying a bill and confirming. Their fields come from what
//! each record is, in `entities`; keeping them goes through the database and its rules.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{Object, Value as Field};
use ravenrs_app_ui::{EditorRegistry, EditorRequest, EntityForm, FormText};

use super::entities;
use super::request::Prefill;
use super::store::{Store, date_of_field, field_of_date, id_of_field};
use crate::model::{
    Account, AccountKind, BillInput, Category, Flow, Occurrence, Recurrence, ScheduleInput, TransactionInput,
    date_label,
};
use crate::money::{format_brl, split_installments};

/// What to do once, when a form opened for it saves. Opening the form for anything
/// else forgets it, so cancelling leaves whoever asked as it was.
#[derive(Clone, Default)]
struct AfterSave(Rc<RefCell<Option<Saved>>>);

type Saved = Rc<dyn Fn()>;

impl AfterSave {
    fn set(&self, then: Saved) {
        *self.0.borrow_mut() = Some(then);
    }

    fn forget(&self) {
        self.0.borrow_mut().take();
    }

    fn run(&self) {
        let then = self.0.borrow_mut().take();
        if let Some(then) = then {
            then();
        }
    }
}

/// The account most money moves through, which forms start on.
fn usual_account(accounts: &[Account]) -> usize {
    accounts.iter().position(|a| a.kind == AccountKind::Checking).unwrap_or(0)
}

fn categories_of(categories: &[Category], flow: Flow) -> Vec<Category> {
    categories.iter().filter(|c| c.flow == flow).cloned().collect()
}

/// The category most used in these names, as the form's first choice: the first in
/// alphabetical order would quietly put everything in the wrong place.
fn usual_category(categories: &[Category], used: impl Iterator<Item = String>) -> usize {
    let expenses = categories_of(categories, Flow::Expense);
    let mut counts = vec![0_usize; expenses.len()];
    for name in used {
        if let Some(index) = expenses.iter().position(|category| category.name == name) {
            counts[index] += 1;
        }
    }
    counts
        .iter()
        .enumerate()
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(*index)))
        .map_or(0, |(index, _)| index)
}

/// A form that is made the first time something needs it, once there are records to read.
type Lazy = Rc<OnceCell<Option<EntityForm>>>;

/// What goes in a record's field for a key that may not be there.
fn key_field(id: Option<i64>) -> Field {
    id.map_or(Field::Null, |id| Field::Number(id as f64))
}

fn cents_of(record: &Object, field: &str) -> i64 {
    match record.get(field) {
        Field::Number(cents) => cents.round() as i64,
        _ => 0,
    }
}

fn text_of(record: &Object, field: &str) -> String {
    match record.get(field) {
        Field::Text(text) => text,
        _ => String::new(),
    }
}

fn flow_of(record: &Object) -> Flow {
    if text_of(record, "flow") == Flow::Income.code() { Flow::Income } else { Flow::Expense }
}

/// The category most used in `used`, as the form's first choice.
fn usual_category_id(categories: &[Category], used: impl Iterator<Item = String>) -> Option<i64> {
    let at = usual_category(categories, used);
    categories_of(categories, Flow::Expense).get(at).map(|category| category.id)
}

fn usual_account_id(accounts: &[Account]) -> Option<i64> {
    accounts.get(usual_account(accounts)).map(|account| account.id)
}

/// A new transaction, or changing one.
#[derive(Clone, Default)]
pub(crate) struct TransactionForm {
    form: Lazy,
    after_save: AfterSave,
}

impl TransactionForm {
    pub fn new() -> Self {
        Self::default()
    }

    fn form(&self, store: &Store) -> Option<EntityForm> {
        let (keeper, after) = (store.clone(), self.after_save.clone());
        self.form
            .get_or_init(|| {
                entities::form("Transaction", store, EditorRegistry::default()).map(|form| {
                    form.on_save(move |record, is_new| {
                        let saved = save_transaction(&keeper, record, is_new);
                        if saved {
                            after.run();
                        }
                        saved
                    })
                })
            })
            .clone()
    }

    fn fill_new(store: &Store, flow: Flow, record: &mut Object) {
        let accounts = store.read(|db| db.accounts());
        let (categories, rows) = store.read(|db| Ok((db.categories()?, db.ledger(store.today, None)?)));
        record.set("flow", flow.code());
        record.set("date", field_of_date(store.today));
        record.set("account", key_field(usual_account_id(&accounts)));
        if flow == Flow::Expense {
            let usual = usual_category_id(&categories, rows.into_iter().map(|row| row.category));
            record.set("category", key_field(usual));
        }
    }

    pub fn start_new(&self, store: &Store, flow: Flow) {
        self.after_save.forget();
        let Some(form) = self.form(store) else { return };
        form.start_new(new_text(), |record| Self::fill_new(store, flow, record));
    }

    /// A new spending already filled in, for the person to check and save. `saved` runs
    /// if they do.
    pub fn start_with(&self, store: &Store, fill: &Prefill, saved: Rc<dyn Fn()>) {
        self.after_save.forget();
        let Some(form) = self.form(store) else { return };
        form.start_new(new_text(), |record| {
            Self::fill_new(store, Flow::Expense, record);
            record.set("description", fill.description.clone());
            if let Some(cents) = fill.cents {
                record.set("amount", cents as f64);
            }
            if let Some(date) = fill.date {
                record.set("date", field_of_date(date));
            }
            if let Some(category) = &fill.category {
                record.set("category", category.id as f64);
            }
            if let Some(member) = &fill.member {
                record.set("member", member.id as f64);
            }
            if let Some(account) = &fill.account {
                record.set("account", account.id as f64);
            }
        });
        self.after_save.set(saved);
    }

    pub fn start_edit(&self, store: &Store, id: i64) {
        self.after_save.forget();
        let Some(form) = self.form(store) else { return };
        form.start_edit(
            FormText::new("Alterar lançamento", "Salvar lançamento"),
            &Field::Number(id as f64),
            |_| {},
        );
    }

    #[cfg(test)]
    pub(crate) fn save(&self, store: &Store) {
        if let Some(form) = self.form(store) {
            form.save();
        }
    }

    pub fn view(&self, store: &Store) -> Option<EntityForm> {
        self.form(store)
    }
}

fn new_text() -> FormText {
    FormText::new("Novo lançamento", "Salvar lançamento")
}

/// Goes through the database, which keeps the rules about what a transaction does.
fn save_transaction(store: &Store, record: &Object, is_new: bool) -> bool {
    let (Some(date), Some(account_id), Some(category_id)) = (
        date_of_field(&record.get("date")),
        id_of_field(&record.get("account")),
        id_of_field(&record.get("category")),
    ) else {
        return false;
    };
    let input = TransactionInput {
        date,
        description: text_of(record, "description"),
        amount_cents: cents_of(record, "amount"),
        flow: flow_of(record),
        account_id,
        category_id,
        member_id: id_of_field(&record.get("member")),
    };
    let id = id_of_field(&record.get("id"));
    store
        .write(if is_new { "Lançamento salvo" } else { "Lançamento alterado" }, |db| match id {
            Some(id) if !is_new => db.update_transaction(id, &input),
            _ => db.add_transaction(&input).map(|_| ()),
        })
        .is_some()
}

/// The stepper of a new bill's parts, with what each part comes to.
fn installments_editor(request: &EditorRequest) -> AnyView {
    let (read, write) = (request.value.clone(), request.value.clone());
    let parts = Binding::new(
        move || if let Field::Number(parts) = read.get() { parts as i32 } else { 1 },
        move |parts: i32| write.set(Field::Number(f64::from(parts))),
    );
    let record = request.session.form_record();
    let count = record.as_ref().map_or(1, |record| cents_of(record, "installments").max(1));
    let total = record.as_ref().map_or(0, |record| cents_of(record, "amount"));
    let hint = if count > 1 {
        let shares = split_installments(total, count as u32);
        format!("{}x de {}", shares.len(), format_brl(shares.first().copied().unwrap_or(0)))
    } else {
        "Parcelas: 1, à vista".to_owned()
    };
    Box::new(
        HStack::new((
            Stepper::new(parts).range(1, 60).label("Parcelas"),
            Text::new(hint).style(TextStyle::Callout).foreground(Semantic::InkMuted),
        ))
        .spacing(12.0)
        .align(Align::Center),
    )
}

/// A new bill, or changing one.
#[derive(Clone, Default)]
pub(crate) struct BillForm {
    form: Lazy,
    after_save: AfterSave,
}

impl BillForm {
    pub fn new() -> Self {
        Self::default()
    }

    fn form(&self, store: &Store) -> Option<EntityForm> {
        let (keeper, after) = (store.clone(), self.after_save.clone());
        self.form
            .get_or_init(|| {
                let mut editors = EditorRegistry::default();
                editors.register("Installments", installments_editor);
                entities::form("Bill", store, editors).map(|form| {
                    form.captions(Rc::new(|field, record| {
                        let split = cents_of(record, "installments") > 1;
                        match field {
                            "amount" if split => Some("Valor total".to_owned()),
                            "due" if split => Some("Primeiro vencimento".to_owned()),
                            _ => None,
                        }
                    }))
                    .on_save(move |record, is_new| {
                        let saved = save_bill(&keeper, record, is_new);
                        if saved {
                            after.run();
                        }
                        saved
                    })
                })
            })
            .clone()
    }

    fn fill_new(store: &Store, record: &mut Object) {
        let (categories, bills) = store.read(|db| Ok((db.categories()?, db.open_bills()?)));
        record.set("due", field_of_date(store.today));
        let usual = usual_category_id(&categories, bills.into_iter().map(|bill| bill.category));
        record.set("category", key_field(usual));
    }

    pub fn start_new(&self, store: &Store) {
        self.after_save.forget();
        let Some(form) = self.form(store) else { return };
        form.start_new(bill_text(false), |record| Self::fill_new(store, record));
    }

    /// A new bill already filled in, for the person to check and save. `saved` runs if
    /// they do.
    pub fn start_with(&self, store: &Store, fill: &Prefill, saved: Rc<dyn Fn()>) {
        self.after_save.forget();
        let Some(form) = self.form(store) else { return };
        form.start_new(bill_text(false), |record| {
            Self::fill_new(store, record);
            record.set("description", fill.description.clone());
            if let Some(cents) = fill.cents {
                record.set("amount", cents as f64);
            }
            if let Some(date) = fill.date {
                record.set("due", field_of_date(date));
            }
            if let Some(category) = &fill.category {
                record.set("category", category.id as f64);
            }
            if let Some(member) = &fill.member {
                record.set("member", member.id as f64);
            }
            if let Some(recurrence) = fill.recurrence {
                record.set("recurrence", recurrence.code());
            }
            if let Some(installments) = fill.installments {
                record.set("installments", f64::from(installments));
            }
            if let Some(barcode) = &fill.barcode {
                record.set("barcode", barcode.clone());
            }
        });
        self.after_save.set(saved);
    }

    pub fn start_edit(&self, store: &Store, id: i64) {
        self.after_save.forget();
        let Some(form) = self.form(store) else { return };
        form.start_edit(bill_text(true), &Field::Number(id as f64), |record| {
            // The parts of a bill were decided when it was created.
            record.set("installments", 1.0);
        });
    }

    #[cfg(test)]
    pub(crate) fn save(&self, store: &Store) {
        if let Some(form) = self.form(store) {
            form.save();
        }
    }

    pub fn view(&self, store: &Store) -> Option<EntityForm> {
        self.form(store)
    }
}

fn bill_text(editing: bool) -> FormText {
    FormText::new(if editing { "Alterar conta" } else { "Nova conta a pagar" }, "Salvar conta")
}

/// Goes through the database, which splits a bill into its parts.
fn save_bill(store: &Store, record: &Object, is_new: bool) -> bool {
    let (Some(due), Some(category_id)) =
        (date_of_field(&record.get("due")), id_of_field(&record.get("category")))
    else {
        return false;
    };
    let code = text_of(record, "recurrence");
    let input = BillInput {
        description: text_of(record, "description"),
        category_id,
        member_id: id_of_field(&record.get("member")),
        amount_cents: cents_of(record, "amount"),
        due,
        recurrence: Recurrence::ALL.into_iter().find(|r| r.code() == code).unwrap_or(Recurrence::None),
        installments: cents_of(record, "installments").max(1) as u32,
        barcode: text_of(record, "barcode").chars().filter(char::is_ascii_digit).collect(),
    };
    let id = id_of_field(&record.get("id"));
    let parts = input.installments;
    store
        .write(
            match (is_new, parts > 1) {
                (false, _) => "Conta alterada",
                (true, true) => "Parcelas criadas",
                (true, false) => "Conta salva",
            },
            |db| match id {
                Some(id) if !is_new => db.update_bill(id, &input),
                _ => db.add_bill(&input).map(|_| ()),
            },
        )
        .is_some()
}

/// Paying a bill: from which account, when and how much.
#[derive(Clone, Default)]
pub(crate) struct PayForm {
    form: Lazy,
    bill: Rc<Cell<Option<i64>>>,
    after_save: AfterSave,
}

impl PayForm {
    pub fn new() -> Self {
        Self::default()
    }

    fn form(&self, store: &Store) -> Option<EntityForm> {
        let (keeper, bill, after) = (store.clone(), self.bill.clone(), self.after_save.clone());
        self.form
            .get_or_init(|| {
                entities::form("Payment", store, EditorRegistry::default()).map(|form| {
                    form.on_save(move |record, _| {
                        let (Some(id), Some(date), Some(account)) = (
                            bill.get(),
                            date_of_field(&record.get("date")),
                            id_of_field(&record.get("account")),
                        ) else {
                            return false;
                        };
                        let amount = cents_of(record, "amount");
                        let paid =
                            keeper.write("Conta paga", |db| db.pay_bill(id, account, date, amount)).is_some();
                        if paid {
                            after.run();
                        }
                        paid
                    })
                })
            })
            .clone()
    }

    /// Paying a bill, for the person to check. `paid` runs if they do.
    pub fn start_then(&self, store: &Store, id: i64, paid: Rc<dyn Fn()>) {
        self.start(store, id);
        self.after_save.set(paid);
    }

    pub fn start(&self, store: &Store, id: i64) {
        self.after_save.forget();
        let Some(bill) = store.read(|db| db.bill(id)) else { return };
        let Some(form) = self.form(store) else { return };
        self.bill.set(Some(id));
        let accounts = store.read(|db| db.accounts());
        let text = FormText::new("Pagar conta", "Confirmar pagamento").description(format!(
            "{}, com vencimento em {}.",
            bill.title(),
            date_label(bill.due)
        ));
        form.start_new(text, |record| {
            record.set("account", key_field(usual_account_id(&accounts)));
            record.set("date", field_of_date(store.today));
            record.set("amount", bill.amount_cents as f64);
        });
    }

    #[cfg(test)]
    pub(crate) fn save(&self, store: &Store) {
        if let Some(form) = self.form(store) {
            form.save();
        }
    }

    pub fn view(&self, store: &Store) -> Option<EntityForm> {
        self.form(store)
    }
}

/// The editor of how a plan repeats, kept as the text the database keeps it in.
fn recurrence_editor(request: &EditorRequest) -> AnyView {
    let (read, write) = (request.value.clone(), request.value.clone());
    let rule = Binding::new(
        move || match read.get() {
            Field::Text(text) => RecurrenceRule::parse(&text).unwrap_or_default(),
            _ => RecurrenceRule::default(),
        },
        move |rule: RecurrenceRule| write.set(Field::Text(rule.to_text())),
    );
    let first = request
        .session
        .form_record()
        .and_then(|record| date_of_field(&record.get("start")))
        .unwrap_or_else(|| Date::from_days(0));
    Box::new(RecurrenceEditor::new(rule, first))
}

/// How tall the fields of a plan are, inside their window.
const SCHEDULE_FIELDS_HEIGHT: f32 = 540.0;

/// Planning a payment or a receipt on future days.
#[derive(Clone, Default)]
pub(crate) struct ScheduleForm {
    form: Lazy,
}

impl ScheduleForm {
    pub fn new() -> Self {
        Self::default()
    }

    fn form(&self, store: &Store) -> Option<EntityForm> {
        let keeper = store.clone();
        self.form
            .get_or_init(|| {
                let mut editors = EditorRegistry::default();
                editors.register("Recurrence", recurrence_editor);
                entities::form("Schedule", store, editors).map(|form| {
                    form.scrolling_height(SCHEDULE_FIELDS_HEIGHT)
                        .on_save(move |record, is_new| save_schedule(&keeper, record, is_new))
                })
            })
            .clone()
    }

    /// A new plan, starting on this day and for this person.
    pub fn start_new(&self, store: &Store, start: Date, member_id: Option<i64>) {
        let Some(form) = self.form(store) else { return };
        let accounts = store.read(|db| db.accounts());
        form.start_new(FormText::new("Programar", "Salvar programação"), |record| {
            record.set("start", field_of_date(start));
            record.set("account", key_field(usual_account_id(&accounts)));
            record.set("member", key_field(member_id));
            record.set("rule", RecurrenceRule::default().to_text());
        });
    }

    pub fn start_edit(&self, store: &Store, id: i64) {
        let Some(form) = self.form(store) else { return };
        form.start_edit(
            FormText::new("Alterar programação", "Salvar programação"),
            &Field::Number(id as f64),
            |_| {},
        );
    }

    pub fn view(&self, store: &Store) -> Option<EntityForm> {
        self.form(store)
    }
}

/// Goes through the database, which keeps the days a plan already went through.
fn save_schedule(store: &Store, record: &Object, is_new: bool) -> bool {
    let (Some(start), Some(account_id), Some(category_id)) = (
        date_of_field(&record.get("start")),
        id_of_field(&record.get("account")),
        id_of_field(&record.get("category")),
    ) else {
        return false;
    };
    let input = ScheduleInput {
        description: text_of(record, "description"),
        flow: flow_of(record),
        amount_cents: cents_of(record, "amount"),
        account_id,
        category_id,
        member_id: id_of_field(&record.get("member")),
        start,
        rule: RecurrenceRule::parse(&text_of(record, "rule")).unwrap_or_default(),
    };
    let id = id_of_field(&record.get("id"));
    store
        .write("Programação salva", |db| match id {
            Some(id) if !is_new => db.update_schedule(id, &input),
            _ => db.add_schedule(&input).map(|_| ()),
        })
        .is_some()
}

/// Saying a planned day happened: from which account, when and how much.
#[derive(Clone, Default)]
pub(crate) struct ConfirmForm {
    form: Lazy,
    target: Rc<Cell<Option<(i64, Date)>>>,
}

impl ConfirmForm {
    pub fn new() -> Self {
        Self::default()
    }

    fn form(&self, store: &Store) -> Option<EntityForm> {
        let (keeper, target) = (store.clone(), self.target.clone());
        self.form
            .get_or_init(|| {
                entities::form("Confirmation", store, EditorRegistry::default()).map(|form| {
                    form.on_save(move |record, _| {
                        let (Some((id, day)), Some(when), Some(account)) = (
                            target.get(),
                            date_of_field(&record.get("date")),
                            id_of_field(&record.get("account")),
                        ) else {
                            return false;
                        };
                        let amount = cents_of(record, "amount");
                        keeper
                            .write("Confirmado", |db| db.confirm_occurrence(id, day, account, when, amount))
                            .is_some()
                    })
                })
            })
            .clone()
    }

    pub fn start(&self, store: &Store, occurrence: &Occurrence) {
        let Some(form) = self.form(store) else { return };
        let input = &occurrence.schedule.input;
        self.target.set(Some((occurrence.schedule_id, occurrence.day)));
        let verb = if input.flow == Flow::Income { "Confirmar recebimento" } else { "Confirmar pagamento" };
        let text = FormText::new(verb, verb).description(format!(
            "{}, programado para {}.",
            input.description,
            date_label(occurrence.day)
        ));
        form.start_new(text, |record| {
            record.set("account", input.account_id as f64);
            // Something confirmed ahead of its day happened today.
            record.set("date", field_of_date(occurrence.when.min(store.today)));
            record.set("amount", input.amount_cents as f64);
        });
    }

    pub fn view(&self, store: &Store) -> Option<EntityForm> {
        self.form(store)
    }
}

/// All the dialogs, for the screens that open them.
#[derive(Clone)]
pub(crate) struct Forms {
    pub transaction: TransactionForm,
    pub bill: BillForm,
    pub pay: PayForm,
    pub schedule: ScheduleForm,
    pub occurrence: ConfirmForm,
    pub confirm: ConfirmDialog,
}

impl Forms {
    pub fn new() -> Self {
        Self {
            transaction: TransactionForm::new(),
            bill: BillForm::new(),
            pay: PayForm::new(),
            schedule: ScheduleForm::new(),
            occurrence: ConfirmForm::new(),
            confirm: ConfirmDialog::new(),
        }
    }
}
