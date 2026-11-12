//! What can be done with something of the agenda: the buttons on its row and its right-click menu.

use ravenrs::prelude::*;

use super::Item;
use crate::model::{Bill, Occurrence};
use crate::ui::forms::Forms;
use crate::ui::store::Store;

/// The buttons on the row of something of the agenda.
pub(super) fn item_actions(item: &Item, store: &Store, forms: &Forms) -> Vec<AnyView> {
    let title = item.title();
    match item {
        Item::Bill(bill) if bill.paid_on.is_none() => bill_actions(bill, &title, store, forms),
        Item::Bill(_) => Vec::new(),
        Item::Planned(occurrence) => planned_actions(occurrence, &title, store, forms),
    }
}

/// A bill still to pay is paid from its row.
fn bill_actions(bill: &Bill, title: &str, store: &Store, forms: &Forms) -> Vec<AnyView> {
    let (id, store, pay) = (bill.id, store.clone(), forms.pay.clone());
    vec![Box::new(
        Button::new("Pagar")
            .accessible_name(format!("Pagar {title}"))
            .size(ButtonSize::Small)
            .on_click(move || pay.start(&store, id)),
    )]
}

/// A day planned on a rule is confirmed, skipped or undone, and its rule changed or ended.
fn planned_actions(occurrence: &Occurrence, title: &str, store: &Store, forms: &Forms) -> Vec<AnyView> {
    let mut actions: Vec<AnyView> = Vec::new();
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
                            store.write("Confirmação desfeita", |db| db.undo_occurrence(transaction));
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
    actions
}

/// What a right click on something of the agenda offers, as the day beside it does.
pub(super) fn item_menu(item: &Item, store: &Store, forms: &Forms) -> Vec<MenuItem> {
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
