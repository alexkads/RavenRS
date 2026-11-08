//! Everything the app stores, in one SQLite file.
//!
//! Money is kept in whole cents and dates as ISO text, which sorts by date.
//! Writes that touch more than one row run in a transaction, so paying a bill
//! can never leave a payment without its transaction.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use ravenrs::prelude::Date;
use rusqlite::{Connection, ErrorCode, Row, params};

use crate::model::parse_date;

mod backup;
mod bills;
mod ledger;
mod records;
mod schedules;
mod telegram;

pub use backup::BackupContents;
pub use ledger::LedgerRow;
pub use telegram::TelegramLink;

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
    r#"
    ALTER TABLE categories ADD COLUMN normalized_name TEXT;
"#,
    r#"
    CREATE TABLE telegram_pairings (
        id INTEGER PRIMARY KEY,
        token_hash TEXT NOT NULL UNIQUE,
        chat_id INTEGER UNIQUE,
        user_id INTEGER,
        status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'revoked', 'expired')),
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        paired_at INTEGER,
        revoked_at INTEGER
    );
    CREATE INDEX telegram_pairings_active ON telegram_pairings (chat_id, user_id, status);
"#,
    r#"
    -- Several chats at once, one per person: the member a QR was made for.
    ALTER TABLE telegram_pairings ADD COLUMN member_id INTEGER REFERENCES members (id) ON DELETE SET NULL;
"#,
];

/// The visible name keeps its accents and casing; the key makes equivalent names one category.
fn category_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn category_key(name: &str) -> String {
    category_name(name)
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// The app's database.
#[derive(Debug)]
pub struct Database {
    conn: Rc<Connection>,
}

impl Database {
    /// Opens the file at `path`, creating it and its folder when missing.
    pub fn open(path: &Path) -> DbResult<Self> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)
                .map_err(|e| DbError::Rule(format!("Não foi possível criar a pasta dos dados: {e}")))?;
        }
        let db = Self { conn: Rc::new(Connection::open(path)?) };
        // Write-ahead logging survives a crash mid-write without losing what was saved.
        db.conn.pragma_update(None, "journal_mode", "WAL")?;
        db.prepare()?;
        Ok(db)
    }

    /// The connection, for what reads and writes records through it, such as generated screens.
    pub fn connection(&self) -> Rc<Connection> {
        self.conn.clone()
    }

    /// A database that lasts as long as this value, for tests.
    pub fn in_memory() -> DbResult<Self> {
        let db = Self { conn: Rc::new(Connection::open_in_memory()?) };
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
        self.fill_category_keys()?;
        self.conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS categories_flow_normalized_name
             ON categories (flow, normalized_name)",
        )?;
        Ok(())
    }

    fn fill_category_keys(&self) -> DbResult<()> {
        let mut query = self.conn.prepare_cached("SELECT id, name, flow FROM categories")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        let categories = rows.collect::<Result<Vec<_>, _>>()?;
        let mut seen = HashSet::new();
        let keys = categories
            .iter()
            .filter_map(|(id, name, flow)| {
                let key = category_key(name);
                seen.insert((flow.clone(), key.clone())).then_some((*id, key))
            })
            .collect::<Vec<_>>();
        self.atomically(|db| {
            for (id, key) in &keys {
                db.conn
                    .execute("UPDATE categories SET normalized_name = ?2 WHERE id = ?1", params![id, key])?;
            }
            Ok(())
        })
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
