//! The records the rest refers to: members, accounts and categories with their budgets.

use rusqlite::params;

use super::{Database, DbResult, category_key, category_name};
use crate::model::{Account, AccountKind, Category, Flow, Member};

impl Database {
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

    /// The categories of one flow, the most used first, then by name.
    pub fn categories_by_use(&self, flow: Flow) -> DbResult<Vec<Category>> {
        let mut query = self.conn.prepare_cached(
            "SELECT c.id, c.name, c.flow, c.budget_cents FROM categories c
             LEFT JOIN transactions t ON t.category_id = c.id
             WHERE c.flow = ?1 GROUP BY c.id ORDER BY count(t.id) DESC, c.name",
        )?;
        let rows = query.query_map([flow.code()], |r| {
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
        let name = category_name(name);
        self.conn.execute(
            "INSERT INTO categories (name, normalized_name, flow, budget_cents) VALUES (?1, ?2, ?3, ?4)",
            params![name, category_key(&name), flow.code(), budget_cents],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Changes a category.
    pub fn update_category(&self, id: i64, name: &str, flow: Flow, budget_cents: i64) -> DbResult<()> {
        let name = category_name(name);
        self.conn.execute(
            "UPDATE categories SET name = ?2, normalized_name = ?3, flow = ?4, budget_cents = ?5 WHERE id = ?1",
            params![id, name, category_key(&name), flow.code(), budget_cents],
        )?;
        Ok(())
    }

    /// Finds a category under its normalized name, or creates it with no monthly limit.
    pub fn find_or_create_category(&self, name: &str, flow: Flow) -> DbResult<i64> {
        let name = category_name(name);
        let key = category_key(&name);
        self.atomically(|db| {
            db.conn.execute(
                "INSERT INTO categories (name, normalized_name, flow, budget_cents)
                 VALUES (?1, ?2, ?3, 0)
                 ON CONFLICT(flow, normalized_name) DO NOTHING",
                params![name, key, flow.code()],
            )?;
            Ok(db.conn.query_row(
                "SELECT id FROM categories WHERE flow = ?1 AND normalized_name = ?2",
                params![flow.code(), key],
                |row| row.get(0),
            )?)
        })
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
}
