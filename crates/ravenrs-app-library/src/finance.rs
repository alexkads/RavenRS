//! Bills to pay and to receive, with the cash book: settling a bill writes the entry in the same
//! commit, with interest, fine and discount; a settlement may be partial and is reversed by an
//! entry of the opposite sign, never by deleting; a bill is split in installments that add up to
//! the cent (#859).
//!
//! The rules hold wherever records are kept, with no trigger per database: the status of a bill
//! moves only by the ways its state machine declares, a bill paid, cancelled or split does not
//! change ([`StateMachine::locked`]), and an entry written by a settlement, or a settlement
//! reversed, is sealed.
//!
//! ```
//! use ravenrs_app::{MemoryDatabase, ObjectSpace};
//! use ravenrs_app_library::finance::{self, SettleInput};
//! use ravenrs_criteria::DateTime;
//!
//! let database = MemoryDatabase::new(finance::entities())?;
//! let mut space = database.space();
//! let mut bill = space.new_object(finance::COMMITMENT)?;
//! bill.set("description", "Luz");
//! bill.set("kind", "payable");
//! bill.set("amount", 10_000.0);
//! let today = DateTime::from_ymd(2026, 11, 21).expect("a date");
//! bill.set("due_on", ravenrs_app::Value::Date(today));
//! let key = bill.get("id");
//! space.insert(bill)?;
//! space.commit()?;
//! finance::settle(&mut space, &key, SettleInput::new(today, 10_000).interest(300).fine(200).discount(100), today)?;
//! let paid = space.find(finance::COMMITMENT, &key)?.expect("the bill");
//! assert_eq!(paid.text("status"), "paid");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use ravenrs_app::{
    EntityInfo, EnumOption, FieldInfo, FieldKind, Object, ObjectSpace, Query, Seal, SpaceError, StateMachine,
    Value, key_text,
};
use ravenrs_criteria::{DateTime, Remainder, allocate};

/// The entity of the categories of money in and out.
pub const FINANCE_CATEGORY: &str = "FinanceCategory";
/// The entity of the cash book.
pub const LEDGER_ENTRY: &str = "LedgerEntry";
/// The entity of the bills to pay and to receive.
pub const COMMITMENT: &str = "Commitment";
/// The entity of the settlements of bills.
pub const SETTLEMENT: &str = "Settlement";

/// The most installments a bill is split in.
pub const MOST_INSTALLMENTS: u32 = 120;

fn key() -> FieldInfo {
    FieldInfo::new("id", FieldKind::Integer).key()
}

fn money(name: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Money)
}

fn reference(name: &str, target: &str) -> FieldInfo {
    FieldInfo::new(name, FieldKind::Reference(target.to_owned()))
}

fn options(pairs: &[(&str, &str)]) -> FieldKind {
    FieldKind::Enum(pairs.iter().map(|(code, caption)| EnumOption::new(*code, *caption)).collect())
}

/// A category of money in, out, or both.
pub fn finance_category() -> EntityInfo {
    EntityInfo::new(FINANCE_CATEGORY)
        .caption("Financial categories")
        .icon("tag")
        .field(key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new(
            "kind",
            options(&[("income", "Income"), ("expense", "Expense"), ("both", "Both")]),
        ))
        .field(FieldInfo::new("notes", FieldKind::LongText))
        .rule(ravenrs_app::Rule::required("name"))
        .rule(ravenrs_app::Rule::unique("name"))
}

/// A line of the cash book. One written by a settlement is sealed: it is reversed, not changed.
pub fn ledger_entry() -> EntityInfo {
    EntityInfo::new(LEDGER_ENTRY)
        .caption("Cash book")
        .icon("book")
        .field(key())
        .field(FieldInfo::new("on", FieldKind::Date).caption("Date"))
        .field(FieldInfo::new("description", FieldKind::Text).default_property())
        .field(FieldInfo::new("kind", options(&[("income", "Income"), ("expense", "Expense")])))
        .field(reference("category_id", FINANCE_CATEGORY).caption("Category"))
        .field(money("amount"))
        .field(reference("settlement_id", SETTLEMENT).caption("Settlement").read_only())
        .field(reference("reverses_id", LEDGER_ENTRY).caption("Reverses").read_only())
        .sealed(Seal::when("Not IsNull([settlement_id])"))
}

/// The ways the status of a bill moves, each only while what was paid agrees with it.
fn commitment_states() -> StateMachine {
    StateMachine::new("status")
        .state("open", "Open")
        .state("partial", "Partly paid")
        .state("paid", "Paid")
        .state("cancelled", "Cancelled")
        .state("split", "Split")
        .start("open")
        .transition("open", "partial", "Settle part")
        .when("[settled] > 0 And [settled] < [amount]")
        .transition("open", "paid", "Settle")
        .when("[settled] = [amount]")
        .transition("open", "cancelled", "Cancel")
        .when("[settled] = 0")
        .transition("open", "split", "Split")
        .when("[settled] = 0")
        .transition("partial", "paid", "Settle")
        .when("[settled] = [amount]")
        .transition("partial", "open", "Reverse")
        .when("[settled] = 0")
        .transition("partial", "cancelled", "Cancel")
        .transition("paid", "partial", "Reverse")
        .when("[settled] > 0 And [settled] < [amount]")
        .transition("paid", "open", "Reverse")
        .when("[settled] = 0")
        .transition("split", "cancelled", "Cancel")
        .locked("paid")
        .locked("cancelled")
        .locked("split")
        .unlocked_with(["settled"])
}

/// A bill to pay or to receive, which may be an installment of another.
pub fn commitment() -> EntityInfo {
    EntityInfo::new(COMMITMENT)
        .caption("Bills")
        .icon("receipt")
        .field(key())
        .field(FieldInfo::new("description", FieldKind::Text).default_property())
        .field(FieldInfo::new("kind", options(&[("payable", "To pay"), ("receivable", "To receive")])))
        .field(reference("category_id", FINANCE_CATEGORY).caption("Category"))
        .field(FieldInfo::new("counterparty", FieldKind::Text))
        .field(money("amount"))
        .field(FieldInfo::new("due_on", FieldKind::Date).caption("Due"))
        .field(
            FieldInfo::new(
                "status",
                options(&[
                    ("open", "Open"),
                    ("partial", "Partly paid"),
                    ("paid", "Paid"),
                    ("cancelled", "Cancelled"),
                    ("split", "Split"),
                ]),
            )
            .default_value("open"),
        )
        .field(money("settled").caption("Settled").read_only().default_value(0.0))
        .field(money("balance").caption("Balance").calculated("[amount] - [settled]"))
        .field(reference("origin_id", COMMITMENT).caption("Split from").read_only())
        .field(FieldInfo::new("installment", FieldKind::Integer).read_only())
        .field(FieldInfo::new("installments", FieldKind::Integer).read_only())
        .rule(ravenrs_app::Rule::required("description"))
        .state_machine(commitment_states())
}

/// A settlement of a bill, with what was added and taken off. Once reversed it is sealed.
pub fn settlement() -> EntityInfo {
    EntityInfo::new(SETTLEMENT)
        .caption("Settlements")
        .icon("check")
        .field(key())
        .field(reference("commitment_id", COMMITMENT).caption("Bill").read_only())
        .field(FieldInfo::new("on", FieldKind::Date).caption("Date").read_only())
        .field(money("principal").read_only())
        .field(money("interest").read_only())
        .field(money("fine").read_only())
        .field(money("discount").read_only())
        .field(money("total").calculated("[principal] + [interest] + [fine] - [discount]"))
        .field(reference("entry_id", LEDGER_ENTRY).caption("Entry").read_only())
        .field(FieldInfo::new("reversed_on", FieldKind::Date).caption("Reversed on").read_only())
        .field(
            FieldInfo::new("reverse_reason", FieldKind::LongText).caption("Why it was reversed").read_only(),
        )
        .sealed(Seal::when("Not IsNull([reversed_on])"))
}

/// Every entity of the finances, for a database to know.
pub fn entities() -> Vec<EntityInfo> {
    vec![finance_category(), ledger_entry(), commitment(), settlement()]
}

/// What a settlement takes: the date, how much of the bill, and what is added or taken off, in
/// cents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SettleInput {
    /// When it was paid or received.
    pub on: DateTime,
    /// How much of the bill it settles.
    pub principal: i64,
    /// Interest added.
    pub interest: i64,
    /// Fine added.
    pub fine: i64,
    /// Discount taken off.
    pub discount: i64,
}

impl SettleInput {
    /// Settles `principal` cents of the bill on this date, with nothing added or taken off.
    pub fn new(on: DateTime, principal: i64) -> Self {
        Self { on, principal, interest: 0, fine: 0, discount: 0 }
    }

    /// With this interest, in cents.
    pub fn interest(mut self, cents: i64) -> Self {
        self.interest = cents;
        self
    }

    /// With this fine, in cents.
    pub fn fine(mut self, cents: i64) -> Self {
        self.fine = cents;
        self
    }

    /// With this discount, in cents.
    pub fn discount(mut self, cents: i64) -> Self {
        self.discount = cents;
        self
    }

    /// What goes to the cash book.
    pub fn total(&self) -> i64 {
        self.principal + self.interest + self.fine - self.discount
    }
}

fn refused(why: impl Into<String>) -> SpaceError {
    SpaceError::Refused(why.into())
}

fn cents(record: &Object, field: &str) -> i64 {
    record.cents(field).unwrap_or(0)
}

fn found(space: &dyn ObjectSpace, entity: &str, key: &Value) -> Result<Object, SpaceError> {
    space.find(entity, key)?.ok_or_else(|| refused(format!("there is no {entity} {key:?}")))
}

/// Commits what waits, or throws it all away.
fn kept(space: &mut dyn ObjectSpace) -> Result<(), SpaceError> {
    space.commit().inspect_err(|_| space.rollback())
}

fn status_after(settled: i64, amount: i64) -> &'static str {
    if settled <= 0 {
        "open"
    } else if settled >= amount {
        "paid"
    } else {
        "partial"
    }
}

/// Settles a bill, or part of it: the settlement, the cash book entry of the total and the new
/// status of the bill, in one commit. A date after `today`, an amount below zero, a discount that
/// leaves nothing or more than what is open are refused, and nothing is written. Gives the
/// settlement.
pub fn settle(
    space: &mut dyn ObjectSpace,
    bill: &Value,
    input: SettleInput,
    today: DateTime,
) -> Result<Object, SpaceError> {
    let mut commitment = found(space, COMMITMENT, bill)?;
    let status = commitment.text("status");
    if status != "open" && status != "partial" {
        return Err(refused(format!("a bill that is {status} is not settled")));
    }
    if input.on.days > today.days {
        return Err(refused("the settlement date cannot be after today"));
    }
    if [input.principal, input.interest, input.fine, input.discount].iter().any(|cents| *cents < 0) {
        return Err(refused("no amount of a settlement is below zero"));
    }
    if input.principal == 0 {
        return Err(refused("a settlement settles something"));
    }
    let amount = cents(&commitment, "amount");
    let settled = cents(&commitment, "settled");
    if input.principal > amount - settled {
        return Err(refused("a settlement is not more than what is open"));
    }
    if input.total() <= 0 {
        return Err(refused("the discount leaves nothing to settle"));
    }
    let mut settlement = space.new_object(SETTLEMENT)?;
    let mut entry = space.new_object(LEDGER_ENTRY)?;
    settlement.set("commitment_id", bill.clone());
    settlement.set("on", Value::Date(input.on));
    settlement.set("principal", input.principal as f64);
    settlement.set("interest", input.interest as f64);
    settlement.set("fine", input.fine as f64);
    settlement.set("discount", input.discount as f64);
    settlement.set("entry_id", entry.get("id"));
    entry.set("on", Value::Date(input.on));
    entry.set("description", format!("Settlement of {}", commitment.text("description")));
    let kind = if commitment.text("kind") == "receivable" { "income" } else { "expense" };
    entry.set("kind", kind);
    entry.set("category_id", commitment.get("category_id"));
    entry.set("amount", input.total() as f64);
    entry.set("settlement_id", settlement.get("id"));
    let now_settled = settled + input.principal;
    commitment.set("settled", now_settled as f64);
    commitment.set("status", status_after(now_settled, amount));
    let written = space
        .insert(settlement.clone())
        .and_then(|()| space.insert(entry))
        .and_then(|()| space.save(commitment))
        .and_then(|()| kept(space));
    if written.is_err() {
        space.rollback();
    }
    written.map(|()| settlement)
}

/// Reverses a settlement, with a reason: an entry of the opposite sign that points to the first,
/// the settlement marked reversed and the bill open again for what it settled, in one commit.
/// Nothing is deleted.
pub fn reverse(
    space: &mut dyn ObjectSpace,
    settlement: &Value,
    reason: &str,
    today: DateTime,
) -> Result<(), SpaceError> {
    if reason.trim().is_empty() {
        return Err(refused("a reversal says why"));
    }
    let mut kept_settlement = found(space, SETTLEMENT, settlement)?;
    if !matches!(kept_settlement.get("reversed_on"), Value::Null) {
        return Err(refused("this settlement is reversed already"));
    }
    let original = found(space, LEDGER_ENTRY, &kept_settlement.get("entry_id"))?;
    let bill = kept_settlement.get("commitment_id");
    let mut commitment = found(space, COMMITMENT, &bill)?;
    let mut back = space.new_object(LEDGER_ENTRY)?;
    back.set("on", Value::Date(today));
    back.set("description", format!("Reversal of {}", original.text("description")));
    back.set("kind", original.get("kind"));
    back.set("category_id", original.get("category_id"));
    back.set("amount", -(cents(&original, "amount") as f64));
    back.set("settlement_id", settlement.clone());
    back.set("reverses_id", original.get("id"));
    kept_settlement.set("reversed_on", Value::Date(today));
    kept_settlement.set("reverse_reason", reason.trim());
    let now_settled = cents(&commitment, "settled") - cents(&kept_settlement, "principal");
    commitment.set("settled", now_settled as f64);
    commitment.set("status", status_after(now_settled, cents(&commitment, "amount")));
    let written = space
        .insert(back)
        .and_then(|()| space.save(kept_settlement))
        .and_then(|()| space.save(commitment))
        .and_then(|()| kept(space));
    if written.is_err() {
        space.rollback();
    }
    written
}

/// The same day `months` months after `first`, or the last day of a shorter month.
fn months_after(first: DateTime, months: u32) -> DateTime {
    let (year, month, day) = first.ymd();
    let index = i64::from(year) * 12 + i64::from(month) - 1 + i64::from(months);
    let (year, month) =
        (i32::try_from(index.div_euclid(12)).unwrap_or(year), (index.rem_euclid(12) + 1) as u32);
    (1..=day).rev().find_map(|day| DateTime::from_ymd(year, month, day)).unwrap_or(first)
}

/// What a split would give, without writing: the due date and amount of each installment, adding
/// up exactly to `amount`, with what is left over in the last.
pub fn plan(amount: i64, installments: u32, first_due: DateTime) -> Vec<(DateTime, i64)> {
    allocate(amount, installments, Remainder::Last)
        .into_iter()
        .enumerate()
        .map(|(at, cents)| (months_after(first_due, u32::try_from(at).unwrap_or(0)), cents))
        .collect()
}

/// Splits an open bill with nothing settled in installments from `first_due`, one a month, as
/// [`plan`] gives them; the bill becomes split and leaves what is open. Gives the installments.
pub fn split(
    space: &mut dyn ObjectSpace,
    bill: &Value,
    installments: u32,
    first_due: DateTime,
) -> Result<Vec<Object>, SpaceError> {
    if !(2..=MOST_INSTALLMENTS).contains(&installments) {
        return Err(refused(format!("a bill is split in 2 to {MOST_INSTALLMENTS} installments")));
    }
    let mut origin = found(space, COMMITMENT, bill)?;
    if origin.text("status") != "open" || cents(&origin, "settled") != 0 {
        return Err(refused("only an open bill with nothing settled is split"));
    }
    let mut made = Vec::new();
    let description = origin.text("description");
    for (at, (due, cents)) in plan(cents(&origin, "amount"), installments, first_due).into_iter().enumerate()
    {
        let mut part = space.new_object(COMMITMENT)?;
        part.set("description", format!("{description} ({}/{installments})", at + 1));
        part.set("kind", origin.get("kind"));
        part.set("category_id", origin.get("category_id"));
        part.set("counterparty", origin.get("counterparty"));
        part.set("amount", cents as f64);
        part.set("due_on", Value::Date(due));
        part.set("status", "open");
        part.set("settled", 0.0);
        part.set("origin_id", bill.clone());
        part.set("installment", (at + 1) as f64);
        part.set("installments", f64::from(installments));
        made.push(part);
    }
    origin.set("status", "split");
    let mut written = Ok(());
    for part in &made {
        written = written.and_then(|()| space.insert(part.clone()));
    }
    let written = written.and_then(|()| space.save(origin)).and_then(|()| kept(space));
    if written.is_err() {
        space.rollback();
    }
    written.map(|()| made)
}

/// Cancels a bill with nothing settled; one that was split takes its installments still open
/// with it, and leaves the others as they are. In one commit.
pub fn cancel(space: &mut dyn ObjectSpace, bill: &Value) -> Result<(), SpaceError> {
    let mut origin = found(space, COMMITMENT, bill)?;
    let was = origin.text("status");
    origin.set("status", "cancelled");
    let mut written = space.save(origin);
    if was == "split" {
        for mut part in space.query(COMMITMENT, &Query::all())? {
            if part.key("origin_id") == key_text(bill) && part.text("status") == "open" {
                part.set("status", "cancelled");
                written = written.and_then(|()| space.save(part));
            }
        }
    }
    let written = written.and_then(|()| kept(space));
    if written.is_err() {
        space.rollback();
    }
    written
}

/// What is open, in cents, at a day: to pay and to receive, and how much of each is overdue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Position {
    /// Open to pay.
    pub payable: i64,
    /// Open to receive.
    pub receivable: i64,
    /// Of what is to pay, what was due before the day.
    pub overdue_payable: i64,
    /// Of what is to receive, what was due before the day.
    pub overdue_receivable: i64,
}

/// What is open at `today`, as the space shows the bills: what is left of each bill open or partly
/// paid. A split bill counts by its installments, never twice.
pub fn position(space: &dyn ObjectSpace, today: DateTime) -> Result<Position, SpaceError> {
    let mut found = Position::default();
    for bill in space.query(COMMITMENT, &Query::all())? {
        let status = bill.text("status");
        if status != "open" && status != "partial" {
            continue;
        }
        let left = cents(&bill, "amount") - cents(&bill, "settled");
        let overdue = matches!(bill.get("due_on"), Value::Date(due) if due.days < today.days);
        if bill.text("kind") == "receivable" {
            found.receivable += left;
            if overdue {
                found.overdue_receivable += left;
            }
        } else {
            found.payable += left;
            if overdue {
                found.overdue_payable += left;
            }
        }
    }
    Ok(found)
}

/// A line of a [`Statement`]: the entry, with money in as more and out as less, and the balance
/// after it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct StatementLine {
    /// When.
    pub on: DateTime,
    /// What.
    pub description: String,
    /// In cents: more for what came in, less for what went out.
    pub amount: i64,
    /// The balance after it.
    pub balance: i64,
}

/// The cash book between two days: the balance before, each line with the balance after it, and
/// the balance at the end.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Statement {
    /// The balance of everything before `from`.
    pub opening: i64,
    /// The entries from `from` to `to`, by date and then as written.
    pub lines: Vec<StatementLine>,
    /// The balance at the end.
    pub closing: i64,
}

/// The cash book from `from` to `to`, both included, as the space shows it: whom may not read it
/// gets an empty one.
pub fn statement(space: &dyn ObjectSpace, from: DateTime, to: DateTime) -> Result<Statement, SpaceError> {
    let mut entries: Vec<(i64, i64, String, i64)> = space
        .query(LEDGER_ENTRY, &Query::all())?
        .into_iter()
        .filter_map(|entry| {
            let Value::Date(on) = entry.get("on") else { return None };
            let amount = cents(&entry, "amount");
            let signed = if entry.text("kind") == "income" { amount } else { -amount };
            let order = entry.number("id").map_or(0, |id| id as i64);
            Some((on.days, order, entry.text("description"), signed))
        })
        .collect();
    entries.sort();
    let opening: i64 = entries.iter().filter(|(day, ..)| *day < from.days).map(|(.., amount)| amount).sum();
    let mut balance = opening;
    let lines = entries
        .into_iter()
        .filter(|(day, ..)| (from.days..=to.days).contains(day))
        .map(|(day, _, description, amount)| {
            balance += amount;
            StatementLine { on: DateTime { days: day, seconds: 0 }, description, amount, balance }
        })
        .collect();
    Ok(Statement { opening, lines, closing: balance })
}

/// What came in and went out in a month of a year, in cents, by category.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MonthTotals {
    /// The month, from 1 to 12.
    pub month: u32,
    /// What came in.
    pub income: i64,
    /// What went out.
    pub expense: i64,
    /// By the key of the category, as text, what came in less what went out; empty for none.
    pub by_category: std::collections::BTreeMap<String, i64>,
}

/// The cash book of a year, month by month, as the space shows it: twelve months, the empty ones
/// with zeros.
pub fn annual(space: &dyn ObjectSpace, year: i32) -> Result<Vec<MonthTotals>, SpaceError> {
    let mut months: Vec<MonthTotals> =
        (1..=12).map(|month| MonthTotals { month, ..MonthTotals::default() }).collect();
    for entry in space.query(LEDGER_ENTRY, &Query::all())? {
        let Value::Date(on) = entry.get("on") else { continue };
        let (at, month, _) = on.ymd();
        if at != year {
            continue;
        }
        let Some(totals) = months.get_mut(month as usize - 1) else { continue };
        let amount = cents(&entry, "amount");
        let income = entry.text("kind") == "income";
        if income {
            totals.income += amount;
        } else {
            totals.expense += amount;
        }
        let category = entry.key("category_id").unwrap_or_default();
        *totals.by_category.entry(category).or_default() += if income { amount } else { -amount };
    }
    Ok(months)
}

/// What falls due in a month: the bills still open, by due date, and of them, those already
/// overdue at `today`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Forecast {
    /// The open bills due in the month, by due date.
    pub due: Vec<Object>,
    /// What is left of them to pay, in cents.
    pub payable: i64,
    /// What is left of them to receive, in cents.
    pub receivable: i64,
    /// How many of them were due before `today`.
    pub overdue: usize,
}

/// The open bills due in the month of `month` (any day of it), as the space shows them.
pub fn forecast(space: &dyn ObjectSpace, month: DateTime, today: DateTime) -> Result<Forecast, SpaceError> {
    let (year, number, _) = month.ymd();
    let mut found = Forecast::default();
    let mut due: Vec<(i64, Object)> = space
        .query(COMMITMENT, &Query::all())?
        .into_iter()
        .filter(|bill| matches!(bill.text("status").as_str(), "open" | "partial"))
        .filter_map(|bill| {
            let Value::Date(on) = bill.get("due_on") else { return None };
            let (y, m, _) = on.ymd();
            (y == year && m == number).then_some((on.days, bill))
        })
        .collect();
    due.sort_by_key(|(day, _)| *day);
    for (day, bill) in due {
        let left = cents(&bill, "amount") - cents(&bill, "settled");
        if bill.text("kind") == "receivable" {
            found.receivable += left;
        } else {
            found.payable += left;
        }
        if day < today.days {
            found.overdue += 1;
        }
        found.due.push(bill);
    }
    Ok(found)
}

/// Whether a category of this kind serves a movement of this kind: `income` and `receivable` go
/// with income, `expense` and `payable` with expense, and `both` with either.
fn serves(category: &str, movement: &str) -> bool {
    let wants = if matches!(movement, "income" | "receivable") { "income" } else { "expense" };
    category == "both" || category == wants
}

/// A watcher for a unit of work that keeps the categories right, by what `stored` reads: a bill or
/// an entry with a category of the other kind is refused, and so is deleting a category in use or
/// changing it to a kind its movements do not have. Put it on each space with `on_event`.
pub fn category_guard(stored: ravenrs_app::SpaceMaker) -> ravenrs_app::SpaceWatcher {
    use ravenrs_app::{Change, SpaceEvent};
    Box::new(move |event| {
        let SpaceEvent::Committing(changes) = event else { return Ok(()) };
        let reader = stored();
        let kind_of = |key: &Value| -> Option<String> {
            let pending = changes.iter().rev().find_map(|change| match change {
                Change::New(object) | Change::Updated(object)
                    if object.entity() == FINANCE_CATEGORY && object.get("id") == *key =>
                {
                    Some(object.text("kind"))
                }
                _ => None,
            });
            pending
                .or_else(|| reader.find(FINANCE_CATEGORY, key).ok().flatten().map(|found| found.text("kind")))
        };
        let in_use = |key: &Value| -> Vec<String> {
            [COMMITMENT, LEDGER_ENTRY]
                .iter()
                .flat_map(|entity| reader.query(entity, &Query::all()).unwrap_or_default())
                .filter(|record| record.key("category_id") == key_text(key))
                .map(|record| record.text("kind"))
                .collect()
        };
        for change in changes {
            match change {
                Change::New(object) | Change::Updated(object)
                    if object.entity() == COMMITMENT || object.entity() == LEDGER_ENTRY =>
                {
                    let category = object.get("category_id");
                    if matches!(category, Value::Null) {
                        continue;
                    }
                    if let Some(kind) = kind_of(&category)
                        && !serves(&kind, &object.text("kind"))
                    {
                        return Err(format!(
                            "the category of {} is for the other kind",
                            object.text("description")
                        ));
                    }
                }
                Change::Updated(object) if object.entity() == FINANCE_CATEGORY => {
                    let kind = object.text("kind");
                    if in_use(&object.get("id")).iter().any(|movement| !serves(&kind, movement)) {
                        return Err(format!("{} has movements of the other kind", object.text("name")));
                    }
                }
                Change::Deleted { entity, key } if entity == FINANCE_CATEGORY && !in_use(key).is_empty() => {
                    return Err("a category in use is not deleted".to_owned());
                }
                _ => {}
            }
        }
        Ok(())
    })
}
