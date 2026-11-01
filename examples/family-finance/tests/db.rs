//! The rules that keep the money right, run against a real SQLite database.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]
#![allow(clippy::inconsistent_digit_grouping, reason = "amounts read as reais_cents")]

use family_finance::db::Database;
use family_finance::filter;
use family_finance::model::{AccountKind, BillInput, BillStatus, Flow, Recurrence, TransactionInput};
use family_finance::{reports, seed};
use ravenrs::prelude::Date;
use ravenrs_report::ReportEngine;

fn date(y: i32, m: u8, d: u8) -> Date {
    Date::new(y, m, d).expect("valid date")
}

struct Fixture {
    db: Database,
    account: i64,
    groceries: i64,
    salary: i64,
}

fn fixture() -> Fixture {
    let db = Database::in_memory().expect("database");
    let account = db.add_account("Banco", AccountKind::Checking, 100_00).expect("account");
    let groceries = db.add_category("Mercado", Flow::Expense, 500_00).expect("category");
    let salary = db.add_category("Salário", Flow::Income, 0).expect("category");
    Fixture { db, account, groceries, salary }
}

fn expense(f: &Fixture, day: Date, text: &str, cents: i64) -> i64 {
    f.db.add_transaction(&TransactionInput {
        date: day,
        description: text.to_owned(),
        amount_cents: cents,
        flow: Flow::Expense,
        account_id: f.account,
        category_id: f.groceries,
        member_id: None,
    })
    .expect("transaction")
}

fn bill(f: &Fixture, due: Date, cents: i64, recurrence: Recurrence, installments: u32) -> Vec<i64> {
    f.db.add_bill(&BillInput {
        description: "Luz".to_owned(),
        category_id: f.groceries,
        member_id: None,
        amount_cents: cents,
        due,
        recurrence,
        installments,
        barcode: String::new(),
    })
    .expect("bill")
}

fn balance(f: &Fixture) -> i64 {
    f.db.accounts().expect("accounts")[0].balance_cents
}

#[test]
fn the_balance_follows_the_transactions() {
    let f = fixture();
    expense(&f, date(2026, 3, 1), "Feira", 30_00);
    f.db.add_transaction(&TransactionInput {
        date: date(2026, 3, 5),
        description: "Salário".to_owned(),
        amount_cents: 1_000_00,
        flow: Flow::Income,
        account_id: f.account,
        category_id: f.salary,
        member_id: None,
    })
    .expect("income");
    assert_eq!(balance(&f), 100_00 - 30_00 + 1_000_00);
}

#[test]
fn the_schema_refuses_bad_data() {
    let f = fixture();
    let bad = TransactionInput {
        date: date(2026, 3, 1),
        description: "Feira".to_owned(),
        amount_cents: 0,
        flow: Flow::Expense,
        account_id: f.account,
        category_id: f.groceries,
        member_id: None,
    };
    assert!(f.db.add_transaction(&bad).is_err(), "zero amount");
    assert!(
        f.db.add_transaction(&TransactionInput { amount_cents: 1, account_id: 999, ..bad }).is_err(),
        "an account that does not exist"
    );
    let duplicate = f.db.add_account("Banco", AccountKind::Cash, 0).expect_err("same name");
    assert!(duplicate.to_string().contains("Já existe"), "{duplicate}");
}

#[test]
fn an_account_in_use_cannot_be_deleted() {
    let f = fixture();
    expense(&f, date(2026, 3, 1), "Feira", 30_00);
    let refused = f.db.delete_account(f.account).expect_err("in use");
    assert!(refused.to_string().contains("em uso"), "{refused}");
    assert_eq!(f.db.accounts().expect("accounts").len(), 1);
}

#[test]
fn paying_a_bill_records_the_expense_and_can_be_undone() {
    let f = fixture();
    let id = bill(&f, date(2026, 3, 10), 40_00, Recurrence::None, 1)[0];
    let transaction = f.db.pay_bill(id, f.account, date(2026, 3, 9), 42_00).expect("pay");
    assert_eq!(balance(&f), 58_00, "what was really paid, with interest");
    let paid = f.db.bill(id).expect("read").expect("bill");
    assert_eq!(paid.paid_on, Some(date(2026, 3, 9)));
    assert_eq!(paid.status(date(2026, 4, 1)), BillStatus::Paid);
    assert!(f.db.pay_bill(id, f.account, date(2026, 3, 9), 42_00).is_err(), "not twice");
    assert_eq!(balance(&f), 58_00);

    f.db.reopen_bill(id).expect("undo");
    assert_eq!(balance(&f), 100_00);
    assert_eq!(f.db.transaction(transaction).expect("read"), None);
    assert_eq!(f.db.bill(id).expect("read").expect("bill").paid_on, None);
}

#[test]
fn deleting_the_expense_reopens_the_bill() {
    let f = fixture();
    let id = bill(&f, date(2026, 3, 10), 40_00, Recurrence::None, 1)[0];
    let transaction = f.db.pay_bill(id, f.account, date(2026, 3, 9), 40_00).expect("pay");
    f.db.delete_transaction(transaction).expect("delete");
    assert_eq!(f.db.bill(id).expect("read").expect("bill").paid_on, None);
}

#[test]
fn a_failed_payment_leaves_nothing_behind() {
    let f = fixture();
    let id = bill(&f, date(2026, 3, 10), 40_00, Recurrence::Monthly, 1)[0];
    assert!(f.db.pay_bill(id, 999, date(2026, 3, 9), 40_00).is_err(), "an account that does not exist");
    assert_eq!(f.db.bill(id).expect("read").expect("bill").paid_on, None);
    assert_eq!(f.db.bills().expect("bills").len(), 1, "the next one was not scheduled");
    assert_eq!(balance(&f), 100_00);
}

#[test]
fn a_paid_bill_cannot_be_deleted() {
    let f = fixture();
    let id = bill(&f, date(2026, 3, 10), 40_00, Recurrence::None, 1)[0];
    f.db.pay_bill(id, f.account, date(2026, 3, 9), 40_00).expect("pay");
    assert!(f.db.delete_bill(id).is_err());
    assert_eq!(balance(&f), 60_00);
}

#[test]
fn a_repeating_bill_schedules_the_next_one_only_once() {
    let f = fixture();
    let id = bill(&f, date(2026, 1, 31), 40_00, Recurrence::Monthly, 1)[0];
    f.db.pay_bill(id, f.account, date(2026, 1, 31), 40_00).expect("pay");
    f.db.reopen_bill(id).expect("undo");
    f.db.pay_bill(id, f.account, date(2026, 1, 31), 40_00).expect("pay again");
    let bills = f.db.bills().expect("bills");
    assert_eq!(bills.len(), 2, "paying twice scheduled one");
    assert_eq!(bills[1].due, date(2026, 2, 28), "February has no 31st");

    f.db.pay_bill(bills[1].id, f.account, date(2026, 2, 28), 40_00).expect("pay");
    assert_eq!(f.db.bills().expect("bills")[2].due, date(2026, 3, 31), "and March goes back to it");
}

#[test]
fn a_yearly_bill_comes_back_next_year() {
    let f = fixture();
    let id = bill(&f, date(2026, 3, 10), 40_00, Recurrence::Yearly, 1)[0];
    f.db.pay_bill(id, f.account, date(2026, 3, 10), 40_00).expect("pay");
    assert_eq!(f.db.bills().expect("bills")[1].due, date(2027, 3, 10));
}

#[test]
fn installments_add_up_and_fall_month_by_month() {
    let f = fixture();
    let ids = bill(&f, date(2026, 11, 30), 100_00, Recurrence::Monthly, 3);
    let bills = f.db.bills().expect("bills");
    assert_eq!(ids.len(), 3);
    assert_eq!(bills.iter().map(|b| b.amount_cents).sum::<i64>(), 100_00);
    assert_eq!(
        bills.iter().map(|b| b.due).collect::<Vec<_>>(),
        [date(2026, 11, 30), date(2026, 12, 30), date(2027, 1, 30)]
    );
    assert_eq!(bills[1].title(), "Luz (2/3)");
    assert!(bills.iter().all(|b| b.recurrence == Recurrence::None), "installments end by themselves");
}

#[test]
fn status_depends_on_the_day() {
    let f = fixture();
    let id = bill(&f, date(2026, 3, 10), 40_00, Recurrence::None, 1)[0];
    let bill = f.db.bill(id).expect("read").expect("bill");
    assert_eq!(bill.status(date(2026, 3, 9)), BillStatus::Upcoming);
    assert_eq!(bill.status(date(2026, 3, 10)), BillStatus::DueToday);
    assert_eq!(bill.status(date(2026, 3, 11)), BillStatus::Overdue);
}

#[test]
fn month_totals_include_empty_months_and_ignore_other_ones() {
    let f = fixture();
    expense(&f, date(2026, 3, 31), "Março", 10_00);
    expense(&f, date(2026, 4, 1), "Abril", 20_00);
    let months = f.db.monthly_totals(date(2026, 5, 15), 3).expect("totals");
    assert_eq!(
        months.iter().map(|m| m.month.as_str()).collect::<Vec<_>>(),
        ["2026-03", "2026-04", "2026-05"]
    );
    assert_eq!(months.iter().map(|m| m.expense_cents).collect::<Vec<_>>(), [10_00, 20_00, 0]);
    assert_eq!(f.db.balance_history(date(2026, 5, 15), 3).expect("history"), [90_00, 70_00, 70_00]);
}

#[test]
fn totals_between_leave_out_the_end_day() {
    let f = fixture();
    expense(&f, date(2026, 3, 1), "Dentro", 10_00);
    expense(&f, date(2026, 3, 15), "Fora", 20_00);
    let totals = f.db.totals_between(date(2026, 3, 1), date(2026, 3, 15)).expect("totals");
    assert_eq!((totals.income_cents, totals.expense_cents), (0, 10_00));
}

#[test]
fn the_budget_counts_only_that_month() {
    let f = fixture();
    expense(&f, date(2026, 3, 31), "Março", 10_00);
    expense(&f, date(2026, 4, 1), "Abril", 20_00);
    let lines = f.db.budget_lines(date(2026, 4, 20)).expect("lines");
    assert_eq!(lines.len(), 1, "income categories have no budget");
    assert_eq!((lines[0].spent_cents, lines[0].budget_cents), (20_00, 500_00));
}

#[test]
fn an_expression_filter_runs_in_the_database() {
    let f = fixture();
    expense(&f, date(2026, 3, 1), "Feira barata", 30_00);
    expense(&f, date(2026, 3, 2), "Feira cara", 300_00);
    expense(&f, date(2026, 4, 2), "Outro mês", 300_00);
    let compiled =
        filter::compile("[Valor] > 100 And [Descrição] Like 'Feira%'").expect("valid").expect("a filter");
    let rows = f.db.ledger(date(2026, 3, 15), Some(&compiled)).expect("rows");
    assert_eq!(rows.iter().map(|r| r.description.as_str()).collect::<Vec<_>>(), ["Feira cara"]);
    assert_eq!(f.db.ledger(date(2026, 3, 15), None).expect("rows").len(), 2);
}

#[test]
fn typed_text_cannot_change_the_query() {
    let f = fixture();
    expense(&f, date(2026, 3, 1), "Feira", 30_00);
    let hostile = "[Descrição] = 'x''; DROP TABLE transactions; --'";
    let compiled = filter::compile(hostile).expect("valid").expect("a filter");
    assert!(!compiled.sql.contains("DROP"), "values travel as parameters: {}", compiled.sql);
    assert!(f.db.ledger(date(2026, 3, 15), Some(&compiled)).expect("rows").is_empty());
    assert_eq!(f.db.ledger(date(2026, 3, 15), None).expect("rows").len(), 1, "the table is still there");
}

#[test]
fn a_bad_filter_says_why() {
    assert_eq!(filter::compile("  ").expect("blank"), None);
    assert!(filter::compile("[Valor] >").is_err(), "does not parse");
    assert!(filter::compile("[Preço] > 10").is_err(), "no such field");
}

#[test]
fn a_file_database_keeps_its_data_and_migrates_once() {
    let folder = std::env::temp_dir().join(format!("family-finance-{}", std::process::id()));
    let path = folder.join("nested").join("financas.db");
    {
        let db = Database::open(&path).expect("create");
        assert_eq!(db.schema_version().expect("version"), 1);
        assert!(db.is_empty().expect("empty"));
        db.add_account("Banco", AccountKind::Cash, 5_00).expect("account");
        db.backup_to(&folder.join("copia.db")).expect("backup");
        assert!(db.backup_to(&folder.join("copia.db")).is_err(), "never over an existing file");
    }
    let again = Database::open(&path).expect("reopen");
    assert_eq!(again.accounts().expect("accounts")[0].balance_cents, 5_00);
    let copy = Database::open(&folder.join("copia.db")).expect("the backup opens");
    assert_eq!(copy.accounts().expect("accounts")[0].name, "Banco");
    drop((again, copy));
    std::fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn the_sample_family_has_a_bit_of_everything() {
    let db = Database::in_memory().expect("database");
    let today = date(2026, 11, 15);
    seed::load_demo(&db, today).expect("seed");
    let bills = db.bills().expect("bills");
    assert!(bills.iter().any(|b| b.status(today) == BillStatus::Overdue), "something to warn about");
    assert!(bills.iter().any(|b| b.status(today) == BillStatus::Upcoming));
    assert!(bills.iter().any(|b| b.status(today) == BillStatus::Paid));
    assert!(bills.iter().any(|b| b.installments > 1));
    assert!(!db.ledger(today, None).expect("rows").is_empty());
    let future =
        db.ledger(today, None).expect("rows").into_iter().filter(|r| r.date > today.to_string()).count();
    assert_eq!(future, 0, "nothing happened in the future");
    assert_eq!(
        db.monthly_totals(today, 12).expect("totals").iter().filter(|m| m.expense_cents == 0).count(),
        0
    );
}

#[test]
fn every_report_renders_over_the_sample_family_and_exports() {
    let db = Database::in_memory().expect("database");
    let today = date(2026, 11, 15);
    seed::load_demo(&db, today).expect("seed");
    let mut engine = ReportEngine::new();
    for kind in reports::ReportKind::ALL {
        let document =
            reports::render(&mut engine, &db, kind, today.first_of_month(), today).expect("render");
        assert!(!document.pages.is_empty(), "{kind:?}");
        assert!(document.errors.is_empty(), "{kind:?}: {:?}", document.errors);
        for format in [
            ravenrs_report_export::ExportFormat::Pdf,
            ravenrs_report_export::ExportFormat::Xlsx,
            ravenrs_report_export::ExportFormat::Csv,
        ] {
            let file = ravenrs_report_export::export(&document, format, None).expect("export");
            assert!(!file.bytes.is_empty(), "{kind:?} {format:?}");
        }
    }
}
