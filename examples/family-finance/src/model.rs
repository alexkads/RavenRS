//! The records the app keeps, and the rules that need no database.

use ravenrs::prelude::Date;
use ravenrs::widgets::days_in_month;

/// Whether money leaves or arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flow {
    /// Money spent.
    Expense,
    /// Money received.
    Income,
}

impl Flow {
    /// The value stored in the database.
    pub fn code(self) -> &'static str {
        match self {
            Self::Expense => "expense",
            Self::Income => "income",
        }
    }

    /// Reads the value stored in the database.
    pub fn from_code(code: &str) -> Self {
        if code == "income" { Self::Income } else { Self::Expense }
    }

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Expense => "Despesa",
            Self::Income => "Receita",
        }
    }
}

/// What kind of place an account is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountKind {
    /// A checking account.
    Checking,
    /// A savings account.
    Savings,
    /// Cash in hand.
    Cash,
    /// A credit card.
    Card,
}

impl AccountKind {
    /// Every kind, in the order forms offer them.
    pub const ALL: [Self; 4] = [Self::Checking, Self::Savings, Self::Cash, Self::Card];

    /// The value stored in the database.
    pub fn code(self) -> &'static str {
        match self {
            Self::Checking => "checking",
            Self::Savings => "savings",
            Self::Cash => "cash",
            Self::Card => "card",
        }
    }

    /// Reads the value stored in the database.
    pub fn from_code(code: &str) -> Self {
        Self::ALL.into_iter().find(|k| k.code() == code).unwrap_or(Self::Checking)
    }

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Checking => "Conta corrente",
            Self::Savings => "Poupança",
            Self::Cash => "Dinheiro",
            Self::Card => "Cartão de crédito",
        }
    }
}

/// How often a bill comes back after being paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recurrence {
    /// Only once.
    None,
    /// Every month.
    Monthly,
    /// Every year.
    Yearly,
}

impl Recurrence {
    /// Every option, in the order forms offer them.
    pub const ALL: [Self; 3] = [Self::None, Self::Monthly, Self::Yearly];

    /// The value stored in the database.
    pub fn code(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    /// Reads the value stored in the database.
    pub fn from_code(code: &str) -> Self {
        Self::ALL.into_iter().find(|r| r.code() == code).unwrap_or(Self::None)
    }

    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Não repete",
            Self::Monthly => "Todo mês",
            Self::Yearly => "Todo ano",
        }
    }
}

/// Where a bill stands on a given day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BillStatus {
    /// Already paid.
    Paid,
    /// Past its due date and unpaid.
    Overdue,
    /// Due today.
    DueToday,
    /// Due later.
    Upcoming,
}

impl BillStatus {
    /// The name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Paid => "Paga",
            Self::Overdue => "Vencida",
            Self::DueToday => "Vence hoje",
            Self::Upcoming => "A vencer",
        }
    }
}

/// A person in the family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// Primary key.
    pub id: i64,
    /// Name.
    pub name: String,
}

/// A place money is kept, with its balance worked out from the transactions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// Primary key.
    pub id: i64,
    /// Name.
    pub name: String,
    /// Kind.
    pub kind: AccountKind,
    /// The balance before the first transaction.
    pub opening_cents: i64,
    /// The balance now.
    pub balance_cents: i64,
}

/// What money was spent on or came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    /// Primary key.
    pub id: i64,
    /// Name.
    pub name: String,
    /// Expenses or income.
    pub flow: Flow,
    /// The monthly limit; zero when there is none.
    pub budget_cents: i64,
}

/// A transaction as a form fills it in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionInput {
    /// The day it happened.
    pub date: Date,
    /// What it was.
    pub description: String,
    /// The amount, always positive.
    pub amount_cents: i64,
    /// Expense or income.
    pub flow: Flow,
    /// The account it moved.
    pub account_id: i64,
    /// Its category.
    pub category_id: i64,
    /// Who it belongs to.
    pub member_id: Option<i64>,
}

/// A stored transaction with the names of what it refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    /// Primary key.
    pub id: i64,
    /// What the form filled in.
    pub input: TransactionInput,
    /// The account's name.
    pub account: String,
    /// The category's name.
    pub category: String,
    /// The member's name; empty when there is none.
    pub member: String,
}

/// A bill as a form fills it in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BillInput {
    /// What it is.
    pub description: String,
    /// Its category.
    pub category_id: i64,
    /// Who it belongs to.
    pub member_id: Option<i64>,
    /// The amount, or the total when split into installments.
    pub amount_cents: i64,
    /// The due date, or the first one when split into installments.
    pub due: Date,
    /// How often it comes back.
    pub recurrence: Recurrence,
    /// How many installments; one when it is not split.
    pub installments: u32,
    /// The digits of the payment slip's barcode; empty when there is none.
    pub barcode: String,
}

/// A stored bill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bill {
    /// Primary key.
    pub id: i64,
    /// What it is, without the installment number.
    pub description: String,
    /// Its category.
    pub category_id: i64,
    /// The category's name.
    pub category: String,
    /// Who it belongs to.
    pub member_id: Option<i64>,
    /// The member's name; empty when there is none.
    pub member: String,
    /// The amount.
    pub amount_cents: i64,
    /// The due date.
    pub due: Date,
    /// How often it comes back.
    pub recurrence: Recurrence,
    /// The day of the month it is meant to fall on, kept so a bill due on the 31st returns to it after February.
    pub anchor_day: u8,
    /// Which installment this is, from 1.
    pub installment: u32,
    /// How many installments there are.
    pub installments: u32,
    /// The digits of the payment slip's barcode.
    pub barcode: String,
    /// When it was paid.
    pub paid_on: Option<Date>,
    /// The transaction that paid it.
    pub transaction_id: Option<i64>,
}

impl Bill {
    /// Where the bill stands on `today`.
    pub fn status(&self, today: Date) -> BillStatus {
        if self.paid_on.is_some() {
            BillStatus::Paid
        } else if self.due < today {
            BillStatus::Overdue
        } else if self.due == today {
            BillStatus::DueToday
        } else {
            BillStatus::Upcoming
        }
    }

    /// The description with the installment number, such as `Geladeira (2/10)`.
    pub fn title(&self) -> String {
        if self.installments > 1 {
            format!("{} ({}/{})", self.description, self.installment, self.installments)
        } else {
            self.description.clone()
        }
    }
}

/// A month's totals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonthTotals {
    /// `YYYY-MM`.
    pub month: String,
    /// Money received.
    pub income_cents: i64,
    /// Money spent.
    pub expense_cents: i64,
}

/// What a category spent in a month against its limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetLine {
    /// The category's key.
    pub category_id: i64,
    /// The category's name.
    pub category: String,
    /// The monthly limit; zero when there is none.
    pub budget_cents: i64,
    /// What was spent.
    pub spent_cents: i64,
}

/// The first day of `month`'s month, and of the next one: a half-open range for queries.
pub fn month_range(month: Date) -> (Date, Date) {
    let first = month.first_of_month();
    (first, first.add_months(1))
}

/// The month's name in Portuguese, such as `novembro de 2026`.
pub fn month_label(month: Date) -> String {
    const NAMES: [&str; 12] = [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ];
    format!("{} de {}", NAMES[usize::from(month.month().clamp(1, 12)) - 1], month.year())
}

/// The month's short name, such as `nov/26`, for chart axes.
pub fn month_short(month: Date) -> String {
    const NAMES: [&str; 12] =
        ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
    format!("{}/{:02}", NAMES[usize::from(month.month().clamp(1, 12)) - 1], month.year().rem_euclid(100))
}

/// A date as Brazilians write it: `31/10/2026`.
pub fn date_label(date: Date) -> String {
    format!("{:02}/{:02}/{:04}", date.day(), date.month(), date.year())
}

/// Reads an ISO date (`2026-10-31`).
pub fn parse_date(text: &str) -> Option<Date> {
    let mut parts = text.get(..10)?.split('-');
    let (year, month, day) =
        (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    Date::new(year, month, day)
}

/// The day `months` months after `from`, on `anchor_day` or on the month's last day when it is shorter.
pub fn shift_months(from: Date, months: i32, anchor_day: u8) -> Date {
    let first = from.first_of_month().add_months(months);
    let day = anchor_day.clamp(1, days_in_month(first.year(), first.month()));
    Date::new(first.year(), first.month(), day).unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap_or_else(|| panic!("invalid date"))
    }

    #[test]
    fn a_bill_on_the_31st_returns_to_it_after_february() {
        let january = date(2026, 1, 31);
        let february = shift_months(january, 1, 31);
        assert_eq!(february, date(2026, 2, 28));
        assert_eq!(shift_months(february, 1, 31), date(2026, 3, 31));
    }

    #[test]
    fn labels_are_in_portuguese() {
        assert_eq!(month_label(date(2026, 3, 9)), "março de 2026");
        assert_eq!(month_short(date(2026, 11, 9)), "nov/26");
        assert_eq!(date_label(date(2026, 3, 9)), "09/03/2026");
        assert_eq!(parse_date("2026-03-09"), Some(date(2026, 3, 9)));
        assert_eq!(parse_date("nope"), None);
    }
}
