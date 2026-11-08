//! Bills: a new one, changing one, and paying one.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_app::{Object, Value as Field};
use ravenrs_app_ui::{EditorRegistry, EntityForm, FormText};

use super::{
    AfterSave, Lazy, cents_of, installments_editor, key_field, text_of, usual_account_id, usual_category_id,
};
use crate::model::{BillInput, Recurrence, date_label};
use crate::ui::entities;
use crate::ui::request::Prefill;
use crate::ui::store::{Store, date_of_field, field_of_date, id_of_field};

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
