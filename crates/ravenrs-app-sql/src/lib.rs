//! Records in a SQL database, as an [`ObjectSpace`], whatever the database: what is the same for all
//! of them is here, and a [`SqlBackend`] says the rest. `ravenrs-app-sqlite` and
//! `ravenrs-app-postgres` are the ones that come with RavenRS.

use ravenrs_app_i18n::{Language, Texts};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::rc::Rc;

use ravenrs_app::{
    Change, ChangeSet, EntityInfo, FieldInfo, FieldKind, ModelError, Object, ObjectSpace, Query, SpaceError,
    SpaceEvent, SpaceWatcher, Value, key_of, key_text, run_query,
};
use ravenrs_criteria::{DateTime, Dialect, Environment, SqlParam, to_sql};

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
    /// Anything else, where `{0}` is what the database said.
    pub failed: String,
}

impl Default for SqlWords {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

impl SqlWords {
    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            unique: texts.get("sqlite.unique"),
            in_use: texts.get("sqlite.in_use"),
            rule: texts.get("sqlite.rule"),
            failed: texts.get("sqlite.failed"),
        }
    }

    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }
}

/// What a database said went wrong.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum SqlError {
    /// A value that has to be unique is not, in this column if the database says which.
    Unique(Option<String>),
    /// Something else still points to the record.
    ForeignKey,
    /// Any other rule of the table.
    Constraint,
    /// Anything else, in its own words.
    Other(String),
    /// The database is held by another connection right now, which passes: trying later may work.
    Busy(String),
}

impl SqlError {
    /// Anything else, in its own words.
    pub fn other(error: impl fmt::Display) -> Self {
        Self::Other(error.to_string())
    }
}

impl fmt::Display for SqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unique(column) => write!(f, "not unique: {}", column.as_deref().unwrap_or("")),
            Self::ForeignKey => f.write_str("something else points to this record"),
            Self::Constraint => f.write_str("a rule of the table refused it"),
            Self::Other(why) | Self::Busy(why) => f.write_str(why),
        }
    }
}

/// What a column holds in a row that was read.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SqlCell {
    /// Nothing.
    Null,
    /// A whole number.
    Integer(i64),
    /// Any other number.
    Real(f64),
    /// Yes or no.
    Bool(bool),
    /// Text.
    Text(String),
    /// A date and time.
    Date(DateTime),
}

/// Rows that were read, with the names of their columns.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct SqlRows {
    /// The names of the columns.
    pub names: Vec<String>,
    /// The rows.
    pub rows: Vec<Vec<SqlCell>>,
}

impl SqlRows {
    /// These rows.
    pub fn new(names: Vec<String>, rows: Vec<Vec<SqlCell>>) -> Self {
        Self { names, rows }
    }
}

/// What differs from one database to another.
pub trait SqlBackend {
    /// How expressions are translated for it.
    fn dialect(&self) -> Dialect;
    /// A column for this kind of field.
    fn column_type(&self, kind: &FieldKind) -> &'static str;
    /// The place of a value in a statement, from 1.
    fn slot(&self, at: usize) -> String;
    /// How to skip rows with no limit on how many come after.
    fn offset_only(&self, offset: usize) -> String;
    /// The columns a table has.
    fn columns_of(&self, table: &str) -> Result<Vec<String>, SqlError>;
    /// Runs a statement, saying how many rows it changed.
    fn execute(&self, sql: &str, values: &[Value]) -> Result<u64, SqlError>;
    /// Runs statements with no values, for the schema and for transactions.
    fn batch(&self, sql: &str) -> Result<(), SqlError>;
    /// Reads rows.
    fn query(&self, sql: &str, values: &[Value]) -> Result<SqlRows, SqlError>;
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Where migrations that ran are written down.
pub const MIGRATIONS: &str = "ravenrs_app_migrations";

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

    fn read(&self, names: &[String], row: Vec<SqlCell>) -> Object {
        let mut values = BTreeMap::new();
        for (name, cell) in names.iter().zip(row) {
            let kind = self.info.field_named(name).map(|f| &f.kind);
            let value = match (cell, kind) {
                (SqlCell::Null, _) => Value::Null,
                (SqlCell::Integer(n), Some(FieldKind::Boolean)) => Value::Bool(n != 0),
                (SqlCell::Integer(n), _) => Value::Number(n as f64),
                (SqlCell::Real(n), _) => Value::Number(n),
                (SqlCell::Bool(on), _) => Value::Bool(on),
                (SqlCell::Date(moment), _) => Value::Date(moment),
                (SqlCell::Text(text), Some(FieldKind::Date | FieldKind::DateTime)) => {
                    DateTime::parse(&text).map_or(Value::Text(text), Value::Date)
                }
                (SqlCell::Text(text), _) => Value::Text(text),
            };
            values.insert(name.clone(), value);
        }
        Object::from_values(&self.info.name, values)
    }
}

struct Shared<B> {
    backend: B,
    entities: Vec<Entity>,
    words: SqlWords,
    next_keys: RefCell<HashMap<String, i64>>,
}

/// Records in a database. Any number of spaces share it.
pub struct SqlDatabase<B>(Rc<Shared<B>>);

impl<B> Clone for SqlDatabase<B> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<B> fmt::Debug for SqlDatabase<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlDatabase").field("entities", &self.0.entities.len()).finish()
    }
}

/// Puts a database together, one entity at a time.
pub struct SqlBuilder<B> {
    backend: B,
    entities: Vec<Entity>,
    words: SqlWords,
}

impl<B> fmt::Debug for SqlBuilder<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlBuilder").field("entities", &self.entities.len()).finish()
    }
}

impl<B: SqlBackend> SqlBuilder<B> {
    /// What to say when the database refuses something.
    pub fn words(mut self, words: SqlWords) -> Self {
        self.words = words;
        self
    }

    /// An entity on a table that exists, whose schema is not touched.
    pub fn mapped(mut self, entity: EntityInfo, map: TableMap) -> Result<Self, ModelError> {
        entity.check()?;
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
                format!("{column} {}{key}", mine.backend.column_type(&field.kind))
            })
            .collect();
        let wrong = |e: SqlError| ModelError(e.to_string());
        let sql = format!("CREATE TABLE IF NOT EXISTS {} ({})", quoted(&added.map.table), columns.join(", "));
        mine.backend.batch(&sql).map_err(wrong)?;
        // A table that was already there gets the fields added since.
        let has = mine.backend.columns_of(&added.map.table).map_err(wrong)?;
        for field in added.stored() {
            let column = added.map.column_of(&field.name);
            if !has.iter().any(|found| found == column) {
                let add = format!(
                    "ALTER TABLE {} ADD COLUMN {} {}",
                    quoted(&added.map.table),
                    quoted(column),
                    mine.backend.column_type(&field.kind)
                );
                mine.backend.batch(&add).map_err(wrong)?;
            }
        }
        Ok(mine)
    }

    /// Runs this once in the life of the database, for what adding a column cannot do, such as
    /// renaming one or moving data. A migration that fails leaves everything as it was.
    pub fn migration(self, id: &str, sql: &str) -> Result<Self, ModelError> {
        let wrong = |e: SqlError| ModelError(format!("the migration {id}: {e}"));
        let backend = &self.backend;
        backend
            .batch(&format!("CREATE TABLE IF NOT EXISTS {MIGRATIONS} (id TEXT PRIMARY KEY)"))
            .map_err(wrong)?;
        let asked = format!("SELECT 1 FROM {MIGRATIONS} WHERE id = {}", backend.slot(1));
        let done = !backend.query(&asked, &[Value::Text(id.to_owned())]).map_err(wrong)?.rows.is_empty();
        if !done {
            backend.batch("BEGIN").map_err(wrong)?;
            let insert = format!("INSERT INTO {MIGRATIONS} (id) VALUES ({})", backend.slot(1));
            let ran = backend
                .batch(sql)
                .and_then(|()| backend.execute(&insert, &[Value::Text(id.to_owned())]).map(|_| ()))
                .and_then(|()| backend.batch("COMMIT"));
            if let Err(error) = ran {
                let _ = backend.batch("ROLLBACK");
                return Err(wrong(error));
            }
        }
        Ok(self)
    }

    /// Ready to read and write.
    pub fn build(self) -> SqlDatabase<B> {
        SqlDatabase(Rc::new(Shared {
            backend: self.backend,
            entities: self.entities,
            words: self.words,
            next_keys: RefCell::default(),
        }))
    }
}

impl<B> SqlDatabase<B> {
    /// The database itself, for what only one kind of database does, such as a backup.
    pub fn backend(&self) -> &B {
        &self.0.backend
    }

    /// Forgets the next keys it handed out, so they are counted again from what is stored: after
    /// the records were replaced underneath, as a restore does.
    pub fn forget_next_keys(&self) {
        self.0.next_keys.borrow_mut().clear();
    }
}

impl<B: SqlBackend> SqlDatabase<B> {
    /// Starts one over this. See [`SqlBuilder`].
    pub fn builder(backend: impl Into<B>) -> SqlBuilder<B> {
        SqlBuilder { backend: backend.into(), entities: Vec::new(), words: SqlWords::default() }
    }

    /// A new unit of work.
    pub fn space(&self) -> SqlObjectSpace<B> {
        SqlObjectSpace {
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

/// A unit of work over a [`SqlDatabase`]. A commit is one transaction.
pub struct SqlObjectSpace<B> {
    shared: Rc<Shared<B>>,
    environment: Environment,
    changes: ChangeSet,
    watchers: Vec<SpaceWatcher>,
}

impl<B> fmt::Debug for SqlObjectSpace<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlObjectSpace").finish_non_exhaustive()
    }
}

impl<B: SqlBackend> SqlObjectSpace<B> {
    fn entity(&self, name: &str) -> Result<&Entity, SpaceError> {
        self.shared
            .entities
            .iter()
            .find(|entity| entity.info.name == name)
            .ok_or_else(|| SpaceError::UnknownEntity(name.to_owned()))
    }

    /// Says what the database refused, in words for people.
    fn refused(&self, entity: &Entity, error: &SqlError) -> SpaceError {
        let words = &self.shared.words;
        match error {
            SqlError::ForeignKey => SpaceError::Refused(words.in_use.clone()),
            SqlError::Unique(column) => {
                let caption = column
                    .as_deref()
                    .and_then(|column| {
                        entity.info.fields.iter().find(|f| entity.map.column_of(&f.name) == column)
                    })
                    .map_or_else(String::new, |field| field.caption.clone());
                SpaceError::Refused(words.unique.replace("{0}", &caption))
            }
            SqlError::Constraint => SpaceError::Refused(words.rule.clone()),
            SqlError::Other(why) => SpaceError::Storage(words.failed.replace("{0}", why)),
            SqlError::Busy(why) => SpaceError::Temporary(words.failed.replace("{0}", why)),
        }
    }

    fn select(&self, entity: &Entity, tail: &str, parameters: &[Value]) -> Result<Vec<Object>, SpaceError> {
        self.select_columns(entity, "*", tail, parameters)
    }

    fn select_columns(
        &self,
        entity: &Entity,
        columns: &str,
        tail: &str,
        parameters: &[Value],
    ) -> Result<Vec<Object>, SpaceError> {
        let sql = format!("SELECT {columns} FROM ({}) AS {}{tail}", entity.select(), quoted("record"));
        let read = self.shared.backend.query(&sql, parameters).map_err(|e| self.refused(entity, &e))?;
        let mut found = Vec::with_capacity(read.rows.len());
        for row in read.rows {
            let mut object = entity.read(&read.names, row);
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
        let backend = &self.shared.backend;
        let sql = format!(
            "SELECT 1 FROM {} WHERE {} = {}",
            quoted(&entity.map.table),
            quoted(entity.map.column_of(&field.name)),
            backend.slot(1)
        );
        let value = match field.kind {
            FieldKind::Integer | FieldKind::Decimal(_) => key.parse().map_or(Value::Null, Value::Number),
            _ => Value::Text(key.to_owned()),
        };
        backend.query(&sql, &[value]).is_ok_and(|read| !read.rows.is_empty())
    }

    fn next_key(&self, entity: &Entity, field: &FieldInfo) -> Result<i64, SpaceError> {
        let sql = format!(
            "SELECT COALESCE(MAX({}), 0) + 1 FROM {}",
            quoted(entity.map.column_of(&field.name)),
            quoted(&entity.map.table)
        );
        let read = self.shared.backend.query(&sql, &[]).map_err(|e| self.refused(entity, &e))?;
        let stored = match read.rows.first().and_then(|row| row.first()) {
            Some(SqlCell::Integer(next)) => *next,
            Some(SqlCell::Real(next)) => *next as i64,
            _ => 1,
        };
        let mut keys = self.shared.next_keys.borrow_mut();
        let next = keys.get(&entity.info.name).copied().unwrap_or(0).max(stored);
        keys.insert(entity.info.name.clone(), next + 1);
        Ok(next)
    }

    fn write(&self, entity: &Entity, change: &Change) -> Result<(), SpaceError> {
        let failed = |e: SqlError| self.refused(entity, &e);
        let table = quoted(&entity.map.table);
        let key_field =
            entity.info.key_field().ok_or_else(|| SpaceError::MissingKey(entity.info.name.clone()))?;
        let key_column = quoted(entity.map.column_of(&key_field.name));
        let backend = &self.shared.backend;
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
                let slots: Vec<String> = (1..=stored.len()).map(|at| backend.slot(at)).collect();
                let sql =
                    format!("INSERT INTO {table} ({}) VALUES ({})", columns.join(", "), slots.join(", "));
                let values: Vec<Value> = stored.iter().map(|f| object.get(&f.name)).collect();
                backend.execute(&sql, &values).map_err(failed)?;
            }
            Change::Updated(object) => {
                let object = prepared(object);
                let stored: Vec<&FieldInfo> = entity.stored().filter(|f| !f.is_key).collect();
                let sets: Vec<String> = stored
                    .iter()
                    .enumerate()
                    .map(|(at, f)| {
                        format!("{} = {}", quoted(entity.map.column_of(&f.name)), backend.slot(at + 1))
                    })
                    .collect();
                let sql = format!(
                    "UPDATE {table} SET {} WHERE {key_column} = {}",
                    sets.join(", "),
                    backend.slot(stored.len() + 1)
                );
                let mut values: Vec<Value> = stored.iter().map(|f| object.get(&f.name)).collect();
                values.push(object.get(&key_field.name));
                let changed = backend.execute(&sql, &values).map_err(failed)?;
                if changed == 0 {
                    return Err(SpaceError::Conflict(format!("a record of {} is gone", entity.info.name)));
                }
            }
            Change::Deleted { key, .. } => {
                let sql = format!("DELETE FROM {table} WHERE {key_column} = {}", backend.slot(1));
                backend.execute(&sql, std::slice::from_ref(key)).map_err(failed)?;
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

    fn apply_all(&self, changes: &[Change]) -> Result<(), SpaceError> {
        for change in changes {
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
                let tail = format!(" WHERE {} = {}", quoted(&field.name), self.shared.backend.slot(1));
                let stored = self.select(entity, &tail, &[new.get(&field.name)])?;
                ravenrs_app::check_transitions(&entity.info, stored.first(), new)?;
            }
            self.write(entity, change)?;
        }
        Ok(())
    }
}

impl<B: SqlBackend> ObjectSpace for SqlObjectSpace<B> {
    fn entities(&self) -> Vec<EntityInfo> {
        self.shared.entities.iter().map(|entity| entity.info.clone()).collect()
    }

    fn query(&self, name: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        let entity = self.entity(name)?;
        let backend = &self.shared.backend;
        // What waits for a commit, or what SQL cannot say, goes through memory.
        let translated = query.filter.as_ref().map(|filter| to_sql(filter, backend.dialect()));
        let in_database = !self.changes.touches(name)
            && !entity.info.fields.iter().any(|f| f.calculated.is_some())
            && !matches!(translated, Some(Err(_)));
        if !in_database {
            let rows = self.select(entity, "", &[])?;
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
                    SqlParam::Value(value) => parameters.push(value.clone()),
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
            tail.push_str(&backend.offset_only(query.offset));
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
        self.select_columns(entity, &columns, &tail, &parameters)
    }

    fn find(&self, name: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        let entity = self.entity(name)?;
        let Some(text) = key_text(key) else { return Ok(None) };
        let field = entity.info.key_field().ok_or_else(|| SpaceError::MissingKey(name.to_owned()))?;
        let tail = format!(" WHERE {} = {}", quoted(&field.name), self.shared.backend.slot(1));
        if self.changes.touches(name) {
            let mut by_key = BTreeMap::new();
            for object in self.select(entity, &tail, std::slice::from_ref(key))? {
                by_key.insert(text.clone(), object);
            }
            self.changes.overlay(&entity.info, &mut by_key);
            return Ok(by_key.remove(&text).map(|mut object| {
                entity.info.compute(&mut object, &self.environment);
                object
            }));
        }
        Ok(self.select(entity, &tail, std::slice::from_ref(key))?.into_iter().next())
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
        let storage = |e: SqlError| match e {
            SqlError::Busy(why) => SpaceError::Temporary(why),
            other => SpaceError::Storage(other.to_string()),
        };
        shared.backend.batch("BEGIN").map_err(storage)?;
        if let Err(error) = self.apply_all(&changes) {
            let _ = shared.backend.batch("ROLLBACK");
            return Err(error);
        }
        if let Err(error) = shared.backend.batch("COMMIT") {
            let _ = shared.backend.batch("ROLLBACK");
            return Err(storage(error));
        }
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
