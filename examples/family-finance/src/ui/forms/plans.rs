//! Plans: payments and receipts on future days, and saying a planned day happened.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{Object, Value as Field};
use ravenrs_app_ui::{EditorRegistry, EditorRequest, EntityForm, FormText};

use super::{Lazy, cents_of, flow_of, key_field, text_of, usual_account_id};
use crate::model::{Flow, Occurrence, ScheduleInput, date_label};
use crate::ui::entities;
use crate::ui::store::{Store, date_of_field, field_of_date, id_of_field};

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
