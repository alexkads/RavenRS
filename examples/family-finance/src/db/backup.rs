//! Copies of the whole file: making one, reading what one holds and restoring from it.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::{Database, DbError, DbResult, MIGRATIONS};

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

impl Database {
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
}
