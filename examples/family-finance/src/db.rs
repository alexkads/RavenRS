//! Everything the app stores, in one SQLite file.
//!
//! Money is kept in whole cents and dates as ISO text, which sorts by date.
//! Writes that touch more than one row run in a transaction, so paying a bill
//! can never leave a payment without its transaction.

use std::fmt;
use std::path::Path;

use ravenrs::prelude::Date;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, ErrorCode, OptionalExtension, Row, params, params_from_iter};

use crate::filter::CompiledFilter;
use crate::model::{
    Account, AccountKind, Bill, BillInput, BudgetLine, Category, Flow, Member, MonthTotals, Recurrence,
    Transaction, TransactionInput, month_range, parse_date, shift_months,
};
use crate::money::split_installments;

/// Why a database operation failed, in words the user can act on.
#[derive(Debug)]
pub enum DbError {
    /// A rule of the app refused the change.
    Rule(String),
    /// SQLite reported a problem.
    Sql(rusqlite::Error),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(message) => f.write_str(message),
            Self::Sql(e) => match e.sqlite_error_code() {
                Some(ErrorCode::ConstraintViolation) => {
                    let text = e.to_string();
                    if text.contains("UNIQUE") {
                        f.write_str("Já existe um registro com esse nome. Escolha outro.")
                    } else if text.contains("FOREIGN KEY") {
                        f.write_str("Este registro está em uso. Remova antes o que depende dele.")
                    } else {
                        f.write_str("Os dados não passaram nas regras do banco. Revise o formulário.")
                    }
                }
                _ => write!(f, "Não foi possível acessar o banco de dados: {e}"),
            },
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e)
    }
}

/// The result of a database operation.
pub type DbResult<T> = Result<T, DbError>;

/// Numbered schema steps; `PRAGMA user_version` records how many have run.
const MIGRATIONS: &[&str] = &[r#"
    CREATE TABLE members (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) > 0)
    );
    CREATE TABLE accounts (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) > 0),
        kind TEXT NOT NULL CHECK (kind IN ('checking', 'savings', 'cash', 'card')),
        opening_cents INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE categories (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL CHECK (length(trim(name)) > 0),
        flow TEXT NOT NULL CHECK (flow IN ('expense', 'income')),
        budget_cents INTEGER NOT NULL DEFAULT 0 CHECK (budget_cents >= 0),
        UNIQUE (name, flow)
    );
    CREATE TABLE transactions (
        id INTEGER PRIMARY KEY,
        date TEXT NOT NULL,
        description TEXT NOT NULL CHECK (length(trim(description)) > 0),
        amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
        flow TEXT NOT NULL CHECK (flow IN ('expense', 'income')),
        account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE RESTRICT,
        category_id INTEGER NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
        member_id INTEGER REFERENCES members (id) ON DELETE SET NULL
    );
    CREATE INDEX transactions_date ON transactions (date);
    CREATE INDEX transactions_category ON transactions (category_id, date);
    CREATE TABLE bills (
        id INTEGER PRIMARY KEY,
        description TEXT NOT NULL CHECK (length(trim(description)) > 0),
        category_id INTEGER NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
        member_id INTEGER REFERENCES members (id) ON DELETE SET NULL,
        amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
        due TEXT NOT NULL,
        recurrence TEXT NOT NULL DEFAULT 'none' CHECK (recurrence IN ('none', 'monthly', 'yearly')),
        anchor_day INTEGER NOT NULL,
        installment INTEGER NOT NULL DEFAULT 1,
        installments INTEGER NOT NULL DEFAULT 1,
        barcode TEXT NOT NULL DEFAULT '',
        paid_on TEXT,
        transaction_id INTEGER REFERENCES transactions (id) ON DELETE SET NULL
    );
    CREATE INDEX bills_due ON bills (due);
    -- What the advanced filter and the reports read. The names are the ones users type.
    CREATE VIEW ledger AS
    SELECT t.id AS "Id", t.date AS "Data", t.description AS "Descrição",
           t.amount_cents / 100.0 AS "Valor",
           CASE t.flow WHEN 'income' THEN 'Receita' ELSE 'Despesa' END AS "Tipo",
           a.name AS "Conta", c.name AS "Categoria", COALESCE(m.name, '') AS "Membro"
    FROM transactions t
    JOIN accounts a ON a.id = t.account_id
    JOIN categories c ON c.id = t.category_id
    LEFT JOIN members m ON m.id = t.member_id;
"#];

const BILL_COLUMNS: &str = "b.id, b.description, b.category_id, c.name, b.member_id, COALESCE(m.name, ''),
    b.amount_cents, b.due, b.recurrence, b.anchor_day, b.installment, b.installments, b.barcode, b.paid_on,
    b.transaction_id";
const BILL_JOINS: &str =
    "FROM bills b JOIN categories c ON c.id = b.category_id LEFT JOIN members m ON m.id = b.member_id";

/// The app's database.
#[derive(Debug)]
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens the file at `path`, creating it and its folder when missing.
    pub fn open(path: &Path) -> DbResult<Self> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)
                .map_err(|e| DbError::Rule(format!("Não foi possível criar a pasta dos dados: {e}")))?;
        }
        let db = Self { conn: Connection::open(path)? };
        // Write-ahead logging survives a crash mid-write without losing what was saved.
        db.conn.pragma_update(None, "journal_mode", "WAL")?;
        db.prepare()?;
        Ok(db)
    }

    /// A database that lasts as long as this value, for tests.
    pub fn in_memory() -> DbResult<Self> {
        let db = Self { conn: Connection::open_in_memory()? };
        db.prepare()?;
        Ok(db)
    }

    fn prepare(&self) -> DbResult<()> {
        self.conn.pragma_update(None, "foreign_keys", true)?;
        let done: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, step) in MIGRATIONS.iter().enumerate().skip(done as usize) {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(step)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(())
    }

    /// The number of migrations applied.
    pub fn schema_version(&self) -> DbResult<i64> {
        Ok(self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// There are no accounts yet, so this is the first run.
    pub fn is_empty(&self) -> DbResult<bool> {
        let accounts: i64 = self.conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))?;
        Ok(accounts == 0)
    }

    /// Runs `work` in one transaction: all of it is saved, or none. Inside another
    /// call it joins that transaction, so an error undoes the whole outer work.
    pub fn atomically<T>(&self, work: impl FnOnce(&Self) -> DbResult<T>) -> DbResult<T> {
        if !self.conn.is_autocommit() {
            return work(self);
        }
        let tx = self.conn.unchecked_transaction()?;
        let value = work(self)?;
        tx.commit()?;
        Ok(value)
    }

    /// Removes everything the user entered.
    pub fn clear(&self) -> DbResult<()> {
        self.atomically(|db| {
            db.conn.execute_batch(
                "DELETE FROM bills; DELETE FROM transactions; DELETE FROM categories;
                 DELETE FROM accounts; DELETE FROM members;",
            )?;
            Ok(())
        })
    }

    /// Writes a consistent copy of the database to `path`, even while it is in use.
    pub fn backup_to(&self, path: &Path) -> DbResult<()> {
        if path.exists() {
            return Err(DbError::Rule(format!("Já existe um arquivo em {}.", path.display())));
        }
        self.conn.execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
        Ok(())
    }

    // Members

    /// Everyone in the family, by name.
    pub fn members(&self) -> DbResult<Vec<Member>> {
        let mut query = self.conn.prepare_cached("SELECT id, name FROM members ORDER BY name")?;
        let rows = query.query_map([], |r| Ok(Member { id: r.get(0)?, name: r.get(1)? }))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Adds a person and returns the key.
    pub fn add_member(&self, name: &str) -> DbResult<i64> {
        self.conn.execute("INSERT INTO members (name) VALUES (?1)", [name.trim()])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Renames a person.
    pub fn rename_member(&self, id: i64, name: &str) -> DbResult<()> {
        self.conn.execute("UPDATE members SET name = ?2 WHERE id = ?1", params![id, name.trim()])?;
        Ok(())
    }

    /// Removes a person; their transactions and bills stay, without a member.
    pub fn delete_member(&self, id: i64) -> DbResult<()> {
        self.conn.execute("DELETE FROM members WHERE id = ?1", [id])?;
        Ok(())
    }

    // Accounts

    /// Every account with its balance, by name.
    pub fn accounts(&self) -> DbResult<Vec<Account>> {
        let mut query = self.conn.prepare_cached(
            "SELECT a.id, a.name, a.kind, a.opening_cents,
                    a.opening_cents + COALESCE(SUM(CASE t.flow WHEN 'income' THEN t.amount_cents ELSE -t.amount_cents END), 0)
             FROM accounts a LEFT JOIN transactions t ON t.account_id = a.id
             GROUP BY a.id ORDER BY a.name",
        )?;
        let rows = query.query_map([], |r| {
            Ok(Account {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: AccountKind::from_code(&r.get::<_, String>(2)?),
                opening_cents: r.get(3)?,
                balance_cents: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Adds an account and returns the key.
    pub fn add_account(&self, name: &str, kind: AccountKind, opening_cents: i64) -> DbResult<i64> {
        self.conn.execute(
            "INSERT INTO accounts (name, kind, opening_cents) VALUES (?1, ?2, ?3)",
            params![name.trim(), kind.code(), opening_cents],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Changes an account.
    pub fn update_account(&self, id: i64, name: &str, kind: AccountKind, opening_cents: i64) -> DbResult<()> {
        self.conn.execute(
            "UPDATE accounts SET name = ?2, kind = ?3, opening_cents = ?4 WHERE id = ?1",
            params![id, name.trim(), kind.code(), opening_cents],
        )?;
        Ok(())
    }

    /// Removes an account; refused while transactions use it.
    pub fn delete_account(&self, id: i64) -> DbResult<()> {
        self.conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        Ok(())
    }

    // Categories

    /// Every category, by name.
    pub fn categories(&self) -> DbResult<Vec<Category>> {
        let mut query =
            self.conn.prepare_cached("SELECT id, name, flow, budget_cents FROM categories ORDER BY name")?;
        let rows = query.query_map([], |r| {
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                flow: Flow::from_code(&r.get::<_, String>(2)?),
                budget_cents: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Adds a category and returns the key.
    pub fn add_category(&self, name: &str, flow: Flow, budget_cents: i64) -> DbResult<i64> {
        self.conn.execute(
            "INSERT INTO categories (name, flow, budget_cents) VALUES (?1, ?2, ?3)",
            params![name.trim(), flow.code(), budget_cents],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Changes a category.
    pub fn update_category(&self, id: i64, name: &str, flow: Flow, budget_cents: i64) -> DbResult<()> {
        self.conn.execute(
            "UPDATE categories SET name = ?2, flow = ?3, budget_cents = ?4 WHERE id = ?1",
            params![id, name.trim(), flow.code(), budget_cents],
        )?;
        Ok(())
    }

    /// Sets a category's monthly limit.
    pub fn set_budget(&self, category_id: i64, budget_cents: i64) -> DbResult<()> {
        self.conn.execute(
            "UPDATE categories SET budget_cents = ?2 WHERE id = ?1",
            params![category_id, budget_cents],
        )?;
        Ok(())
    }

    /// Removes a category; refused while transactions or bills use it.
    pub fn delete_category(&self, id: i64) -> DbResult<()> {
        self.conn.execute("DELETE FROM categories WHERE id = ?1", [id])?;
        Ok(())
    }

    // Transactions

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

    // Bills

    /// Saves a bill, or one bill per installment with the total split so it adds up exactly.
    /// Returns the keys.
    pub fn add_bill(&self, input: &BillInput) -> DbResult<Vec<i64>> {
        let parts = input.installments.max(1);
        let anchor = input.due.day();
        // Installments end by themselves, so they never repeat.
        let recurrence = if parts > 1 { Recurrence::None } else { input.recurrence };
        self.atomically(|db| {
            let mut ids = Vec::with_capacity(parts as usize);
            for (i, amount) in split_installments(input.amount_cents, parts).into_iter().enumerate() {
                db.conn.execute(
                    "INSERT INTO bills (description, category_id, member_id, amount_cents, due, recurrence,
                                        anchor_day, installment, installments, barcode)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        input.description.trim(),
                        input.category_id,
                        input.member_id,
                        amount,
                        shift_months(input.due, i as i32, anchor).to_string(),
                        recurrence.code(),
                        anchor,
                        i as u32 + 1,
                        parts,
                        input.barcode.trim(),
                    ],
                )?;
                ids.push(db.conn.last_insert_rowid());
            }
            Ok(ids)
        })
    }

    /// Changes one unpaid bill.
    pub fn update_bill(&self, id: i64, input: &BillInput) -> DbResult<()> {
        self.conn.execute(
            "UPDATE bills SET description = ?2, category_id = ?3, member_id = ?4, amount_cents = ?5, due = ?6,
                    recurrence = ?7, anchor_day = ?8, barcode = ?9
             WHERE id = ?1",
            params![
                id,
                input.description.trim(),
                input.category_id,
                input.member_id,
                input.amount_cents,
                input.due.to_string(),
                input.recurrence.code(),
                input.due.day(),
                input.barcode.trim(),
            ],
        )?;
        Ok(())
    }

    /// Removes a bill; a paid one has to be reopened first, so its transaction is not left behind.
    pub fn delete_bill(&self, id: i64) -> DbResult<()> {
        if self.bill(id)?.is_some_and(|b| b.paid_on.is_some()) {
            return Err(DbError::Rule(
                "Esta conta já foi paga. Desfaça o pagamento antes de excluir.".to_owned(),
            ));
        }
        self.conn.execute("DELETE FROM bills WHERE id = ?1", [id])?;
        Ok(())
    }

    /// One bill.
    pub fn bill(&self, id: i64) -> DbResult<Option<Bill>> {
        let sql = format!("SELECT {BILL_COLUMNS} {BILL_JOINS} WHERE b.id = ?1");
        Ok(self.conn.prepare_cached(&sql)?.query_row([id], bill_from_row).optional()?)
    }

    /// Every bill by due date, oldest first.
    pub fn bills(&self) -> DbResult<Vec<Bill>> {
        let sql = format!("SELECT {BILL_COLUMNS} {BILL_JOINS} ORDER BY b.due, b.id");
        let mut query = self.conn.prepare_cached(&sql)?;
        let rows = query.query_map([], bill_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Unpaid bills by due date, oldest first.
    pub fn open_bills(&self) -> DbResult<Vec<Bill>> {
        let sql = format!("SELECT {BILL_COLUMNS} {BILL_JOINS} WHERE b.paid_on IS NULL ORDER BY b.due, b.id");
        let mut query = self.conn.prepare_cached(&sql)?;
        let rows = query.query_map([], bill_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Pays a bill from an account: records the expense, marks the bill paid and,
    /// when the bill repeats, schedules the next one. All of it or none of it.
    /// Returns the new transaction's key.
    pub fn pay_bill(&self, id: i64, account_id: i64, paid_on: Date, amount_cents: i64) -> DbResult<i64> {
        self.atomically(|db| {
            let Some(bill) = db.bill(id)? else {
                return Err(DbError::Rule("Esta conta não existe mais.".to_owned()));
            };
            if bill.paid_on.is_some() {
                return Err(DbError::Rule("Esta conta já foi paga.".to_owned()));
            }
            let transaction = db.add_transaction(&TransactionInput {
                date: paid_on,
                description: bill.title(),
                amount_cents,
                flow: Flow::Expense,
                account_id,
                category_id: bill.category_id,
                member_id: bill.member_id,
            })?;
            db.conn.execute(
                "UPDATE bills SET paid_on = ?2, transaction_id = ?3 WHERE id = ?1",
                params![id, paid_on.to_string(), transaction],
            )?;
            db.schedule_next(&bill)?;
            Ok(transaction)
        })
    }

    /// Undoes a payment: the bill is open again and the expense goes away.
    pub fn reopen_bill(&self, id: i64) -> DbResult<()> {
        self.atomically(|db| {
            let transaction = db.bill(id)?.and_then(|b| b.transaction_id);
            db.conn.execute("UPDATE bills SET paid_on = NULL, transaction_id = NULL WHERE id = ?1", [id])?;
            if let Some(transaction) = transaction {
                db.conn.execute("DELETE FROM transactions WHERE id = ?1", [transaction])?;
            }
            Ok(())
        })
    }

    fn schedule_next(&self, bill: &Bill) -> DbResult<()> {
        let months = match bill.recurrence {
            Recurrence::None => return Ok(()),
            Recurrence::Monthly => 1,
            Recurrence::Yearly => 12,
        };
        let next = shift_months(bill.due, months, bill.anchor_day);
        // Paying, undoing and paying again must not schedule the next one twice.
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bills WHERE description = ?1 AND due = ?2",
            params![bill.description, next.to_string()],
            |r| r.get(0),
        )?;
        if exists == 0 {
            self.conn.execute(
                "INSERT INTO bills (description, category_id, member_id, amount_cents, due, recurrence, anchor_day)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    bill.description,
                    bill.category_id,
                    bill.member_id,
                    bill.amount_cents,
                    next.to_string(),
                    bill.recurrence.code(),
                    bill.anchor_day
                ],
            )?;
        }
        Ok(())
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

fn date_at(row: &Row<'_>, index: usize) -> rusqlite::Result<Date> {
    let text: String = row.get(index)?;
    parse_date(&text).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            format!("invalid date: {text}").into(),
        )
    })
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

fn bill_from_row(r: &Row<'_>) -> rusqlite::Result<Bill> {
    Ok(Bill {
        id: r.get(0)?,
        description: r.get(1)?,
        category_id: r.get(2)?,
        category: r.get(3)?,
        member_id: r.get(4)?,
        member: r.get(5)?,
        amount_cents: r.get(6)?,
        due: date_at(r, 7)?,
        recurrence: Recurrence::from_code(&r.get::<_, String>(8)?),
        anchor_day: r.get(9)?,
        installment: r.get(10)?,
        installments: r.get(11)?,
        barcode: r.get(12)?,
        paid_on: r.get::<_, Option<String>>(13)?.as_deref().and_then(parse_date),
        transaction_id: r.get(14)?,
    })
}
