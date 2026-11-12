//! Backups of a whole database, and the copies an application keeps by itself.
//!
//! A backup is a file of its own, written while the database is in use and checked before it is
//! kept. Each one carries a stamp: the application, the tenant and the migrations it was made
//! with. Restoring reads the stamp first and refuses another application, another tenant or other
//! migrations, then keeps a copy of what is there now, then replaces everything at once. If
//! anything fails along the way, the database stays as it was.
//!
//! ```
//! use std::rc::Rc;
//!
//! use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace};
//! use ravenrs_app_sqlite::{BackupStamp, SqliteDatabase, TableMap, SqliteBackups};
//!
//! let member = EntityInfo::new("Member")
//!     .field(FieldInfo::new("id", FieldKind::Integer).key())
//!     .field(FieldInfo::new("name", FieldKind::Text));
//! let connection = Rc::new(rusqlite::Connection::open_in_memory()?);
//! let database = SqliteDatabase::builder(connection).managed(member, TableMap::new("members"))?.build();
//! let folder = std::env::temp_dir().join(format!("ravenrs-backup-doc-{}", std::process::id()));
//! # let _ = std::fs::remove_dir_all(&folder);
//! let backups = database.backups(BackupStamp::new("Church").tenant("central")).copies_in(folder.join("copies"));
//!
//! let file = folder.join("today.db");
//! backups.save(&file)?;
//! let what = backups.inspect(&file)?;
//! assert_eq!(what.stamp.tenant.as_deref(), Some("central"));
//! backups.restore(&file)?;
//! # std::fs::remove_dir_all(&folder)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::{MIGRATIONS, SqliteDatabase, quoted};

/// The table in a backup that says what it is. Only backups have it.
const STAMP: &str = "ravenrs_backup";

/// How many copies stay in the folder unless said otherwise; the oldest go first.
pub const COPIES_KEPT: usize = 14;

/// What a backup says about where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackupStamp {
    /// The application, which a backup has to be of to be restored.
    pub application: String,
    /// The tenant, such as one organization of many, which a backup has to be of too.
    pub tenant: Option<String>,
    /// The version of the application that made it, to show; it is not compared.
    pub version: Option<String>,
    /// The migrations the database had, in order. Filled in when a backup is made.
    pub migrations: Vec<String>,
}

impl BackupStamp {
    /// Backups of this application.
    pub fn new(application: impl Into<String>) -> Self {
        Self { application: application.into(), tenant: None, version: None, migrations: Vec::new() }
    }

    /// Of this tenant only: one of another is refused.
    pub fn tenant(mut self, tenant: impl Into<String>) -> Self {
        self.tenant = Some(tenant.into());
        self
    }

    /// Made by this version of the application.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

/// What a backup holds, read without changing it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackupInfo {
    /// Where it came from.
    pub stamp: BackupStamp,
    /// When it was made, in seconds since 1970.
    pub made: u64,
    /// How many rows each table has, by table, without its own bookkeeping.
    pub rows: Vec<(String, i64)>,
}

/// Why a backup was not made, read or restored.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackupError {
    /// The file is the database in use.
    SameFile,
    /// Something is already there, and a backup does not overwrite it.
    Exists(PathBuf),
    /// The file is not a backup, or not one this library made.
    NotABackup,
    /// It is a backup of another application.
    OtherApplication(String),
    /// It is a backup of another tenant, or of none when one is expected.
    OtherTenant(Option<String>),
    /// It was made with other migrations: an older or a newer version of the application.
    OtherMigrations,
    /// A table of this database is missing from it.
    MissingTable(String),
    /// It is damaged.
    Damaged,
    /// Its rows do not hold together: a reference points at nothing.
    Broken,
    /// Reading or writing failed.
    Storage(String),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameFile => write!(f, "that is the database in use"),
            Self::Exists(path) => write!(f, "there is already a file at {}", path.display()),
            Self::NotABackup => write!(f, "that file is not a backup"),
            Self::OtherApplication(name) => write!(f, "that is a backup of {name}"),
            Self::OtherTenant(Some(name)) => write!(f, "that is a backup of {name}"),
            Self::OtherTenant(None) => write!(f, "that backup is of no one in particular"),
            Self::OtherMigrations => write!(f, "that backup is of another version of the application"),
            Self::MissingTable(table) => write!(f, "that backup has no table {table}"),
            Self::Damaged => write!(f, "that backup is damaged"),
            Self::Broken => write!(f, "the records of that backup do not hold together"),
            Self::Storage(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for BackupError {}

fn storage(error: impl fmt::Display) -> BackupError {
    BackupError::Storage(error.to_string())
}

/// Why a copy was kept, as its file name says.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct KeptCopy {
    /// The file.
    pub path: PathBuf,
    /// Why, as given to [`Backups::keep_copy`].
    pub reason: String,
    /// When, in seconds since 1970.
    pub made: u64,
    /// Which of the copies of that second it is, from 0.
    pub sequence: u32,
}

/// Backups of one database. See [`SqliteDatabase::backups`].
#[derive(Clone)]
pub struct Backups {
    database: SqliteDatabase,
    stamp: BackupStamp,
    folder: Option<PathBuf>,
    keep: usize,
}

impl fmt::Debug for Backups {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Backups").field("stamp", &self.stamp).field("folder", &self.folder).finish()
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}

/// The tables of a database that hold records, without SQLite's and a backup's own.
fn tables_of(connection: &Connection, schema: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(&format!(
        "SELECT name FROM {schema}.sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
         AND name <> '{STAMP}' ORDER BY name"
    ))?;
    statement.query_map([], |row| row.get(0))?.collect()
}

fn columns_of(connection: &Connection, schema: &str, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA {schema}.table_info({})", quoted(table)))?;
    statement.query_map([], |row| row.get(1))?.collect()
}

fn migrations_of(connection: &Connection, schema: &str) -> rusqlite::Result<Vec<String>> {
    let has: bool = connection
        .prepare(&format!("SELECT 1 FROM {schema}.sqlite_master WHERE type = 'table' AND name = ?1"))?
        .exists([MIGRATIONS])?;
    if !has {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(&format!("SELECT id FROM {schema}.{MIGRATIONS} ORDER BY id"))?;
    statement.query_map([], |row| row.get(0))?.collect()
}

fn remove_database_files(path: &Path) {
    for end in ["", "-wal", "-shm", "-journal"] {
        let mut file = path.as_os_str().to_owned();
        file.push(end);
        let _ = std::fs::remove_file(file);
    }
}

/// Backups of a SQLite database.
pub trait SqliteBackups {
    /// Backups of this database, stamped with this.
    fn backups(&self, stamp: BackupStamp) -> Backups;
}

impl SqliteBackups for SqliteDatabase {
    fn backups(&self, stamp: BackupStamp) -> Backups {
        Backups { database: self.clone(), stamp, folder: None, keep: COPIES_KEPT }
    }
}

impl Backups {
    /// Keeps the copies made before a restore, and by [`keep_copy`](Self::keep_copy), in this
    /// folder. Without one, a restore keeps no copy.
    pub fn copies_in(mut self, folder: impl Into<PathBuf>) -> Self {
        self.folder = Some(folder.into());
        self
    }

    /// How many copies stay in the folder; the oldest go first. [`COPIES_KEPT`] unless said.
    pub fn keep(mut self, copies: usize) -> Self {
        self.keep = copies.max(1);
        self
    }

    fn connection(&self) -> &Connection {
        self.database.backend().connection()
    }

    /// The file of the database in use, if it is in one.
    fn own_file(&self) -> Option<PathBuf> {
        self.connection().path().filter(|path| !path.is_empty()).map(PathBuf::from)
    }

    fn is_own(&self, path: &Path) -> bool {
        let own = self.own_file().and_then(|own| std::fs::canonicalize(own).ok());
        own.is_some() && own == std::fs::canonicalize(path).ok()
    }

    /// Writes a backup to `target`, consistent even while the database is in use, and checks it
    /// before it is there. It never overwrites a file.
    pub fn save(&self, target: &Path) -> Result<BackupInfo, BackupError> {
        if self.is_own(target) {
            return Err(BackupError::SameFile);
        }
        if target.exists() {
            return Err(BackupError::Exists(target.to_owned()));
        }
        if let Some(folder) = target.parent().filter(|folder| !folder.as_os_str().is_empty()) {
            std::fs::create_dir_all(folder).map_err(storage)?;
        }
        let mut draft = target.as_os_str().to_owned();
        draft.push(".parcial");
        let draft = PathBuf::from(draft);
        remove_database_files(&draft);
        let written = self.write_draft(&draft).and_then(|()| self.inspect(&draft));
        let info = match written {
            Ok(info) => info,
            Err(error) => {
                remove_database_files(&draft);
                return Err(error);
            }
        };
        std::fs::rename(&draft, target).map_err(|error| {
            remove_database_files(&draft);
            storage(error)
        })?;
        Ok(info)
    }

    fn write_draft(&self, draft: &Path) -> Result<(), BackupError> {
        // A copy of what is committed, whatever is being written meanwhile.
        self.connection().execute("VACUUM INTO ?1", [draft.to_string_lossy()]).map_err(storage)?;
        let copy = Connection::open(draft).map_err(storage)?;
        let stamp = &self.stamp;
        let migrations = migrations_of(&copy, "main").map_err(storage)?;
        copy.execute_batch(&format!("CREATE TABLE {STAMP} (key TEXT PRIMARY KEY, value TEXT)"))
            .map_err(storage)?;
        let rows: [(&str, Option<String>); 5] = [
            ("application", Some(stamp.application.clone())),
            ("tenant", stamp.tenant.clone()),
            ("version", stamp.version.clone()),
            ("migrations", Some(migrations.join("\n"))),
            ("made", Some(now().to_string())),
        ];
        for (key, value) in rows {
            copy.execute(&format!("INSERT INTO {STAMP} (key, value) VALUES (?1, ?2)"), (key, value))
                .map_err(storage)?;
        }
        Ok(())
    }

    /// Reads what a backup holds and whether this database can take it, without changing either.
    pub fn inspect(&self, path: &Path) -> Result<BackupInfo, BackupError> {
        if self.is_own(path) {
            return Err(BackupError::SameFile);
        }
        let file = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| BackupError::NotABackup)?;
        let check: String = file
            .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
            .map_err(|_| BackupError::NotABackup)?;
        if check != "ok" {
            return Err(BackupError::Damaged);
        }
        let read = |key: &str| -> Result<Option<String>, BackupError> {
            file.query_row(&format!("SELECT value FROM {STAMP} WHERE key = ?1"), [key], |row| row.get(0))
                .optional()
                .map_err(|_| BackupError::NotABackup)
                .map(Option::flatten)
        };
        let application = read("application")?.ok_or(BackupError::NotABackup)?;
        let stamp = BackupStamp {
            application,
            tenant: read("tenant")?,
            version: read("version")?,
            migrations: read("migrations")?
                .map(|text| text.lines().filter(|line| !line.is_empty()).map(str::to_owned).collect())
                .unwrap_or_default(),
        };
        if stamp.application != self.stamp.application {
            return Err(BackupError::OtherApplication(stamp.application));
        }
        if stamp.tenant != self.stamp.tenant {
            return Err(BackupError::OtherTenant(stamp.tenant));
        }
        if stamp.migrations != migrations_of(self.connection(), "main").map_err(storage)? {
            return Err(BackupError::OtherMigrations);
        }
        let theirs = tables_of(&file, "main").map_err(storage)?;
        for table in tables_of(self.connection(), "main").map_err(storage)? {
            if !theirs.contains(&table) {
                return Err(BackupError::MissingTable(table));
            }
        }
        let rows = theirs
            .iter()
            .filter(|table| table.as_str() != MIGRATIONS)
            .map(|table| {
                let count = file.query_row(&format!("SELECT COUNT(*) FROM {}", quoted(table)), [], |row| {
                    row.get::<_, i64>(0)
                });
                count.map(|count| (table.clone(), count)).map_err(storage)
            })
            .collect::<Result<_, _>>()?;
        let made = read("made")?.and_then(|text| text.parse().ok()).unwrap_or(0);
        Ok(BackupInfo { stamp, made, rows })
    }

    /// Replaces everything in the database with the backup at `path`, or changes nothing. A copy
    /// of what is there now is kept first, in the folder of [`copies_in`](Self::copies_in), and
    /// returned. The backup itself is left as it was.
    pub fn restore(&self, path: &Path) -> Result<(BackupInfo, Option<PathBuf>), BackupError> {
        let info = self.inspect(path)?;
        let copy = match &self.folder {
            Some(_) => Some(self.keep_copy("before-restore")?),
            None => None,
        };
        self.take_all_from(path)?;
        // Keys are counted again from what is there now.
        self.database.forget_next_keys();
        Ok((info, copy))
    }

    fn take_all_from(&self, path: &Path) -> Result<(), BackupError> {
        let connection = self.connection();
        connection.execute("ATTACH ?1 AS backup", [path.to_string_lossy()]).map_err(storage)?;
        // Rows arrive in any order, so references are checked once they are all in.
        let enforced: i64 =
            connection.pragma_query_value(None, "foreign_keys", |row| row.get(0)).map_err(storage)?;
        connection.pragma_update(None, "foreign_keys", false).map_err(storage)?;
        let result = self.replace_all();
        let _ = connection.pragma_update(None, "foreign_keys", enforced);
        let _ = connection.execute("DETACH backup", []);
        result
    }

    fn replace_all(&self) -> Result<(), BackupError> {
        let connection = self.connection();
        let transaction = connection.unchecked_transaction().map_err(storage)?;
        for table in tables_of(connection, "main").map_err(storage)? {
            if table == MIGRATIONS {
                continue;
            }
            let theirs = columns_of(connection, "backup", &table).map_err(storage)?;
            let shared: Vec<String> = columns_of(connection, "main", &table)
                .map_err(storage)?
                .into_iter()
                .filter(|column| theirs.contains(column))
                .map(|column| quoted(&column))
                .collect();
            let name = quoted(&table);
            connection.execute(&format!("DELETE FROM main.{name}"), []).map_err(storage)?;
            if !shared.is_empty() {
                let columns = shared.join(", ");
                connection
                    .execute(
                        &format!("INSERT INTO main.{name} ({columns}) SELECT {columns} FROM backup.{name}"),
                        [],
                    )
                    .map_err(storage)?;
            }
        }
        let broken = connection
            .prepare("PRAGMA main.foreign_key_check")
            .and_then(|mut statement| statement.exists([]))
            .map_err(storage)?;
        if broken {
            // Dropping the transaction rolls it back.
            return Err(BackupError::Broken);
        }
        transaction.commit().map_err(storage)
    }

    /// Keeps a copy of the database in the folder of [`copies_in`](Self::copies_in), named for
    /// when and why, then removes the oldest beyond [`keep`](Self::keep). `reason` is a short
    /// word of letters, digits and dashes, such as `daily` or `before-erase`.
    pub fn keep_copy(&self, reason: &str) -> Result<PathBuf, BackupError> {
        let folder = self.folder.clone().ok_or_else(|| storage("no folder for copies"))?;
        let reason: String =
            reason.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        let made = now();
        // Copies of the same second keep the order they were made in.
        let sequence = self
            .copies()
            .iter()
            .filter(|copy| copy.made == made)
            .map(|copy| copy.sequence + 1)
            .max()
            .unwrap_or(0);
        let target = folder.join(format!("copy-{made:012}-{sequence:04}-{reason}.db"));
        self.save(&target)?;
        self.prune();
        Ok(target)
    }

    /// The copies in the folder, newest first.
    pub fn copies(&self) -> Vec<KeptCopy> {
        let Some(folder) = &self.folder else { return Vec::new() };
        let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
        let mut found: Vec<KeptCopy> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_str()?.to_owned();
                let rest = name.strip_prefix("copy-")?.strip_suffix(".db")?;
                let (made, rest) = rest.split_once('-')?;
                let (sequence, reason) = rest.split_once('-')?;
                Some(KeptCopy {
                    path: entry.path(),
                    reason: reason.to_owned(),
                    made: made.parse().ok()?,
                    sequence: sequence.parse().ok()?,
                })
            })
            .collect();
        found.sort_by_key(|copy| std::cmp::Reverse((copy.made, copy.sequence)));
        found
    }

    fn prune(&self) {
        for old in self.copies().into_iter().skip(self.keep) {
            remove_database_files(&old.path);
        }
    }
}
