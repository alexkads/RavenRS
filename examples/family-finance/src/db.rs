//! Everything the app stores, in one SQLite file.
//!
//! Money is kept in whole cents and dates as ISO text, which sorts by date.
//! Writes that touch more than one row run in a transaction, so paying a bill
//! can never leave a payment without its transaction.

use std::fmt;
use std::path::Path;

use ravenrs::prelude::{Date, Frequency, RecurrenceRule};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, Row, params, params_from_iter};

use crate::filter::CompiledFilter;
use crate::model::{
    Account, AccountKind, Bill, BillInput, BudgetLine, Category, Flow, Member, MonthTotals, Occurrence,
    Recurrence, Schedule, ScheduleInput, Transaction, TransactionInput, month_range, parse_date,
    shift_months,
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
const MIGRATIONS: &[&str] = &[
    r#"
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
"#,
    r#"
    -- What is planned: the days come from the rule, not from a row per day.
    CREATE TABLE schedules (
        id INTEGER PRIMARY KEY,
        description TEXT NOT NULL CHECK (length(trim(description)) > 0),
        flow TEXT NOT NULL CHECK (flow IN ('expense', 'income')),
        amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
        account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE RESTRICT,
        category_id INTEGER NOT NULL REFERENCES categories (id) ON DELETE RESTRICT,
        member_id INTEGER REFERENCES members (id) ON DELETE SET NULL,
        start TEXT NOT NULL,
        rule TEXT NOT NULL
    );
    -- What happened to one day of a rule. No row means it is still ahead, on its own day.
    -- Deleting the transaction deletes the row, so the day is pending again.
    CREATE TABLE schedule_days (
        schedule_id INTEGER NOT NULL REFERENCES schedules (id) ON DELETE CASCADE,
        day TEXT NOT NULL,
        moved_to TEXT,
        skipped INTEGER NOT NULL DEFAULT 0,
        transaction_id INTEGER REFERENCES transactions (id) ON DELETE CASCADE,
        PRIMARY KEY (schedule_id, day)
    );
"#,
];

const BILL_COLUMNS: &str = "b.id, b.description, b.category_id, c.name, b.member_id, COALESCE(m.name, ''),
    b.amount_cents, b.due, b.recurrence, b.anchor_day, b.installment, b.installments, b.barcode, b.paid_on,
    b.transaction_id";
const BILL_JOINS: &str =
    "FROM bills b JOIN categories c ON c.id = b.category_id LEFT JOIN members m ON m.id = b.member_id";

/// What a backup file holds, read before anything is replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupContents {
    /// The number of migrations it has.
    pub version: i64,
    /// How many transactions.
    pub transactions: i64,
    /// How many bills.
    pub bills: i64,
    /// How many accounts.
    pub accounts: i64,
    /// The day of its latest transaction, as stored.
    pub last_date: Option<String>,
}

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
                "DELETE FROM schedules; DELETE FROM bills; DELETE FROM transactions; DELETE FROM categories;
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

    /// Reads what a file holds, without changing it. A file that is not a backup
    /// of this app, is damaged or came from a newer version says so in words.
    pub fn inspect_backup(path: &Path) -> DbResult<BackupContents> {
        const NOT_A_BACKUP: &str = "Esse arquivo não é um backup do Raven Finanças.";
        let not_a_backup = |_| DbError::Rule(NOT_A_BACKUP.to_owned());
        let conn =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(not_a_backup)?;
        let version: i64 =
            conn.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(not_a_backup)?;
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
                 AND name IN ('members', 'accounts', 'categories', 'transactions', 'bills')",
                [],
                |r| r.get(0),
            )
            .map_err(not_a_backup)?;
        if version < 1 || tables != 5 {
            return Err(DbError::Rule(NOT_A_BACKUP.to_owned()));
        }
        if version > MIGRATIONS.len() as i64 {
            return Err(DbError::Rule(
                "Esse backup veio de uma versão mais nova do app. Atualize o app e tente de novo.".to_owned(),
            ));
        }
        let check: String = conn
            .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
            .map_err(|_| DbError::Rule("Esse backup está danificado.".to_owned()))?;
        if check != "ok" {
            return Err(DbError::Rule("Esse backup está danificado.".to_owned()));
        }
        let count = |table: &str| -> DbResult<i64> {
            Ok(conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
        };
        Ok(BackupContents {
            version,
            transactions: count("transactions")?,
            bills: count("bills")?,
            accounts: count("accounts")?,
            last_date: conn.query_row("SELECT MAX(date) FROM transactions", [], |r| r.get(0))?,
        })
    }

    /// What this database holds now, in the same terms as a backup.
    pub fn contents(&self) -> DbResult<BackupContents> {
        let count = |table: &str| -> DbResult<i64> {
            Ok(self.conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
        };
        Ok(BackupContents {
            version: self.schema_version()?,
            transactions: count("transactions")?,
            bills: count("bills")?,
            accounts: count("accounts")?,
            last_date: self.conn.query_row("SELECT MAX(date) FROM transactions", [], |r| r.get(0))?,
        })
    }

    /// Replaces all the data with what is in the backup at `path`, or changes
    /// nothing. The backup is left as it was: a copy of it is what gets migrated,
    /// so one from an older version works, and then both have the same tables.
    pub fn restore_from(&self, path: &Path) -> DbResult<()> {
        Self::inspect_backup(path)?;
        let copy = std::env::temp_dir().join(format!("raven-financas-restaurando-{}.db", std::process::id()));
        Self::remove_database_files(&copy);
        let result = std::fs::copy(path, &copy)
            .map_err(|e| DbError::Rule(format!("Não foi possível ler o backup: {e}")))
            .and_then(|_| Self::open(&copy).map(drop))
            .and_then(|()| self.take_all_from(&copy));
        Self::remove_database_files(&copy);
        result
    }

    fn remove_database_files(path: &Path) {
        for end in ["", "-wal", "-shm"] {
            let mut file = path.as_os_str().to_owned();
            file.push(end);
            let _ = std::fs::remove_file(file);
        }
    }

    fn take_all_from(&self, path: &Path) -> DbResult<()> {
        self.conn.execute("ATTACH ?1 AS backup", [path.to_string_lossy()])?;
        // Rows arrive in any order, and `ON DELETE RESTRICT` acts at once even
        // when deferred, so the check has to wait until they are all in.
        self.conn.pragma_update(None, "foreign_keys", false)?;
        let result = self.atomically(|db| {
            let names: Vec<String> = {
                let mut query = db.conn.prepare(
                    "SELECT name FROM main.sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                )?;
                query.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
            };
            for name in names {
                db.conn.execute_batch(&format!(
                    "DELETE FROM main.\"{name}\"; INSERT INTO main.\"{name}\" SELECT * FROM backup.\"{name}\";"
                ))?;
            }
            let broken: i64 =
                db.conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
            if broken > 0 {
                return Err(DbError::Rule("Esse backup está danificado.".to_owned()));
            }
            Ok(())
        });
        let on = self.conn.pragma_update(None, "foreign_keys", true);
        let detached = self.conn.execute("DETACH backup", []);
        result?;
        on?;
        detached?;
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

    /// Moves an unpaid bill to another day and person. It comes back on the new day from then on.
    pub fn move_bill(&self, id: i64, due: Date, member_id: Option<i64>) -> DbResult<()> {
        self.conn.execute(
            "UPDATE bills SET due = ?2, anchor_day = ?3, member_id = ?4 WHERE id = ?1 AND paid_on IS NULL",
            params![id, due.to_string(), due.day(), member_id],
        )?;
        Ok(())
    }

    // Schedules

    /// Saves something planned and returns the key.
    pub fn add_schedule(&self, input: &ScheduleInput) -> DbResult<i64> {
        self.conn.execute(
            "INSERT INTO schedules (description, flow, amount_cents, account_id, category_id, member_id, start, rule)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                input.description.trim(),
                input.flow.code(),
                input.amount_cents,
                input.account_id,
                input.category_id,
                input.member_id,
                input.start.to_string(),
                input.rule.to_text(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Changes what is planned. Days already confirmed or moved stay as they are.
    pub fn update_schedule(&self, id: i64, input: &ScheduleInput) -> DbResult<()> {
        self.conn.execute(
            "UPDATE schedules SET description = ?2, flow = ?3, amount_cents = ?4, account_id = ?5,
                    category_id = ?6, member_id = ?7, start = ?8, rule = ?9 WHERE id = ?1",
            params![
                id,
                input.description.trim(),
                input.flow.code(),
                input.amount_cents,
                input.account_id,
                input.category_id,
                input.member_id,
                input.start.to_string(),
                input.rule.to_text(),
            ],
        )?;
        Ok(())
    }

    /// Stops planning it. What already became a transaction stays.
    pub fn delete_schedule(&self, id: i64) -> DbResult<()> {
        self.conn.execute("DELETE FROM schedules WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Everything planned.
    pub fn schedules(&self) -> DbResult<Vec<Schedule>> {
        let mut query = self.conn.prepare_cached(
            "SELECT s.id, s.description, s.flow, s.amount_cents, s.account_id, s.category_id, s.member_id,
                    s.start, s.rule, a.name, c.name, COALESCE(m.name, '')
             FROM schedules s JOIN accounts a ON a.id = s.account_id JOIN categories c ON c.id = s.category_id
             LEFT JOIN members m ON m.id = s.member_id ORDER BY s.start, s.id",
        )?;
        let rows = query.query_map([], |r| {
            let rule: String = r.get(8)?;
            Ok(Schedule {
                id: r.get(0)?,
                input: ScheduleInput {
                    description: r.get(1)?,
                    flow: Flow::from_code(&r.get::<_, String>(2)?),
                    amount_cents: r.get(3)?,
                    account_id: r.get(4)?,
                    category_id: r.get(5)?,
                    member_id: r.get(6)?,
                    start: date_at(r, 7)?,
                    // A rule the app cannot read happens once rather than hide what was planned.
                    rule: RecurrenceRule::parse(&rule).unwrap_or(RecurrenceRule::new(Frequency::Once)),
                },
                account: r.get(9)?,
                category: r.get(10)?,
                member: r.get(11)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One thing planned.
    pub fn schedule(&self, id: i64) -> DbResult<Option<Schedule>> {
        Ok(self.schedules()?.into_iter().find(|s| s.id == id))
    }

    /// What falls on `from..=to`, by the day it falls on: each rule's days, with
    /// those moved in or out of the range accounted for. Skipped days are left out.
    pub fn occurrences(&self, from: Date, to: Date) -> DbResult<Vec<Occurrence>> {
        let mut query = self.conn.prepare_cached(
            "SELECT day, moved_to, skipped, transaction_id FROM schedule_days WHERE schedule_id = ?1",
        )?;
        let mut found = Vec::new();
        for schedule in self.schedules()? {
            let marks = query.query_map([schedule.id], |r| {
                Ok((
                    date_at(r, 0)?,
                    r.get::<_, Option<String>>(1)?.as_deref().and_then(parse_date),
                    r.get::<_, bool>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })?;
            let marks: Vec<_> = marks.collect::<Result<_, _>>()?;
            let make = |day: Date, when: Date, transaction_id: Option<i64>| Occurrence {
                schedule_id: schedule.id,
                day,
                when,
                transaction_id,
                schedule: schedule.clone(),
            };
            for day in schedule.input.rule.occurrences(schedule.input.start, from, to) {
                if !marks.iter().any(|mark| mark.0 == day) {
                    found.push(make(day, day, None));
                }
            }
            for (day, moved_to, skipped, transaction_id) in marks {
                let when = moved_to.unwrap_or(day);
                if !skipped && when >= from && when <= to {
                    found.push(make(day, when, transaction_id));
                }
            }
        }
        found.sort_by_key(|o| (o.when, o.schedule_id));
        Ok(found)
    }

    /// Records that a planned day happened: the transaction goes in and the day is done.
    pub fn confirm_occurrence(
        &self,
        schedule_id: i64,
        day: Date,
        account_id: i64,
        when: Date,
        amount_cents: i64,
    ) -> DbResult<i64> {
        self.atomically(|db| {
            let Some(schedule) = db.schedule(schedule_id)? else {
                return Err(DbError::Rule("Esta programação não existe mais.".to_owned()));
            };
            let done: Option<Option<i64>> = db
                .conn
                .query_row(
                    "SELECT transaction_id FROM schedule_days WHERE schedule_id = ?1 AND day = ?2",
                    params![schedule_id, day.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            if done.flatten().is_some() {
                return Err(DbError::Rule("Este dia já foi confirmado.".to_owned()));
            }
            let transaction = db.add_transaction(&TransactionInput {
                date: when,
                description: schedule.input.description.clone(),
                amount_cents,
                flow: schedule.input.flow,
                account_id,
                category_id: schedule.input.category_id,
                member_id: schedule.input.member_id,
            })?;
            db.mark_day(schedule_id, day, Some(when), false, Some(transaction))?;
            Ok(transaction)
        })
    }

    /// Undoes a confirmation: the transaction goes away and the day is ahead again.
    pub fn undo_occurrence(&self, transaction_id: i64) -> DbResult<()> {
        self.delete_transaction(transaction_id)
    }

    /// Leaves one day out, keeping the rule.
    pub fn skip_occurrence(&self, schedule_id: i64, day: Date) -> DbResult<()> {
        self.mark_day(schedule_id, day, None, true, None)
    }

    /// Moves one day of a rule, keeping the others where they are.
    pub fn move_occurrence(&self, schedule_id: i64, day: Date, to: Date) -> DbResult<()> {
        self.mark_day(schedule_id, day, Some(to), false, None)
    }

    fn mark_day(
        &self,
        schedule_id: i64,
        day: Date,
        moved_to: Option<Date>,
        skipped: bool,
        transaction_id: Option<i64>,
    ) -> DbResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO schedule_days (schedule_id, day, moved_to, skipped, transaction_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![schedule_id, day.to_string(), moved_to.map(|d| d.to_string()), skipped, transaction_id],
        )?;
        Ok(())
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
