//! What the app's records are, for the screens generated from them. `Database` keeps owning the
//! schema, the migrations and every write that has rules of its own: these tables are only read.

use ravenrs::prelude::*;
use ravenrs_app::{
    AppearanceRule, EntityInfo, EnumOption, FieldInfo, FieldKind, ModelError, Object, Rule, Value as Field,
};
use ravenrs_app_sqlite::{SqlWords, SqliteDatabase, TableMap};
use ravenrs_app_ui::{EditorRegistry, EntityForm, Words};

use super::store::Store;
use crate::model::{AccountKind, BillStatus, Flow, Recurrence, parse_date};
use crate::money::format_brl;

/// An account's balance, from its opening and everything that moved through it.
const BALANCE: &str = "accounts.opening_cents + COALESCE((SELECT SUM(CASE t.flow WHEN 'income' THEN t.amount_cents \
                       ELSE -t.amount_cents END) FROM transactions t WHERE t.account_id = accounts.id), 0)";

const POSITIVE: &str = "O valor precisa ser maior que zero.";

fn options<T: Copy>(all: &[T], code: fn(T) -> &'static str, label: fn(T) -> &'static str) -> FieldKind {
    FieldKind::Enum(all.iter().map(|item| EnumOption::new(code(*item), label(*item))).collect())
}

fn id_field() -> FieldInfo {
    FieldInfo::new("id", FieldKind::Integer).key()
}

fn name_field(placeholder: &str) -> FieldInfo {
    FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property().placeholder(placeholder)
}

fn reference(name: &str, target: &str, caption: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Reference(target.to_owned())).caption(caption).editor("Select")
}

fn money_field(name: &str, caption: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Money).caption(caption).placeholder("0,00")
}

/// A record's amount: there has to be one, and it is never zero.
fn amount_rules(entity: EntityInfo, field: &str) -> EntityInfo {
    entity.rule(Rule::required(field)).rule(Rule::range(field, Some(1.0), None).message(POSITIVE))
}

/// The table has no empty amounts, and people can leave one empty.
fn empty_is_zero(object: &mut Object, field: &str) {
    if object.get(field) == Field::Null {
        object.set(field, 0.0);
    }
}

fn trimmed_name(object: &mut Object) {
    if let Field::Text(name) = object.get("name") {
        object.set("name", name.trim());
    }
}

pub(crate) fn account_entity() -> EntityInfo {
    EntityInfo::new("Account")
        .caption("Contas")
        .field(id_field())
        .field(name_field("Ex.: Banco do bairro"))
        .field(
            FieldInfo::new("kind", options(&AccountKind::ALL, AccountKind::code, AccountKind::label))
                .caption("Tipo"),
        )
        .field(
            FieldInfo::new("opening", FieldKind::Money)
                .caption("Saldo inicial")
                .in_list(false)
                .placeholder("0,00"),
        )
        .field(FieldInfo::new("balance", FieldKind::Money).caption("Saldo").read_only().in_detail(false))
        .rule(Rule::required("name"))
}

pub(crate) fn category_entity() -> EntityInfo {
    EntityInfo::new("Category")
        .caption("Categorias")
        .field(id_field())
        .field(name_field("Ex.: Pets"))
        .field(
            FieldInfo::new("flow", options(&[Flow::Expense, Flow::Income], Flow::code, Flow::label))
                .caption("Tipo")
                .editor("Segmented"),
        )
        .field(FieldInfo::new("budget", FieldKind::Money).caption("Limite mensal").placeholder("Sem limite"))
        .rule(Rule::required("name"))
}

pub(crate) fn member_entity() -> EntityInfo {
    EntityInfo::new("Member")
        .caption("Família")
        .field(id_field())
        .field(name_field("Ex.: Maria"))
        .rule(Rule::required("name"))
}

/// An account as a choice when paying, which says how much it has.
fn account_choice_entity() -> EntityInfo {
    EntityInfo::new("AccountChoice")
        .field(id_field())
        .field(FieldInfo::new("label", FieldKind::Text).default_property())
        .field(FieldInfo::new("name", FieldKind::Text))
        .field(FieldInfo::new("balance", FieldKind::Money))
}

fn flow_field(captions: [&str; 2]) -> FieldInfo {
    FieldInfo::new(
        "flow",
        FieldKind::Enum(vec![
            EnumOption::new(Flow::Expense.code(), captions[0]),
            EnumOption::new(Flow::Income.code(), captions[1]),
        ]),
    )
    .caption("Tipo")
    .editor("Segmented")
    .wide()
}

fn description_field(placeholder: &str) -> FieldInfo {
    FieldInfo::new("description", FieldKind::Text)
        .caption("Descrição")
        .placeholder(placeholder)
        .autofocus()
        .wide()
}

fn member_field() -> FieldInfo {
    reference("member", "Member", "De quem").none_caption("Família toda")
}

fn transaction_entity() -> EntityInfo {
    amount_rules(
        EntityInfo::new("Transaction")
            .field(id_field())
            .field(flow_field(["Despesa", "Receita"]))
            .field(description_field("Ex.: Supermercado"))
            .field(money_field("amount", "Valor"))
            .field(FieldInfo::new("date", FieldKind::Date).caption("Data"))
            .field(reference("account", "Account", "Conta"))
            .field(reference("category", "Category", "Categoria").lookup_filter("[flow] = ?flow"))
            .field(member_field())
            .rule(Rule::required("description"))
            .rule(Rule::required("date")),
        "amount",
    )
}

fn bill_entity() -> EntityInfo {
    amount_rules(
        EntityInfo::new("Bill")
            .field(id_field())
            .field(description_field("Ex.: Energia elétrica"))
            .field(money_field("amount", "Valor"))
            .field(FieldInfo::new("due", FieldKind::Date).caption("Vencimento"))
            .field(
                reference("category", "Category", "Categoria")
                    .lookup_filter(format!("[flow] = '{}'", Flow::Expense.code())),
            )
            .field(member_field())
            .field(
                FieldInfo::new("recurrence", options(&Recurrence::ALL, Recurrence::code, Recurrence::label))
                    .caption("Repetição")
                    .editor("Segmented")
                    .wide(),
            )
            .field(
                FieldInfo::new("installments", FieldKind::Integer)
                    .caption("Parcelas")
                    .editor("Installments")
                    .default_value(1.0)
                    .wide(),
            )
            .field(
                FieldInfo::new("barcode", FieldKind::Text)
                    .caption("Código de barras do boleto")
                    .placeholder("Só os números; opcional")
                    .wide(),
            )
            .rule(Rule::required("description"))
            .rule(Rule::required("due"))
            .appearance(
                AppearanceRule::new("OneRepeatsOrSplits", ["recurrence"])
                    .criteria("[installments] > 1")
                    .visible(false),
            )
            // A bill is split when it is created, and each part is then a bill of its own.
            .appearance(
                AppearanceRule::new("SplitOnlyWhenNew", ["installments"])
                    .criteria("Not IsNull([id])")
                    .visible(false),
            ),
        "amount",
    )
}

fn schedule_entity() -> EntityInfo {
    amount_rules(
        EntityInfo::new("Schedule")
            .field(id_field())
            .field(flow_field(["Pagamento", "Recebimento"]))
            .field(description_field("Ex.: Aluguel da garagem"))
            .field(money_field("amount", "Valor"))
            .field(FieldInfo::new("start", FieldKind::Date).caption("Primeiro dia"))
            .field(reference("account", "Account", "Conta"))
            .field(reference("category", "Category", "Categoria").lookup_filter("[flow] = ?flow"))
            .field(member_field().wide())
            .field(FieldInfo::new("rule", FieldKind::Text).caption("Repetição").editor("Recurrence").wide())
            .rule(Rule::required("description"))
            .rule(Rule::required("start")),
        "amount",
    )
}

/// Paying a bill: kept by the bill, and not as a record of its own.
fn payment_entity() -> EntityInfo {
    amount_rules(
        EntityInfo::new("Payment")
            .form_columns(2)
            .field(id_field())
            .field(reference("account", "AccountChoice", "Pagar com").wide())
            .field(money_field("amount", "Valor pago").hint("Mude se houve juros ou desconto."))
            .field(FieldInfo::new("date", FieldKind::Date).caption("Pago em"))
            .rule(Rule::required("date")),
        "amount",
    )
}

/// Saying a planned day happened.
fn confirmation_entity() -> EntityInfo {
    amount_rules(
        EntityInfo::new("Confirmation")
            .form_columns(2)
            .field(id_field())
            .field(reference("account", "Account", "Conta").wide())
            .field(money_field("amount", "Valor"))
            .field(FieldInfo::new("date", FieldKind::Date).caption("Quando"))
            .rule(Rule::required("date")),
        "amount",
    )
}

/// The month's transactions, on the view that expressions typed by people run over, so the fields
/// here are what those expressions call them.
pub(crate) fn ledger_entry_entity() -> EntityInfo {
    let text = |name: &str| FieldInfo::new(name, FieldKind::Text);
    EntityInfo::new("LedgerEntry")
        .caption("Lançamentos")
        .field(FieldInfo::new("Id", FieldKind::Integer).key().in_list(true))
        .field(text("Data"))
        .field(text("Descrição").default_property())
        .field(text("Categoria"))
        .field(text("Conta"))
        .field(text("Membro"))
        .field(text("Tipo"))
        .field(FieldInfo::new("Valor", FieldKind::Decimal(2)))
}

/// A bill as a row of the list, which says how it stands.
pub(crate) fn bill_row_entity() -> EntityInfo {
    EntityInfo::new("BillRow")
        .caption("Contas a pagar")
        .field(FieldInfo::new("id", FieldKind::Integer).key().in_list(true))
        .field(FieldInfo::new("due", FieldKind::Text))
        .field(FieldInfo::new("title", FieldKind::Text).default_property())
        .field(FieldInfo::new("category", FieldKind::Reference("Category".into())))
        .field(FieldInfo::new("member", FieldKind::Reference("Member".into())))
        .field(FieldInfo::new("recurrence", options(&Recurrence::ALL, Recurrence::code, Recurrence::label)))
        .field(FieldInfo::new("status", FieldKind::Text))
        .field(FieldInfo::new("amount", FieldKind::Money))
        .field(FieldInfo::new("paid_on", FieldKind::Text))
        .field(FieldInfo::new("installment", FieldKind::Integer).in_list(false))
        .field(FieldInfo::new("installments", FieldKind::Integer).in_list(false))
}

/// How a bill stands today, by what its row holds.
fn bill_status(row: &Object, today: Date) -> BillStatus {
    let due = match row.get("due") {
        Field::Text(text) => parse_date(&text),
        _ => None,
    };
    if row.get("paid_on") != Field::Null {
        BillStatus::Paid
    } else if due.is_some_and(|due| due < today) {
        BillStatus::Overdue
    } else if due == Some(today) {
        BillStatus::DueToday
    } else {
        BillStatus::Upcoming
    }
}

/// The description with the installment number, such as `Geladeira (2/10)`.
fn bill_title(row: &Object) -> String {
    let text = match row.get("title") {
        Field::Text(text) => text,
        _ => String::new(),
    };
    match (row.get("installment"), row.get("installments")) {
        (Field::Number(part), Field::Number(parts)) if parts > 1.0 => {
            format!("{text} ({}/{})", part as i64, parts as i64)
        }
        _ => text,
    }
}

fn sql_words() -> SqlWords {
    let mut words = SqlWords::pt_br();
    // The same words as the rest of the app, which come from `DbError`.
    words.unique = "Já existe um registro com esse nome. Escolha outro.".to_owned();
    words.in_use = "Este registro está em uso. Remova antes o que depende dele.".to_owned();
    words.rule = "Os dados não passaram nas regras do banco. Revise o formulário.".to_owned();
    words
}

fn build_database(store: &Store) -> Result<SqliteDatabase, ModelError> {
    let today = store.today;
    let by_ids = |map: TableMap| {
        map.column("amount", "amount_cents")
            .column("account", "account_id")
            .column("category", "category_id")
            .column("member", "member_id")
    };
    Ok(SqliteDatabase::builder(store.db.connection())
        .words(sql_words())
        .mapped(
            account_entity(),
            TableMap::new("accounts")
                .column("opening", "opening_cents")
                .computed("balance", BALANCE)
                .before_save(|object| {
                    trimmed_name(object);
                    empty_is_zero(object, "opening");
                }),
        )?
        .mapped(
            category_entity(),
            TableMap::new("categories")
                .column("budget", "budget_cents")
                .before_save(|object| {
                    trimmed_name(object);
                    empty_is_zero(object, "budget");
                })
                // No limit reads as an empty field, and not as zero reais.
                .after_load(|object| {
                    if object.get("budget") == Field::Number(0.0) {
                        object.set("budget", Field::Null);
                    }
                }),
        )?
        .mapped(member_entity(), TableMap::new("members").before_save(trimmed_name))?
        .mapped(
            account_choice_entity(),
            TableMap::new("accounts").computed("label", "name").computed("balance", BALANCE).after_load(
                |object| {
                    if let (Field::Text(name), Field::Number(cents)) =
                        (object.get("name"), object.get("balance"))
                    {
                        object.set("label", format!("{name} ({})", format_brl(cents as i64)));
                    }
                },
            ),
        )?
        .mapped(transaction_entity(), by_ids(TableMap::new("transactions")))?
        .mapped(bill_entity(), by_ids(TableMap::new("bills")))?
        .mapped(schedule_entity(), by_ids(TableMap::new("schedules")))?
        .mapped(
            bill_row_entity(),
            by_ids(TableMap::new("bills"))
                .computed("title", "description")
                .computed("status", "''")
                .after_load(move |row| {
                    let title = bill_title(row);
                    row.set("title", title);
                    row.set("status", bill_status(row, today).label());
                }),
        )?
        .mapped(
            ledger_entry_entity(),
            // What went out counts against what came in, in the column and in its total.
            TableMap::new("ledger").after_load(|entry| {
                if entry.get("Tipo") != Field::Text(Flow::Income.label().to_owned())
                    && let Field::Number(value) = entry.get("Valor")
                {
                    entry.set("Valor", -value);
                }
            }),
        )?
        .build())
}

/// The records of the app, made once.
pub(crate) fn database(store: &Store) -> Option<SqliteDatabase> {
    store
        .records
        .get_or_init(|| {
            build_database(store)
                .map_err(|e| tracing::error!(error = %e, "the records could not be put together"))
                .ok()
        })
        .clone()
}

/// A form of an entity in a window, in Portuguese, knowing every record the app has.
pub(crate) fn form(entity: &str, store: &Store, editors: EditorRegistry) -> Option<EntityForm> {
    let database = database(store)?;
    let mut entities = database.entities();
    entities.extend([payment_entity(), confirmation_entity()]);
    let mut words = Words::pt_br();
    // The same words as the rest of the app's forms.
    words.rules.required = "Preencha este campo.".to_owned();
    EntityForm::new(entity, entities, move || database.space(), words, editors)
        .map_err(|e| tracing::error!(error = %e, "a form could not be put together"))
        .ok()
}
