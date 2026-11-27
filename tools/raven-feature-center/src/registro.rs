//! Object: a record of an entity, its values by field, and each one read in the type a screen,
//! a rule or a report needs (text, number, whole number, cents, date and key), empty alike when a
//! field is missing, holds another type or the person may not read it.
use std::collections::BTreeMap;

use ravenrs::prelude::*;
use ravenrs_app::{Object, Value};
use ravenrs_criteria::DateTime;

/// The control registered in the center.
pub const ID: &str = "ObjectDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Registro";

/// The bill the page starts from.
fn bill() -> Object {
    let mut values = BTreeMap::new();
    values.insert("description".to_owned(), Value::Text("Conta de luz".into()));
    values.insert("amount".to_owned(), Value::Number(18_990.4));
    values.insert("installments".to_owned(), Value::Number(3.0));
    values.insert("due".to_owned(), Value::Text("2026-12-10".into()));
    values.insert("account_id".to_owned(), Value::Number(7.0));
    values.insert("account_id.name".to_owned(), Value::Text("Conta corrente".into()));
    Object::from_values("Bill", values)
}

fn shown(bill: &Object) -> Vec<String> {
    let or = |value: Option<String>| value.unwrap_or_else(|| "vazio".to_owned());
    vec![
        format!("Entidade: {}", bill.entity()),
        format!("Campos: {}", bill.values().keys().cloned().collect::<Vec<_>>().join(", ")),
        format!(
            "Descrição (text): {}",
            if bill.text("description").is_empty() { "vazio".to_owned() } else { bill.text("description") }
        ),
        format!("Valor bruto (get): {:?}", bill.get("amount")),
        format!("Valor (number): {}", or(bill.number("amount").map(|n| n.to_string()))),
        format!("Valor em centavos (cents): {}", or(bill.cents("amount").map(|c| c.to_string()))),
        format!("Parcelas (integer): {}", or(bill.integer("installments").map(|n| n.to_string()))),
        format!("Vencimento (date): {}", or(bill.date("due").map(|d| d.to_string()))),
        format!("Conta (key): {}", or(bill.key("account_id"))),
    ]
}

/// Every public feature of Object on one page.
#[derive(Debug, Default)]
pub struct RecordPage;

impl View for RecordPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let record = cx.use_state(bill);
        let (pay, keep, plain, again) = (record.clone(), record.clone(), record.clone(), record.clone());
        let lines: Vec<Text> = shown(&record.get()).into_iter().map(Text::new).collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Um registro de uma entidade, lido campo a campo no tipo que a tela precisa. Campo ausente, de outro tipo ou que a pessoa não pode ler vem vazio.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    Button::new("Mudar o valor").on_click(move || {
                        pay.update(|bill| bill.set("amount", 20_000.0));
                    }),
                    Button::new("Só descrição e valor").on_click(move || {
                        keep.update(|bill| bill.keep_only(&["description", "amount"]));
                    }),
                    Button::new("Tirar os caminhos").on_click(move || {
                        plain.update(|bill| *bill = bill.clone().without_paths());
                    }),
                    Button::new("Recomeçar").on_click(move || again.set(bill())),
                ))
                .spacing(8.0),
                VStack::new(lines).spacing(4.0).align(Align::Start),
                Text::new(format!("Hoje do framework (local_today): {}", DateTime { seconds: 0, ..ravenrs_app::local_today() }))
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
