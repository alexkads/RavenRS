//! Records in SQLite, as an [`ObjectSpace`](ravenrs_app::ObjectSpace).
//!
//! ```
//! use std::rc::Rc;
//!
//! use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, Value};
//! use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
//!
//! let account = EntityInfo::new("Account")
//!     .field(FieldInfo::new("id", FieldKind::Integer).key())
//!     .field(FieldInfo::new("name", FieldKind::Text));
//! let connection = Rc::new(rusqlite::Connection::open_in_memory()?);
//! let database = SqliteDatabase::builder(connection).managed(account, TableMap::new("accounts"))?.build();
//!
//! let mut space = database.space();
//! let mut object = space.new_object("Account")?;
//! object.set("name", "Corrente");
//! space.save(object)?;
//! space.commit()?;
//! let found = database.space().query("Account", &Query::all())?;
//! assert_eq!(found[0].get("name"), Value::Text("Corrente".into()));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::borrow::Cow;
use std::rc::Rc;

use ravenrs_app::{AttachmentStore, Diff, EntityInfo, FieldInfo, FieldKind, ModelError, ModelStore, Value};
use ravenrs_app_sql::{
    MIGRATIONS, SqlBackend, SqlBuilder, SqlCell, SqlDatabase, SqlError, SqlObjectSpace, SqlRows,
};
pub use ravenrs_app_sql::{SqlWords, TableMap};
use ravenrs_criteria::Dialect;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params_from_iter};

mod backup;

pub use backup::{BackupError, BackupInfo, BackupStamp, Backups, COPIES_KEPT, KeptCopy, SqliteBackups};

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Where settings of the model are kept.
const SETTINGS: &str = "ravenrs_app_settings";
/// Where the content of attached files is kept.
const FILES: &str = "ravenrs_app_files";

/// A column of a table, as SQLite describes it.
struct Column {
    name: String,
    kind: String,
    required: bool,
    key: bool,
}

fn columns_of(connection: &Connection, table: &str) -> rusqlite::Result<Vec<Column>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({})", quoted(table)))?;
    let rows = statement.query_map([], |row| {
        let default: Option<String> = row.get(4)?;
        let not_null: bool = row.get(3)?;
        Ok(Column {
            name: row.get(1)?,
            kind: row.get::<_, String>(2)?.to_uppercase(),
            required: not_null && default.is_none(),
            key: row.get::<_, i64>(5)? > 0,
        })
    })?;
    rows.collect()
}

/// `order_items` as `OrderItems`.
fn entity_name(table: &str) -> String {
    table
        .split('_')
        .map(|part| {
            let mut letters = part.chars();
            letters.next().map_or_else(String::new, |first| first.to_uppercase().chain(letters).collect())
        })
        .collect()
}

/// What a column holds, by the type SQLite was told; nothing for what a field cannot show.
fn kind_of(declared: &str) -> Option<FieldKind> {
    if declared.contains("BLOB") {
        None
    } else if declared.contains("INT") {
        Some(FieldKind::Integer)
    } else if ["REAL", "FLOA", "DOUB", "NUMERIC", "DECIMAL"].iter().any(|name| declared.contains(name)) {
        Some(FieldKind::Decimal(2))
    } else if declared.contains("DATETIME") || declared.contains("TIMESTAMP") {
        Some(FieldKind::DateTime)
    } else if declared.contains("DATE") {
        Some(FieldKind::Date)
    } else if declared.contains("BOOL") {
        Some(FieldKind::Boolean)
    } else {
        Some(FieldKind::Text)
    }
}

/// Entities from the tables a database already has, for an application over a database that came
/// first: a field per column, a reference per foreign key, and a required field per column that
/// asks for it. A table with no single key is left out, since a record could not be found. What
/// comes out is a start: captions and rules are for the application to give.
pub fn entities_from_schema(connection: &Connection) -> Result<Vec<(EntityInfo, TableMap)>, ModelError> {
    let wrong = |e: rusqlite::Error| ModelError(e.to_string());
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(wrong)?;
    let tables: Vec<String> =
        statement.query_map([], |row| row.get(0)).and_then(Iterator::collect).map_err(wrong)?;
    let mut found = Vec::new();
    for table in tables.iter().filter(|table| ![MIGRATIONS, SETTINGS, FILES].contains(&table.as_str())) {
        let columns = columns_of(connection, table).map_err(wrong)?;
        if columns.iter().filter(|column| column.key).count() != 1 {
            continue;
        }
        let mut statement =
            connection.prepare(&format!("PRAGMA foreign_key_list({})", quoted(table))).map_err(wrong)?;
        let foreign: Vec<(String, String)> = statement
            .query_map([], |row| Ok((row.get(3)?, row.get(2)?)))
            .and_then(Iterator::collect)
            .map_err(wrong)?;
        let names_a_record = columns
            .iter()
            .find(|c| ["name", "title", "description"].contains(&c.name.as_str()))
            .map(|c| c.name.clone());
        let mut entity = EntityInfo::new(entity_name(table));
        for column in &columns {
            let target = foreign.iter().find(|(from, _)| *from == column.name).map(|(_, to)| to);
            let kind = match target {
                Some(to) => FieldKind::Reference(entity_name(to)),
                None => match kind_of(&column.kind) {
                    Some(kind) => kind,
                    None => continue,
                },
            };
            let number = matches!(kind, FieldKind::Integer);
            let field = FieldInfo::new(&column.name, kind);
            let field = match (column.key, number) {
                (true, true) => field.key(),
                (true, false) => field.typed_key(),
                _ => field,
            };
            let field = if names_a_record.as_deref() == Some(column.name.as_str()) {
                field.default_property()
            } else {
                field
            };
            entity = entity.field(field);
            if column.required && !column.key {
                entity = entity.rule(ravenrs_app::Rule::required(&column.name));
            }
        }
        found.push((entity, TableMap::new(table)));
    }
    Ok(found)
}

/// Keeps a layer of the model in the database, under a name, as for one person's settings.
#[derive(Clone, Debug)]
pub struct SqliteModelStore {
    connection: Rc<Connection>,
    key: String,
}

impl SqliteModelStore {
    /// A store under this name, creating where it keeps things if needed.
    pub fn new(connection: Rc<Connection>, key: impl Into<String>) -> Result<Self, ModelError> {
        connection
            .execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS {SETTINGS} (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
            ))
            .map_err(|e| ModelError(e.to_string()))?;
        Ok(Self { connection, key: key.into() })
    }
}

impl ModelStore for SqliteModelStore {
    fn load(&self) -> Result<Option<Diff>, ModelError> {
        let sql = format!("SELECT value FROM {SETTINGS} WHERE key = ?1");
        let text: Option<String> = self
            .connection
            .query_row(&sql, [&self.key], |row| row.get(0))
            .optional()
            .map_err(|e| ModelError(e.to_string()))?;
        text.map(|text| Diff::parse(&text)).transpose()
    }

    fn save(&self, difference: &Diff) -> Result<(), ModelError> {
        let sql = format!("INSERT OR REPLACE INTO {SETTINGS} (key, value) VALUES (?1, ?2)");
        self.connection
            .execute(&sql, [self.key.as_str(), difference.to_json().as_str()])
            .map(|_| ())
            .map_err(|e| ModelError(e.to_string()))
    }
}

/// Keeps the content of attached files in the database, in a table apart from the records.
#[derive(Clone, Debug)]
pub struct SqliteAttachmentStore {
    connection: Rc<Connection>,
}

impl SqliteAttachmentStore {
    /// A store on this connection, creating where it keeps things if needed.
    pub fn new(connection: Rc<Connection>) -> Result<Self, ModelError> {
        connection
            .execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS {FILES} (id INTEGER PRIMARY KEY, name TEXT NOT NULL, content BLOB NOT NULL)"
            ))
            .map_err(|e| ModelError(e.to_string()))?;
        Ok(Self { connection })
    }
}

impl AttachmentStore for SqliteAttachmentStore {
    fn put(&self, name: &str, bytes: &[u8]) -> Result<String, String> {
        let sql = format!("INSERT INTO {FILES} (name, content) VALUES (?1, ?2)");
        self.connection.execute(&sql, rusqlite::params![name, bytes]).map_err(|e| e.to_string())?;
        Ok(self.connection.last_insert_rowid().to_string())
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let sql = format!("SELECT content FROM {FILES} WHERE id = ?1");
        self.connection.query_row(&sql, [key], |row| row.get(0)).optional().map_err(|e| e.to_string())
    }

    fn remove(&self, key: &str) -> Result<(), String> {
        let sql = format!("DELETE FROM {FILES} WHERE id = ?1");
        self.connection.execute(&sql, [key]).map(|_| ()).map_err(|e| e.to_string())
    }
}

fn bound(value: &Value) -> SqlValue {
    match value {
        Value::Bool(on) => SqlValue::Integer(i64::from(*on)),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => SqlValue::Integer(*n as i64),
        Value::Number(n) => SqlValue::Real(*n),
        Value::Text(text) => SqlValue::Text(text.clone()),
        Value::Date(moment) => SqlValue::Text(moment.to_string()),
        _ => SqlValue::Null,
    }
}

fn sql_error(error: &rusqlite::Error) -> SqlError {
    let text = error.to_string();
    if matches!(error.sqlite_error_code(), Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)) {
        return SqlError::Busy(text);
    }
    if error.sqlite_error_code() != Some(ErrorCode::ConstraintViolation) {
        return SqlError::Other(text);
    }
    if text.contains("FOREIGN KEY") {
        return SqlError::ForeignKey;
    }
    if text.contains("UNIQUE") {
        // "UNIQUE constraint failed: accounts.name", possibly of several columns.
        let column = text
            .rsplit(": ")
            .next()
            .and_then(|columns| columns.split(", ").next())
            .and_then(|column| column.rsplit('.').next())
            .map(str::to_owned);
        return SqlError::Unique(column);
    }
    SqlError::Constraint
}

/// A connection to SQLite, which can be shared with the rest of an application.
#[derive(Clone, Debug)]
pub struct SqliteBackend(Rc<Connection>);

impl SqliteBackend {
    /// The connection underneath.
    pub fn connection(&self) -> &Connection {
        &self.0
    }
}

impl From<Rc<Connection>> for SqliteBackend {
    fn from(connection: Rc<Connection>) -> Self {
        Self(connection)
    }
}

impl SqlBackend for SqliteBackend {
    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }

    fn column_type(&self, kind: &FieldKind) -> Cow<'static, str> {
        match kind {
            FieldKind::Integer | FieldKind::Money | FieldKind::Boolean => "INTEGER".into(),
            // The same binary number `Value::Number` is, so nothing changes on the way; the value
            // is rounded to its places before it is written.
            FieldKind::Decimal(_) => "REAL".into(),
            _ => "TEXT".into(),
        }
    }

    fn decimal_upgrade(&self, table: &str, column: &str, places: u8) -> Option<String> {
        let quoted = |name: &str| format!("\"{}\"", name.replace('"', "\"\""));
        let (table, column) = (quoted(table), quoted(column));
        Some(format!(
            "UPDATE {table} SET {column} = round({column}, {places}) \
             WHERE typeof({column}) = 'real' AND {column} <> round({column}, {places})"
        ))
    }

    fn slot(&self, at: usize) -> String {
        format!("?{at}")
    }

    fn offset_only(&self, offset: usize) -> String {
        format!(" LIMIT -1 OFFSET {offset}")
    }

    fn columns_of(&self, table: &str) -> Result<Vec<String>, SqlError> {
        let columns = columns_of(&self.0, table).map_err(|e| sql_error(&e))?;
        Ok(columns.into_iter().map(|column| column.name).collect())
    }

    fn execute(&self, sql: &str, values: &[Value]) -> Result<u64, SqlError> {
        let mut statement = self.0.prepare_cached(sql).map_err(|e| sql_error(&e))?;
        let changed =
            statement.execute(params_from_iter(values.iter().map(bound))).map_err(|e| sql_error(&e))?;
        Ok(changed as u64)
    }

    fn batch(&self, sql: &str) -> Result<(), SqlError> {
        self.0.execute_batch(sql).map_err(|e| sql_error(&e))
    }

    fn query(&self, sql: &str, values: &[Value]) -> Result<SqlRows, SqlError> {
        let mut statement = self.0.prepare_cached(sql).map_err(|e| sql_error(&e))?;
        let names: Vec<String> = statement.column_names().map(str::to_owned).collect();
        let width = names.len();
        let rows = statement
            .query_map(params_from_iter(values.iter().map(bound)), |row| {
                (0..width)
                    .map(|at| {
                        Ok(match row.get_ref(at)? {
                            ValueRef::Null | ValueRef::Blob(_) => SqlCell::Null,
                            ValueRef::Integer(n) => SqlCell::Integer(n),
                            ValueRef::Real(n) => SqlCell::Real(n),
                            ValueRef::Text(bytes) => {
                                SqlCell::Text(String::from_utf8_lossy(bytes).into_owned())
                            }
                        })
                    })
                    .collect::<rusqlite::Result<Vec<SqlCell>>>()
            })
            .map_err(|e| sql_error(&e))?;
        let rows = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| sql_error(&e))?;
        Ok(SqlRows::new(names, rows))
    }
}

/// Records in SQLite, on a connection that can be shared with the rest of an application. Any
/// number of spaces share it.
pub type SqliteDatabase = SqlDatabase<SqliteBackend>;
/// Puts a database together, one entity at a time.
pub type SqliteBuilder = SqlBuilder<SqliteBackend>;
/// A unit of work over a [`SqliteDatabase`]. A commit is one transaction.
pub type SqliteObjectSpace = SqlObjectSpace<SqliteBackend>;
