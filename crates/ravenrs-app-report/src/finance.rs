//! The finance reports as documents, to print or keep as PDF: the statement of the cash book, the
//! totals of a year, the position of what is open and the forecast of a month (#859).
//!
//! Each takes what `ravenrs_app_library::finance` read through the person's space, so whoever
//! cannot read the cash book prints nothing of it, and writes money and dates in `culture`.

use ravenrs_app_library::finance::{Forecast, MonthTotals, Position, Statement};
use ravenrs_criteria::{Culture, DateTime, Value};
use ravenrs_grid_export::document_of;
use ravenrs_report::Document;
use ravenrs_widgets::grid::{GridColumn, GridQuery, Value as Cell, to_table};

use crate::ReportWords;

fn table(title: &str, columns: &[&String], rows: Vec<Vec<String>>) -> Document {
    let columns: Vec<GridColumn> = columns.iter().map(|title| GridColumn::text((*title).clone())).collect();
    let rows: Vec<Vec<Cell>> =
        rows.into_iter().map(|row| row.into_iter().map(Cell::Text).collect()).collect();
    let every: Vec<usize> = (0..rows.len()).collect();
    document_of(&to_table(title, "", &columns, &rows, &GridQuery::default(), &every))
}

fn day(on: DateTime, culture: &Culture) -> String {
    ravenrs_criteria::format_value(&Value::Date(on), "d", culture)
}

/// The statement of the cash book: the balance before, each entry with the balance after it, and
/// the balance at the end.
pub fn statement_document(statement: &Statement, culture: &Culture, words: &ReportWords) -> Document {
    let money = |cents: i64| culture.format_cents(cents);
    let mut rows =
        vec![vec![String::new(), words.finance_opening.clone(), String::new(), money(statement.opening)]];
    rows.extend(statement.lines.iter().map(|line| {
        vec![day(line.on, culture), line.description.clone(), money(line.amount), money(line.balance)]
    }));
    rows.push(vec![String::new(), words.finance_closing.clone(), String::new(), money(statement.closing)]);
    let columns =
        [&words.finance_date, &words.finance_description, &words.finance_amount, &words.finance_balance];
    table(&words.finance_statement, &columns, rows)
}

/// What came in, what went out and what was left, month by month, with the year's total last.
pub fn annual_document(
    year: i32,
    months: &[MonthTotals],
    culture: &Culture,
    words: &ReportWords,
) -> Document {
    let money = |cents: i64| culture.format_cents(cents);
    let mut rows: Vec<Vec<String>> = months
        .iter()
        .map(|month| {
            vec![
                format!("{:02}/{year}", month.month),
                money(month.income),
                money(month.expense),
                money(month.income - month.expense),
            ]
        })
        .collect();
    let (income, expense) = months
        .iter()
        .fold((0, 0), |(income, expense), month| (income + month.income, expense + month.expense));
    rows.push(vec![year.to_string(), money(income), money(expense), money(income - expense)]);
    let columns = [&words.finance_month, &words.finance_income, &words.finance_expense, &words.finance_net];
    table(&format!("{} {year}", words.finance_annual), &columns, rows)
}

/// What is open to pay and to receive, and how much of it was due before the day.
pub fn position_document(position: &Position, culture: &Culture, words: &ReportWords) -> Document {
    let money = |cents: i64| culture.format_cents(cents);
    let rows = vec![
        vec![words.finance_payable.clone(), money(position.payable), money(position.overdue_payable)],
        vec![
            words.finance_receivable.clone(),
            money(position.receivable),
            money(position.overdue_receivable),
        ],
    ];
    let columns = [&words.finance_position, &words.finance_open, &words.finance_overdue];
    table(&words.finance_position, &columns, rows)
}

/// The open bills due in a month, by due date, with what is left of each, and the totals to pay
/// and to receive.
pub fn forecast_document(forecast: &Forecast, culture: &Culture, words: &ReportWords) -> Document {
    let money = |cents: i64| culture.format_cents(cents);
    let mut rows: Vec<Vec<String>> = forecast
        .due
        .iter()
        .map(|bill| {
            let due = match bill.get("due_on") {
                Value::Date(on) => day(on, culture),
                _ => String::new(),
            };
            let left = bill.cents("amount").unwrap_or(0) - bill.cents("settled").unwrap_or(0);
            let kind = if bill.text("kind") == "receivable" {
                &words.finance_receivable
            } else {
                &words.finance_payable
            };
            vec![due, bill.text("description"), kind.clone(), money(left)]
        })
        .collect();
    rows.push(vec![String::new(), words.finance_payable.clone(), String::new(), money(forecast.payable)]);
    rows.push(vec![
        String::new(),
        words.finance_receivable.clone(),
        String::new(),
        money(forecast.receivable),
    ]);
    let columns =
        [&words.finance_due, &words.finance_description, &words.finance_position, &words.finance_open];
    table(&words.finance_forecast, &columns, rows)
}
