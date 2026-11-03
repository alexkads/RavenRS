//! Records in SQLite, as an [`ObjectSpace`].
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

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::rc::Rc;

use ravenrs_app::{
    AttachmentStore, Change, ChangeSet, Diff, EntityInfo, FieldInfo, FieldKind, ModelError, ModelStore,
    Object, ObjectSpace, Query, SpaceError, SpaceEvent, SpaceWatcher, Value, key_of, key_text, run_query,
};
use ravenrs_criteria::{DateTime, Dialect, Environment, SqlParam, to_sql};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params_from_iter};

type Hook = Rc<dyn Fn(&mut Object)>;

/// How an entity lands on a table.
#[derive(Clone)]
pub struct TableMap {
    table: String,
    columns: BTreeMap<String, String>,
    computed: BTreeMap<String, String>,
    before_save: Option<Hook>,
    after_load: Option<Hook>,
}

impl fmt::Debug for TableMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableMap").field("table", &self.table).finish_non_exhaustive()
    }
}

impl TableMap {
    /// On this table, with each field in a column of its name.
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            columns: BTreeMap::new(),
            computed: BTreeMap::new(),
            before_save: None,
            after_load: None,
        }
    }

    /// This field is in a column of another name.
    pub fn column(mut self, field: impl Into<String>, column: impl Into<String>) -> Self {
        self.columns.insert(field.into(), column.into());
        self
    }

    /// This field is not stored: the database works it out with this SQL expression over the
    /// table, which is trusted as written and never comes from people.
    pub fn computed(mut self, field: impl Into<String>, sql: impl Into<String>) -> Self {
        self.computed.insert(field.into(), sql.into());
        self
    }

    /// Runs on a record just before it is written, to fit what the table asks for.
    pub fn before_save(mut self, hook: impl Fn(&mut Object) + 'static) -> Self {
        self.before_save = Some(Rc::new(hook));
        self
    }

    /// Runs on a record just after it is read.
    pub fn after_load(mut self, hook: impl Fn(&mut Object) + 'static) -> Self {
        self.after_load = Some(Rc::new(hook));
        self
    }

    fn column_of<'a>(&'a self, field: &'a str) -> &'a str {
        self.columns.get(field).map_or(field, String::as_str)
    }
}

/// What to say when the database refuses something, where `{0}` is the field's caption.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SqlWords {
    /// A value that has to be unique is not.
    pub unique: String,
    /// Something else still points to the record.
    pub in_use: String,
    /// Any other rule of the table.
    pub rule: String,
    /// Anything else, where `{0}` is what SQLite said.
    pub failed: String,
}

impl Default for SqlWords {
    fn default() -> Self {
        Self {
            unique: "There is already a record with this {0}".into(),
            in_use: "This record is in use. Remove what depends on it first.".into(),
            rule: "The data did not pass the rules of the database.".into(),
            failed: "The database could not be reached: {0}".into(),
        }
    }
}

impl SqlWords {
    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            unique: "Já existe um registro com este valor em {0}.".into(),
            in_use: "Este registro está em uso. Remova antes o que depende dele.".into(),
            rule: "Os dados não passaram nas regras do banco.".into(),
            failed: "Não foi possível acessar o banco de dados: {0}".into(),
        }
    }
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn sql_type(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Integer | FieldKind::Money | FieldKind::Boolean => "INTEGER",
        FieldKind::Decimal(_) => "REAL",
        _ => "TEXT",
    }
}

/// Where migrations that ran are written down.
const MIGRATIONS: &str = "ravenrs_app_migrations";
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

struct Entity {
    info: EntityInfo,
    map: TableMap,
}

impl Entity {
    /// The fields that have a column of their own.
    fn stored(&self) -> impl Iterator<Item = &FieldInfo> {
        self.info.fields.iter().filter(|f| {
            f.calculated.is_none()
                && !matches!(f.kind, FieldKind::Collection(_))
                && !self.map.computed.contains_key(&f.name)
        })
    }

    /// A select with every field under its own name, so filters and order by field work.
    fn select(&self) -> String {
        let mut columns: Vec<String> = self
            .stored()
            .map(|f| format!("{} AS {}", quoted(self.map.column_of(&f.name)), quoted(&f.name)))
            .collect();
        columns.extend(self.map.computed.iter().map(|(field, sql)| format!("({sql}) AS {}", quoted(field))));
        format!(
            "SELECT {} FROM {} AS {}",
            columns.join(", "),
            quoted(&self.map.table),
            quoted(&self.map.table)
        )
    }

    fn read(&self, row: &rusqlite::Row<'_>, names: &[String]) -> rusqlite::Result<Object> {
        let mut values = BTreeMap::new();
        for (at, name) in names.iter().enumerate() {
            let kind = self.info.field_named(name).map(|f| &f.kind);
            let value = match (row.get_ref(at)?, kind) {
                (ValueRef::Null, _) => Value::Null,
                (ValueRef::Integer(n), Some(FieldKind::Boolean)) => Value::Bool(n != 0),
                (ValueRef::Integer(n), _) => Value::Number(n as f64),
                (ValueRef::Real(n), _) => Value::Number(n),
                (ValueRef::Text(bytes), kind) => {
                    let text = String::from_utf8_lossy(bytes).into_owned();
                    match kind {
                        Some(FieldKind::Date | FieldKind::DateTime) => {
                            DateTime::parse(&text).map_or(Value::Text(text), Value::Date)
                        }
                        _ => Value::Text(text),
                    }
                }
                (ValueRef::Blob(_), _) => Value::Null,
            };
            values.insert(name.clone(), value);
        }
        Ok(Object::from_values(&self.info.name, values))
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

struct Shared {
    connection: Rc<Connection>,
    entities: Vec<Entity>,
    words: SqlWords,
    next_keys: RefCell<HashMap<String, i64>>,
}

/// Records in SQLite, on a connection that can be shared with the rest of an application. Any
/// number of spaces share it.
#[derive(Clone)]
pub struct SqliteDatabase(Rc<Shared>);

impl fmt::Debug for SqliteDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteDatabase").field("entities", &self.0.entities.len()).finish()
    }
}

/// Puts a database together, one entity at a time.
pub struct SqliteBuilder {
    connection: Rc<Connection>,
    entities: Vec<Entity>,
    words: SqlWords,
}

impl fmt::Debug for SqliteBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteBuilder").field("entities", &self.entities.len()).finish()
    }
}

impl SqliteBuilder {
    /// What to say when the database refuses something.
    pub fn words(mut self, words: SqlWords) -> Self {
        self.words = words;
        self
    }

    fn checked(entity: &EntityInfo) -> Result<(), ModelError> {
        entity.check()
    }

    /// An entity on a table that exists, whose schema is not touched.
    pub fn mapped(mut self, entity: EntityInfo, map: TableMap) -> Result<Self, ModelError> {
        Self::checked(&entity)?;
        self.entities.push(Entity { info: entity, map });
        Ok(self)
    }

    /// An entity on a table that is created if it is missing.
    pub fn managed(self, entity: EntityInfo, map: TableMap) -> Result<Self, ModelError> {
        let mine = self.mapped(entity, map)?;
        let Some(added) = mine.entities.last() else { return Ok(mine) };
        let columns: Vec<String> = added
            .stored()
            .map(|field| {
                let column = quoted(added.map.column_of(&field.name));
                let key = if field.is_key { " PRIMARY KEY" } else { "" };
                format!("{column} {}{key}", sql_type(&field.kind))
            })
            .collect();
        let sql = format!("CREATE TABLE IF NOT EXISTS {} ({})", quoted(&added.map.table), columns.join(", "));
        mine.connection.execute(&sql, []).map_err(|e| ModelError(e.to_string()))?;
        // A table that was already there gets the fields added since.
        let wrong = |e: rusqlite::Error| ModelError(e.to_string());
        let has = columns_of(&mine.connection, &added.map.table).map_err(wrong)?;
        for field in added.stored() {
            let column = added.map.column_of(&field.name);
            if !has.iter().any(|found| found.name == column) {
                let add = format!(
                    "ALTER TABLE {} ADD COLUMN {} {}",
                    quoted(&added.map.table),
                    quoted(column),
                    sql_type(&field.kind)
                );
                mine.connection.execute(&add, []).map_err(wrong)?;
            }
        }
        Ok(mine)
    }

    /// Runs this once in the life of the database, for what adding a column cannot do, such as
    /// renaming one or moving data. A migration that fails leaves everything as it was.
    pub fn migration(self, id: &str, sql: &str) -> Result<Self, ModelError> {
        let wrong = |e: rusqlite::Error| ModelError(format!("the migration {id}: {e}"));
        let connection = &self.connection;
        connection
            .execute_batch(&format!("CREATE TABLE IF NOT EXISTS {MIGRATIONS} (id TEXT PRIMARY KEY)"))
            .map_err(wrong)?;
        let done = connection
            .prepare(&format!("SELECT 1 FROM {MIGRATIONS} WHERE id = ?1"))
            .and_then(|mut statement| statement.exists([id]))
            .map_err(wrong)?;
        if !done {
            let transaction = connection.unchecked_transaction().map_err(wrong)?;
            connection.execute_batch(sql).map_err(wrong)?;
            connection.execute(&format!("INSERT INTO {MIGRATIONS} (id) VALUES (?1)"), [id]).map_err(wrong)?;
            transaction.commit().map_err(wrong)?;
        }
        Ok(self)
    }

    /// Ready to read and write.
    pub fn build(self) -> SqliteDatabase {
        SqliteDatabase(Rc::new(Shared {
            connection: self.connection,
            entities: self.entities,
            words: self.words,
            next_keys: RefCell::default(),
        }))
    }
}

impl SqliteDatabase {
    /// Starts one on this connection. See [`SqliteBuilder`].
    pub fn builder(connection: Rc<Connection>) -> SqliteBuilder {
        SqliteBuilder { connection, entities: Vec::new(), words: SqlWords::default() }
    }

    /// A new unit of work.
    pub fn space(&self) -> SqliteObjectSpace {
        SqliteObjectSpace {
            shared: self.0.clone(),
            environment: ravenrs_app::environment(),
            changes: ChangeSet::default(),
            watchers: Vec::new(),
        }
    }

    /// The entities it knows.
    pub fn entities(&self) -> Vec<EntityInfo> {
        self.0.entities.iter().map(|entity| entity.info.clone()).collect()
    }
}

/// A unit of work over a [`SqliteDatabase`]. A commit is one transaction.
pub struct SqliteObjectSpace {
    shared: Rc<Shared>,
    environment: Environment,
    changes: ChangeSet,
    watchers: Vec<SpaceWatcher>,
}

impl fmt::Debug for SqliteObjectSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteObjectSpace").finish_non_exhaustive()
    }
}

impl SqliteObjectSpace {
    fn entity(&self, name: &str) -> Result<&Entity, SpaceError> {
        self.shared
            .entities
            .iter()
            .find(|entity| entity.info.name == name)
            .ok_or_else(|| SpaceError::UnknownEntity(name.to_owned()))
    }

    /// Says what the database refused, in words for people.
    fn refused(&self, entity: &Entity, error: &rusqlite::Error) -> SpaceError {
        let words = &self.shared.words;
        let text = error.to_string();
        if error.sqlite_error_code() != Some(ErrorCode::ConstraintViolation) {
            return SpaceError::Storage(words.failed.replace("{0}", &text));
        }
        if text.contains("FOREIGN KEY") {
            return SpaceError::Refused(words.in_use.clone());
        }
        if text.contains("UNIQUE") {
            // "UNIQUE constraint failed: accounts.name", possibly of several columns.
            let caption = text
                .rsplit(": ")
                .next()
                .and_then(|columns| columns.split(", ").next())
                .and_then(|column| column.rsplit('.').next())
                .and_then(|column| {
                    entity.info.fields.iter().find(|f| entity.map.column_of(&f.name) == column)
                })
                .map_or_else(String::new, |field| field.caption.clone());
            return SpaceError::Refused(words.unique.replace("{0}", &caption));
        }
        SpaceError::Refused(words.rule.clone())
    }

    fn select(
        &self,
        entity: &Entity,
        tail: &str,
        parameters: Vec<SqlValue>,
    ) -> Result<Vec<Object>, SpaceError> {
        self.select_columns(entity, "*", tail, parameters)
    }

    fn select_columns(
        &self,
        entity: &Entity,
        columns: &str,
        tail: &str,
        parameters: Vec<SqlValue>,
    ) -> Result<Vec<Object>, SpaceError> {
        let sql = format!("SELECT {columns} FROM ({}){tail}", entity.select());
        let storage = |e: rusqlite::Error| self.refused(entity, &e);
        let mut statement = self.shared.connection.prepare_cached(&sql).map_err(storage)?;
        let names: Vec<String> = statement.column_names().map(str::to_owned).collect();
        let rows = statement
            .query_map(params_from_iter(parameters), |row| entity.read(row, &names))
            .map_err(storage)?;
        let mut found = Vec::new();
        for row in rows {
            let mut object = row.map_err(storage)?;
            entity.info.compute(&mut object, &self.environment);
            if let Some(hook) = &entity.map.after_load {
                hook(&mut object);
            }
            found.push(object);
        }
        Ok(found)
    }

    fn exists(&self, entity: &Entity, key: &str) -> bool {
        let Some(field) = entity.info.key_field() else { return false };
        let sql = format!(
            "SELECT 1 FROM {} WHERE {} = ?1",
            quoted(&entity.map.table),
            quoted(entity.map.column_of(&field.name))
        );
        let value = match field.kind {
            FieldKind::Integer | FieldKind::Decimal(_) => key.parse().map_or(SqlValue::Null, SqlValue::Real),
            _ => SqlValue::Text(key.to_owned()),
        };
        self.shared
            .connection
            .prepare_cached(&sql)
            .and_then(|mut statement| statement.exists([value]))
            .unwrap_or(false)
    }

    fn next_key(&self, entity: &Entity, field: &FieldInfo) -> Result<i64, SpaceError> {
        let sql = format!(
            "SELECT COALESCE(MAX({}), 0) + 1 FROM {}",
            quoted(entity.map.column_of(&field.name)),
            quoted(&entity.map.table)
        );
        let stored: i64 = self
            .shared
            .connection
            .query_row(&sql, [], |row| row.get(0))
            .map_err(|e| self.refused(entity, &e))?;
        let mut keys = self.shared.next_keys.borrow_mut();
        let next = keys.get(&entity.info.name).copied().unwrap_or(0).max(stored);
        keys.insert(entity.info.name.clone(), next + 1);
        Ok(next)
    }

    fn write(&self, entity: &Entity, change: &Change) -> Result<(), SpaceError> {
        let failed = |e: rusqlite::Error| self.refused(entity, &e);
        let table = quoted(&entity.map.table);
        let key_field =
            entity.info.key_field().ok_or_else(|| SpaceError::MissingKey(entity.info.name.clone()))?;
        let key_column = quoted(entity.map.column_of(&key_field.name));
        let connection = &self.shared.connection;
        let prepared = |object: &Object| {
            let mut object = object.clone();
            if let Some(hook) = &entity.map.before_save {
                hook(&mut object);
            }
            object
        };
        match change {
            Change::New(object) => {
                let object = prepared(object);
                let stored: Vec<&FieldInfo> = entity.stored().collect();
                let columns: Vec<String> =
                    stored.iter().map(|f| quoted(entity.map.column_of(&f.name))).collect();
                let slots: Vec<String> = (1..=stored.len()).map(|at| format!("?{at}")).collect();
                let sql =
                    format!("INSERT INTO {table} ({}) VALUES ({})", columns.join(", "), slots.join(", "));
                let values = stored.iter().map(|f| bound(&object.get(&f.name)));
                connection.execute(&sql, params_from_iter(values)).map_err(failed)?;
            }
            Change::Updated(object) => {
                let object = prepared(object);
                let stored: Vec<&FieldInfo> = entity.stored().filter(|f| !f.is_key).collect();
                let sets: Vec<String> = stored
                    .iter()
                    .enumerate()
                    .map(|(at, f)| format!("{} = ?{}", quoted(entity.map.column_of(&f.name)), at + 1))
                    .collect();
                let sql = format!(
                    "UPDATE {table} SET {} WHERE {key_column} = ?{}",
                    sets.join(", "),
                    stored.len() + 1
                );
                let mut values: Vec<SqlValue> = stored.iter().map(|f| bound(&object.get(&f.name))).collect();
                values.push(bound(&object.get(&key_field.name)));
                let changed = connection.execute(&sql, params_from_iter(values)).map_err(failed)?;
                if changed == 0 {
                    return Err(SpaceError::Conflict(format!("a record of {} is gone", entity.info.name)));
                }
            }
            Change::Deleted { key, .. } => {
                let sql = format!("DELETE FROM {table} WHERE {key_column} = ?1");
                connection.execute(&sql, [bound(key)]).map_err(failed)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn tell(&mut self, event: SpaceEvent<'_>) -> Result<(), SpaceError> {
        for watcher in &mut self.watchers {
            watcher(event).map_err(SpaceError::Refused)?;
        }
        Ok(())
    }
}

impl ObjectSpace for SqliteObjectSpace {
    fn entities(&self) -> Vec<EntityInfo> {
        self.shared.entities.iter().map(|entity| entity.info.clone()).collect()
    }

    fn query(&self, name: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        let entity = self.entity(name)?;
        // What waits for a commit, or what SQL cannot say, goes through memory.
        let translated = query.filter.as_ref().map(|filter| to_sql(filter, Dialect::Sqlite));
        let in_database = !self.changes.touches(name)
            && !entity.info.fields.iter().any(|f| f.calculated.is_some())
            && !matches!(translated, Some(Err(_)));
        if !in_database {
            let rows = self.select(entity, "", Vec::new())?;
            let mut by_key: BTreeMap<String, Object> =
                rows.into_iter().filter_map(|o| key_of(&entity.info, &o).map(|k| (k, o))).collect();
            self.changes.overlay(&entity.info, &mut by_key);
            let mut objects: Vec<Object> = by_key.into_values().collect();
            for object in &mut objects {
                entity.info.compute(object, &self.environment);
            }
            return run_query(&entity.info, objects, query, &self.environment);
        }
        ravenrs_app::check_query(&entity.info, query)?;
        let mut tail = String::new();
        let mut parameters = Vec::new();
        if let Some(Ok(sql)) = translated {
            tail.push_str(" WHERE ");
            tail.push_str(&sql.text);
            for parameter in &sql.params {
                match parameter {
                    SqlParam::Value(value) => parameters.push(bound(value)),
                    _ => {
                        return Err(SpaceError::BadQuery(
                            "a filter asks for a value it was not given".into(),
                        ));
                    }
                }
            }
        }
        let key = entity.info.key_field().map(|f| f.name.clone()).unwrap_or_default();
        let mut order: Vec<String> = query
            .sort
            .iter()
            .map(|(field, ascending)| {
                format!("{} {}", quoted(field), if *ascending { "ASC" } else { "DESC" })
            })
            .collect();
        order.push(quoted(&key));
        tail.push_str(&format!(" ORDER BY {}", order.join(", ")));
        if let Some(limit) = query.limit {
            tail.push_str(&format!(" LIMIT {limit} OFFSET {}", query.offset));
        } else if query.offset > 0 {
            tail.push_str(&format!(" LIMIT -1 OFFSET {}", query.offset));
        }
        let columns = query.only.as_ref().map_or_else(
            || "*".to_owned(),
            |only| {
                let mut names: Vec<&str> = only.iter().map(String::as_str).collect();
                if !names.contains(&key.as_str()) {
                    names.push(&key);
                }
                names.iter().map(|name| quoted(name)).collect::<Vec<_>>().join(", ")
            },
        );
        self.select_columns(entity, &columns, &tail, parameters)
    }

    fn find(&self, name: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        let entity = self.entity(name)?;
        let Some(text) = key_text(key) else { return Ok(None) };
        let field = entity.info.key_field().ok_or_else(|| SpaceError::MissingKey(name.to_owned()))?;
        if self.changes.touches(name) {
            let mut by_key = BTreeMap::new();
            let tail = format!(" WHERE {} = ?1", quoted(&field.name));
            for object in self.select(entity, &tail, vec![bound(key)])? {
                by_key.insert(text.clone(), object);
            }
            self.changes.overlay(&entity.info, &mut by_key);
            return Ok(by_key.remove(&text).map(|mut object| {
                entity.info.compute(&mut object, &self.environment);
                object
            }));
        }
        let tail = format!(" WHERE {} = ?1", quoted(&field.name));
        Ok(self.select(entity, &tail, vec![bound(key)])?.into_iter().next())
    }

    fn new_object(&mut self, name: &str) -> Result<Object, SpaceError> {
        let entity = self.entity(name)?;
        let mut object = entity.info.new_object();
        if let Some(field) = entity.info.key_field()
            && matches!(field.kind, FieldKind::Integer)
            && matches!(object.get(&field.name), Value::Null)
        {
            let next = self.next_key(entity, field)?;
            object.set(&field.name, Value::Number(next as f64));
        }
        Ok(object)
    }

    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        let entity = self.entity(object.entity())?;
        let info = entity.info.clone();
        let exists = key_of(&info, &object).is_some_and(|key| self.exists(entity, &key));
        self.changes.save(&info, object, exists)
    }

    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        let entity = self.entity(object.entity())?;
        let info = entity.info.clone();
        let exists = key_of(&info, &object).is_some_and(|key| self.exists(entity, &key));
        self.changes.insert(&info, object, exists)
    }

    fn delete(&mut self, name: &str, key: &Value) -> Result<(), SpaceError> {
        let entity = self.entity(name)?;
        let info = entity.info.clone();
        let exists = key_text(key).is_some_and(|text| self.exists(entity, &text));
        self.changes.delete(&info, key, exists);
        Ok(())
    }

    fn changes(&self) -> Vec<Change> {
        self.changes.list().to_vec()
    }

    fn commit(&mut self) -> Result<(), SpaceError> {
        let changes = self.changes.list().to_vec();
        self.tell(SpaceEvent::Committing(&changes))?;
        let shared = self.shared.clone();
        let transaction =
            shared.connection.unchecked_transaction().map_err(|e| SpaceError::Storage(e.to_string()))?;
        for change in &changes {
            let name = match change {
                Change::New(object) | Change::Updated(object) => object.entity(),
                Change::Deleted { entity, .. } => entity.as_str(),
                _ => continue,
            };
            let entity = self.entity(name)?;
            if let Change::New(new) | Change::Updated(new) = change
                && !entity.info.state_machines.is_empty()
            {
                let field = entity.info.key_field().ok_or_else(|| SpaceError::MissingKey(name.to_owned()))?;
                let tail = format!(" WHERE {} = ?1", quoted(entity.map.column_of(&field.name)));
                let stored = self.select(entity, &tail, vec![bound(&new.get(&field.name))])?;
                ravenrs_app::check_transitions(&entity.info, stored.first(), new)?;
            }
            self.write(entity, change)?;
        }
        transaction.commit().map_err(|e| SpaceError::Storage(e.to_string()))?;
        self.changes.clear();
        let _ = self.tell(SpaceEvent::Committed(&changes));
        Ok(())
    }

    fn rollback(&mut self) {
        self.changes.clear();
        let _ = self.tell(SpaceEvent::RolledBack);
    }

    fn on_event(&mut self, watcher: SpaceWatcher) {
        self.watchers.push(watcher);
    }
}
