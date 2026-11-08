//! Transactions, and what is added up from them: months, balances and budgets.

use ravenrs::prelude::Date;
use rusqlite::types::Value as SqlValue;
use rusqlite::{OptionalExtension, Row, params, params_from_iter};

use super::{Database, DbResult, date_at};
use crate::filter::CompiledFilter;
use crate::model::{BudgetLine, Flow, MonthTotals, Transaction, TransactionInput, month_range};

impl Database {
    /// Saves a transaction and returns the key.
    pub fn add_transaction(&self, input: &TransactionInput) -> DbResult<i64> {
        self.conn.execute(
            "INSERT INTO transactions (date, description, amount_cents, flow, account_id, category_id, member_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                input.date.to_string(),
                input.description.trim(),
                input.amount_cents,
                input.flow.code(),
                input.account_id,
                input.category_id,
                input.member_id
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Changes a transaction.
    pub fn update_transaction(&self, id: i64, input: &TransactionInput) -> DbResult<()> {
        self.conn.execute(
            "UPDATE transactions SET date = ?2, description = ?3, amount_cents = ?4, flow = ?5,
                    account_id = ?6, category_id = ?7, member_id = ?8 WHERE id = ?1",
            params![
                id,
                input.date.to_string(),
                input.description.trim(),
                input.amount_cents,
                input.flow.code(),
                input.account_id,
                input.category_id,
                input.member_id
            ],
        )?;
        Ok(())
    }

    /// Removes a transaction. A bill it paid goes back to unpaid.
    pub fn delete_transaction(&self, id: i64) -> DbResult<()> {
        self.atomically(|db| {
            db.conn.execute(
                "UPDATE bills SET paid_on = NULL, transaction_id = NULL WHERE transaction_id = ?1",
                [id],
            )?;
            db.conn.execute("DELETE FROM transactions WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// One transaction.
    pub fn transaction(&self, id: i64) -> DbResult<Option<Transaction>> {
        let mut query = self.conn.prepare_cached(
            "SELECT t.id, t.date, t.description, t.amount_cents, t.flow, t.account_id, t.category_id, t.member_id,
                    a.name, c.name, COALESCE(m.name, '')
             FROM transactions t JOIN accounts a ON a.id = t.account_id JOIN categories c ON c.id = t.category_id
             LEFT JOIN members m ON m.id = t.member_id WHERE t.id = ?1",
        )?;
        Ok(query.query_row([id], transaction_from_row).optional()?)
    }

    /// The month's transactions as the `ledger` view shows them, newest first, matching `filter`.
    /// Each row is id, date, description, value, type, account, category and member.
    pub fn ledger(&self, month: Date, filter: Option<&CompiledFilter>) -> DbResult<Vec<LedgerRow>> {
        let (from, to) = month_range(month);
        let (criteria, mut values) = match filter {
            Some(f) => (format!("({}) AND ", f.sql), f.params.clone()),
            None => (String::new(), Vec::new()),
        };
        // The filter already numbered its placeholders from 1, so the period comes after them.
        let start = values.len() + 1;
        values.push(SqlValue::Text(from.to_string()));
        values.push(SqlValue::Text(to.to_string()));
        let sql = format!(
            r#"SELECT "Id", "Data", "Descrição", "Valor", "Tipo", "Conta", "Categoria", "Membro" FROM ledger
               WHERE {criteria}"Data" >= ?{start} AND "Data" < ?{end} ORDER BY "Data" DESC, "Id" DESC"#,
            end = start + 1,
        );
        let mut query = self.conn.prepare(&sql)?;
        let rows = query.query_map(params_from_iter(values), |r| {
            Ok(LedgerRow {
                id: r.get(0)?,
                date: r.get(1)?,
                description: r.get(2)?,
                value: r.get(3)?,
                kind: r.get(4)?,
                account: r.get(5)?,
                category: r.get(6)?,
                member: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Income and expenses for the `count` months ending at `last`, oldest first; empty months included.
    pub fn monthly_totals(&self, last: Date, count: u32) -> DbResult<Vec<MonthTotals>> {
        let first = last.first_of_month().add_months(1 - count as i32);
        let (_, end) = month_range(last);
        let mut months: Vec<MonthTotals> = (0..count)
            .map(|i| {
                let month = first.add_months(i as i32).to_string();
                MonthTotals { month: month[..7].to_owned(), ..MonthTotals::default() }
            })
            .collect();
        let mut query = self.conn.prepare_cached(
            "SELECT substr(date, 1, 7), flow, SUM(amount_cents) FROM transactions
             WHERE date >= ?1 AND date < ?2 GROUP BY 1, 2",
        )?;
        let rows = query.query_map([first.to_string(), end.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (month, flow, cents) = row?;
            if let Some(line) = months.iter_mut().find(|m| m.month == month) {
                match Flow::from_code(&flow) {
                    Flow::Income => line.income_cents = cents,
                    Flow::Expense => line.expense_cents = cents,
                }
            }
        }
        Ok(months)
    }

    /// Income and expenses from `from` up to, but not including, `to`.
    pub fn totals_between(&self, from: Date, to: Date) -> DbResult<MonthTotals> {
        let mut query = self.conn.prepare_cached(
            "SELECT COALESCE(SUM(CASE flow WHEN 'income' THEN amount_cents END), 0),
                    COALESCE(SUM(CASE flow WHEN 'expense' THEN amount_cents END), 0)
             FROM transactions WHERE date >= ?1 AND date < ?2",
        )?;
        Ok(query.query_row([from.to_string(), to.to_string()], |r| {
            Ok(MonthTotals { month: String::new(), income_cents: r.get(0)?, expense_cents: r.get(1)? })
        })?)
    }

    /// The total balance at the end of each of the `count` months ending at `last`, oldest first.
    pub fn balance_history(&self, last: Date, count: u32) -> DbResult<Vec<i64>> {
        let first = last.first_of_month().add_months(1 - count as i32);
        let opening: i64 = self.conn.query_row(
            "SELECT (SELECT COALESCE(SUM(opening_cents), 0) FROM accounts)
                  + COALESCE((SELECT SUM(CASE flow WHEN 'income' THEN amount_cents ELSE -amount_cents END)
                              FROM transactions WHERE date < ?1), 0)",
            [first.to_string()],
            |r| r.get(0),
        )?;
        let mut balance = opening;
        Ok(self
            .monthly_totals(last, count)?
            .into_iter()
            .map(|m| {
                balance += m.income_cents - m.expense_cents;
                balance
            })
            .collect())
    }

    /// What each expense category spent in `month` against its limit, biggest spender first.
    pub fn budget_lines(&self, month: Date) -> DbResult<Vec<BudgetLine>> {
        let (from, to) = month_range(month);
        let mut query = self.conn.prepare_cached(
            "SELECT c.id, c.name, c.budget_cents, COALESCE(SUM(t.amount_cents), 0)
             FROM categories c
             LEFT JOIN transactions t ON t.category_id = c.id AND t.flow = 'expense' AND t.date >= ?1 AND t.date < ?2
             WHERE c.flow = 'expense' GROUP BY c.id ORDER BY 4 DESC, c.name",
        )?;
        let rows = query.query_map([from.to_string(), to.to_string()], |r| {
            Ok(BudgetLine {
                category_id: r.get(0)?,
                category: r.get(1)?,
                budget_cents: r.get(2)?,
                spent_cents: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// A row of the `ledger` view.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerRow {
    /// The transaction's key.
    pub id: i64,
    /// ISO date.
    pub date: String,
    /// Description.
    pub description: String,
    /// The amount in reais.
    pub value: f64,
    /// `Despesa` or `Receita`.
    pub kind: String,
    /// The account's name.
    pub account: String,
    /// The category's name.
    pub category: String,
    /// The member's name; empty when there is none.
    pub member: String,
}

fn transaction_from_row(r: &Row<'_>) -> rusqlite::Result<Transaction> {
    Ok(Transaction {
        id: r.get(0)?,
        input: TransactionInput {
            date: date_at(r, 1)?,
            description: r.get(2)?,
            amount_cents: r.get(3)?,
            flow: Flow::from_code(&r.get::<_, String>(4)?),
            account_id: r.get(5)?,
            category_id: r.get(6)?,
            member_id: r.get(7)?,
        },
        account: r.get(8)?,
        category: r.get(9)?,
        member: r.get(10)?,
    })
}
