//! Moving a whole tenant from one database to another, of the same kind or not: SQLite to
//! PostgreSQL when an installation grows, and back.
//!
//! [`TenantExport::read`] reads every entity of a database at once, in one transaction, with the
//! stamp of [`BackupStamp`]: the application, the tenant and the migrations it has. For SQLite,
//! the export comes from a stamped backup (`Backups::export` in `ravenrs-app-sqlite`), never from
//! the database in use. [`TenantImport::run`] then writes it into another database:
//!
//! 1. it refuses whoever is not an administrator;
//! 2. it refuses an export of another application, of another tenant or with other migrations,
//!    a destination that is missing an entity or already has records, and an export whose
//!    references point at nothing, listing those rows; nothing is written in any of these cases;
//! 3. it writes everything in a single transaction, entities that are pointed at first;
//! 4. it reads it all back in the same transaction and compares, table by table, the number of
//!    rows and a hash of their content; any difference, or any failure on the way, rolls
//!    everything back and the destination stays as it was;
//! 5. it checks every record against the rules of its entity, and says which ones break them;
//! 6. it writes down, in the audit trail it was given, who moved what and how many rows.
//!
//! The same code writes to any [`SqlBackend`], so the direction is only which database is the
//! source and which is the destination.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use ravenrs_app::{
    AuditTrail, CONTEXT_SAVE, FieldKind, Object, Permissions, RuleWords, Value, key_text, validate_in_place,
};
use ravenrs_criteria::Dialect;

use crate::{Entity, MIGRATIONS, SqlBackend, SqlDatabase, SqlError, quoted};

/// What a backup or an export says about where it came from: the application, the tenant and the
/// migrations it was made with.
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

/// The migrations of the application itself, without those the framework writes for one kind of
/// database only (decimal upgrades and the guards of sealed records), which differ from SQLite
/// to PostgreSQL for the same application.
fn own_migrations(all: &[String]) -> Vec<String> {
    let mut own: Vec<String> = all
        .iter()
        .filter(|id| !id.starts_with("decimal ") && !id.starts_with("sealed guard "))
        .cloned()
        .collect();
    own.sort();
    own
}

/// A row whose reference points at nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BrokenReference {
    /// The entity of the row.
    pub entity: String,
    /// Its key, as text.
    pub key: String,
    /// The field that refers.
    pub field: String,
    /// The value it holds, as text.
    pub points_to: String,
}

/// How one table came across.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TableCheck {
    /// The entity.
    pub entity: String,
    /// Its table in the destination.
    pub table: String,
    /// Rows in the export.
    pub source_rows: usize,
    /// Rows read back from the destination.
    pub target_rows: usize,
    /// A hash of the content in the export, the same whatever the database.
    pub source_hash: String,
    /// The same hash, of what was read back.
    pub target_hash: String,
}

impl TableCheck {
    /// Whether the destination has exactly what the export had.
    pub fn matches(&self) -> bool {
        self.source_rows == self.target_rows && self.source_hash == self.target_hash
    }
}

/// What a migration did and found.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MigrationReport {
    /// What the export said about itself.
    pub stamp: BackupStamp,
    /// The kind of database it came from.
    pub source: Dialect,
    /// The kind of database it went to.
    pub target: Dialect,
    /// Each table, in the order it was written.
    pub tables: Vec<TableCheck>,
    /// How many references were followed, all of them found.
    pub references_checked: usize,
    /// How many records were checked against the rules of their entity.
    pub records_validated: usize,
    /// The records that break a rule now, as `Entity key: message`. They came across as they were.
    pub rule_violations: Vec<String>,
}

impl MigrationReport {
    /// Whether every table matches.
    pub fn matches(&self) -> bool {
        self.tables.iter().all(TableCheck::matches)
    }

    /// How many rows came across.
    pub fn rows(&self) -> usize {
        self.tables.iter().map(|table| table.target_rows).sum()
    }
}

/// Why a tenant was not exported or imported. Whatever it is, the destination is as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MigrationError {
    /// Whoever asked is not an administrator.
    NotAllowed,
    /// The export is of another application.
    OtherApplication(String),
    /// The export is of another tenant, or of none when one is expected.
    OtherTenant(Option<String>),
    /// The export was made with other migrations than the destination has.
    OtherMigrations {
        /// What the export had.
        export: Vec<String>,
        /// What the destination has.
        destination: Vec<String>,
    },
    /// An entity is in one of them and not in the other.
    MissingEntity(String),
    /// The destination already has records in this table.
    NotEmpty(String),
    /// References of the export point at nothing.
    BrokenReferences(Vec<BrokenReference>),
    /// Writing a table failed, and everything was rolled back.
    Failed {
        /// The table.
        table: String,
        /// What the database said.
        error: String,
    },
    /// What was read back is not what was written, and everything was rolled back.
    Mismatch(Vec<TableCheck>),
    /// Reading or writing failed otherwise.
    Storage(String),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAllowed => write!(f, "moving a tenant to another database is not allowed"),
            Self::OtherApplication(name) => write!(f, "that export is of {name}"),
            Self::OtherTenant(Some(name)) => write!(f, "that export is of {name}"),
            Self::OtherTenant(None) => write!(f, "that export is of no one in particular"),
            Self::OtherMigrations { .. } => write!(f, "that export is of another version of the application"),
            Self::MissingEntity(name) => write!(f, "{name} is not in both databases"),
            Self::NotEmpty(table) => write!(f, "the destination already has records in {table}"),
            Self::BrokenReferences(rows) => write!(f, "{} references point at nothing", rows.len()),
            Self::Failed { table, error } => write!(f, "writing {table} failed: {error}"),
            Self::Mismatch(tables) => write!(f, "{} tables did not come across as they were", tables.len()),
            Self::Storage(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for MigrationError {}

fn storage(error: SqlError) -> MigrationError {
    MigrationError::Storage(error.to_string())
}

/// The rows of one entity.
#[derive(Clone, Debug)]
struct Rows {
    entity: String,
    objects: Vec<Object>,
    hash: String,
}

/// Every record of a tenant, read at once, with where it came from. See [the module](self).
#[derive(Clone, Debug)]
pub struct TenantExport {
    stamp: BackupStamp,
    source: Dialect,
    tables: Vec<Rows>,
}

/// A value as the hash sees it: the same whatever database it was read from.
fn canonical(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::Bool(on) => format!("b{on}"),
        Value::Number(number) => format!("n{number}"),
        Value::Text(text) => format!("t{}:{text}", text.len()),
        Value::Date(moment) => format!("d{moment}"),
        other => format!("?{other:?}"),
    }
}

/// A hash of the content of rows, whatever their order (FNV-1a, 64 bits, over sorted lines).
fn content_hash(entity: &Entity, objects: &[Object]) -> String {
    let mut lines: Vec<String> = objects
        .iter()
        .map(|object| {
            entity
                .stored()
                .map(|field| format!("{}={}", field.name, canonical(&object.get(&field.name))))
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect();
    lines.sort();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for line in &lines {
        for byte in line.bytes().chain(std::iter::once(b'\n')) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

fn migrations_of<B: SqlBackend>(backend: &B) -> Result<Vec<String>, SqlError> {
    if backend.columns_of(MIGRATIONS)?.is_empty() {
        return Ok(Vec::new());
    }
    let rows = backend.query(&format!("SELECT id FROM {MIGRATIONS} ORDER BY id"), &[])?;
    Ok(rows
        .rows
        .into_iter()
        .filter_map(|row| match row.into_iter().next() {
            Some(crate::SqlCell::Text(id)) => Some(id),
            _ => None,
        })
        .collect())
}

fn read_all<B: SqlBackend>(backend: &B, entity: &Entity) -> Result<Vec<Object>, SqlError> {
    let rows = backend.query(&entity.select(), &[])?;
    Ok(rows.rows.into_iter().map(|row| entity.read(&rows.names, row)).collect())
}

/// The entities in an order where those pointed at come before those that point.
fn in_order(entities: &[Entity]) -> Vec<&Entity> {
    let names: BTreeSet<&str> = entities.iter().map(|entity| entity.info.name.as_str()).collect();
    let mut placed: Vec<&Entity> = Vec::with_capacity(entities.len());
    let mut done: BTreeSet<&str> = BTreeSet::new();
    while placed.len() < entities.len() {
        let before = placed.len();
        for entity in entities {
            let name = entity.info.name.as_str();
            if done.contains(name) {
                continue;
            }
            let ready = entity.stored().all(|field| match &field.kind {
                FieldKind::Reference(target) => {
                    target == name || !names.contains(target.as_str()) || done.contains(target.as_str())
                }
                _ => true,
            });
            if ready {
                done.insert(name);
                placed.push(entity);
            }
        }
        // References in a circle: the rest goes as it is.
        if placed.len() == before {
            for entity in entities {
                if done.insert(entity.info.name.as_str()) {
                    placed.push(entity);
                }
            }
        }
    }
    placed
}

impl TenantExport {
    /// Every record of `database`, read in one transaction, stamped with `stamp` and the
    /// migrations the database has.
    pub fn read<B: SqlBackend>(
        database: &SqlDatabase<B>,
        stamp: BackupStamp,
    ) -> Result<Self, MigrationError> {
        let shared = &database.0;
        let backend = &shared.backend;
        let begin = match backend.dialect() {
            Dialect::Postgres => "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",
            _ => "BEGIN",
        };
        backend.batch(begin).map_err(storage)?;
        let read = || -> Result<(Vec<String>, Vec<Rows>), SqlError> {
            let migrations = migrations_of(backend)?;
            let mut tables = Vec::new();
            for entity in in_order(&shared.entities) {
                let objects = read_all(backend, entity)?;
                let hash = content_hash(entity, &objects);
                tables.push(Rows { entity: entity.info.name.clone(), objects, hash });
            }
            Ok((migrations, tables))
        };
        let result = read();
        let _ = backend.batch(if result.is_ok() { "COMMIT" } else { "ROLLBACK" });
        let (migrations, tables) = result.map_err(storage)?;
        let mut stamp = stamp;
        stamp.migrations = migrations;
        Ok(Self { stamp, source: backend.dialect(), tables })
    }

    /// Where it came from.
    pub fn stamp(&self) -> &BackupStamp {
        &self.stamp
    }

    /// The kind of database it came from.
    pub fn source(&self) -> Dialect {
        self.source
    }

    /// How many rows each entity has, in the order they are written.
    pub fn counts(&self) -> Vec<(String, usize)> {
        self.tables.iter().map(|rows| (rows.entity.clone(), rows.objects.len())).collect()
    }

    /// The hash of the content of each entity, the same whatever database it was read from: two
    /// exports with the same hashes hold the same records.
    pub fn hashes(&self) -> Vec<(String, String)> {
        self.tables.iter().map(|rows| (rows.entity.clone(), rows.hash.clone())).collect()
    }

    /// The references that point at nothing in this export.
    pub fn broken_references(&self, database_entities: &[ravenrs_app::EntityInfo]) -> Vec<BrokenReference> {
        let keys: BTreeMap<&str, BTreeSet<String>> = self
            .tables
            .iter()
            .filter_map(|rows| {
                let info = database_entities.iter().find(|info| info.name == rows.entity)?;
                let key = info.key_field()?;
                let found =
                    rows.objects.iter().filter_map(|object| key_text(&object.get(&key.name))).collect();
                Some((rows.entity.as_str(), found))
            })
            .collect();
        let mut broken = Vec::new();
        for rows in &self.tables {
            let Some(info) = database_entities.iter().find(|info| info.name == rows.entity) else { continue };
            for field in &info.fields {
                let FieldKind::Reference(target) = &field.kind else { continue };
                let Some(targets) = keys.get(target.as_str()) else { continue };
                for object in &rows.objects {
                    let Some(points_to) = key_text(&object.get(&field.name)) else { continue };
                    if !targets.contains(&points_to) {
                        broken.push(BrokenReference {
                            entity: rows.entity.clone(),
                            key: info
                                .key_field()
                                .and_then(|key| key_text(&object.get(&key.name)))
                                .unwrap_or_default(),
                            field: field.name.clone(),
                            points_to,
                        });
                    }
                }
            }
        }
        broken
    }
}

/// Writes a [`TenantExport`] into a database. See [the module](self).
pub struct TenantImport<'a, B> {
    database: &'a SqlDatabase<B>,
    expected: BackupStamp,
    audit: Option<Rc<AuditTrail>>,
}

impl<B> fmt::Debug for TenantImport<'_, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantImport").field("expected", &self.expected).finish_non_exhaustive()
    }
}

impl<'a, B: SqlBackend> TenantImport<'a, B> {
    /// Into `database`, which takes only exports of the application and tenant of `expected`.
    pub fn new(database: &'a SqlDatabase<B>, expected: BackupStamp) -> Self {
        Self { database, expected, audit: None }
    }

    /// Writes down each migration in this trail, as a note about the tenant.
    pub fn audit(mut self, trail: Rc<AuditTrail>) -> Self {
        self.audit = Some(trail);
        self
    }

    /// Checks `export`, writes it in one transaction, reads it back and compares. Only an
    /// administrator moves a tenant.
    pub fn run(
        &self,
        export: &TenantExport,
        permissions: &Permissions,
    ) -> Result<MigrationReport, MigrationError> {
        if !permissions.is_administrator() {
            return Err(MigrationError::NotAllowed);
        }
        let shared = &self.database.0;
        let backend = &shared.backend;
        let stamp = &export.stamp;
        if stamp.application != self.expected.application {
            return Err(MigrationError::OtherApplication(stamp.application.clone()));
        }
        if stamp.tenant != self.expected.tenant {
            return Err(MigrationError::OtherTenant(stamp.tenant.clone()));
        }
        let (theirs, ours) =
            (own_migrations(&stamp.migrations), own_migrations(&migrations_of(backend).map_err(storage)?));
        if theirs != ours {
            return Err(MigrationError::OtherMigrations { export: theirs, destination: ours });
        }
        for entity in &shared.entities {
            if !export.tables.iter().any(|rows| rows.entity == entity.info.name) {
                return Err(MigrationError::MissingEntity(entity.info.name.clone()));
            }
        }
        let mut pairs: Vec<(&Entity, &Rows)> = Vec::new();
        for rows in &export.tables {
            let entity = shared
                .entities
                .iter()
                .find(|entity| entity.info.name == rows.entity)
                .ok_or_else(|| MigrationError::MissingEntity(rows.entity.clone()))?;
            pairs.push((entity, rows));
        }
        let infos = self.database.entities();
        let broken = export.broken_references(&infos);
        if !broken.is_empty() {
            return Err(MigrationError::BrokenReferences(broken));
        }
        for (entity, _) in &pairs {
            let count = backend
                .query(&format!("SELECT COUNT(*) FROM {}", quoted(&entity.map.table)), &[])
                .map_err(storage)?;
            let has = matches!(count.rows.first().and_then(|row| row.first()), Some(crate::SqlCell::Integer(n)) if *n > 0);
            if has {
                return Err(MigrationError::NotEmpty(entity.map.table.clone()));
            }
        }

        backend.batch("BEGIN").map_err(storage)?;
        let written = self.write_and_check(&pairs);
        let tables = match written {
            Ok(tables) => tables,
            Err(error) => {
                let _ = backend.batch("ROLLBACK");
                return Err(error);
            }
        };
        if let Err(error) = backend.batch("COMMIT") {
            let _ = backend.batch("ROLLBACK");
            return Err(storage(error));
        }
        self.database.forget_next_keys();

        let (records_validated, rule_violations) = self.revalidate(&pairs);
        let references_checked = pairs
            .iter()
            .map(|(entity, rows)| {
                let refs = entity.stored().filter(|field| matches!(field.kind, FieldKind::Reference(_)));
                refs.map(|field| {
                    rows.objects.iter().filter(|o| key_text(&o.get(&field.name)).is_some()).count()
                })
                .sum::<usize>()
            })
            .sum();
        let report = MigrationReport {
            stamp: stamp.clone(),
            source: export.source,
            target: backend.dialect(),
            tables,
            references_checked,
            records_validated,
            rule_violations,
        };
        if let Some(trail) = &self.audit {
            let tenant = stamp.tenant.clone().unwrap_or_default();
            let what = format!(
                "{} {:?} -> {:?}: {} tables, {} rows",
                stamp.application,
                report.source,
                report.target,
                report.tables.len(),
                report.rows()
            );
            trail.custom("Tenant", &Value::Text(tenant), "migrated", &what);
        }
        Ok(report)
    }

    fn write_and_check(&self, pairs: &[(&Entity, &Rows)]) -> Result<Vec<TableCheck>, MigrationError> {
        let backend = &self.database.0.backend;
        for (entity, rows) in pairs {
            let failed = |error: SqlError| MigrationError::Failed {
                table: entity.map.table.clone(),
                error: error.to_string(),
            };
            let fields: Vec<_> = entity.stored().collect();
            let columns: Vec<String> = fields.iter().map(|f| quoted(entity.map.column_of(&f.name))).collect();
            let slots: Vec<String> = (1..=fields.len()).map(|at| backend.slot(at)).collect();
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                quoted(&entity.map.table),
                columns.join(", "),
                slots.join(", ")
            );
            for object in &rows.objects {
                let values: Vec<Value> = fields.iter().map(|f| object.get(&f.name)).collect();
                backend.execute(&sql, &values).map_err(failed)?;
            }
        }
        let mut checks = Vec::with_capacity(pairs.len());
        for (entity, rows) in pairs {
            let back = read_all(backend, entity).map_err(storage)?;
            checks.push(TableCheck {
                entity: entity.info.name.clone(),
                table: entity.map.table.clone(),
                source_rows: rows.objects.len(),
                target_rows: back.len(),
                source_hash: rows.hash.clone(),
                target_hash: content_hash(entity, &back),
            });
        }
        let wrong: Vec<TableCheck> = checks.iter().filter(|check| !check.matches()).cloned().collect();
        if !wrong.is_empty() {
            return Err(MigrationError::Mismatch(wrong));
        }
        Ok(checks)
    }

    fn revalidate(&self, pairs: &[(&Entity, &Rows)]) -> (usize, Vec<String>) {
        let space = self.database.space();
        let words = RuleWords::default();
        let mut checked = 0;
        let mut violations = Vec::new();
        for (entity, rows) in pairs {
            if entity.info.rules.is_empty() {
                continue;
            }
            let key = entity.info.key_field();
            for object in &rows.objects {
                checked += 1;
                for violation in validate_in_place(&entity.info, object, CONTEXT_SAVE, &space, &words) {
                    if violation.is_error() {
                        let key = key.and_then(|key| key_text(&object.get(&key.name))).unwrap_or_default();
                        violations.push(format!("{} {key}: {}", entity.info.name, violation.message));
                    }
                }
            }
        }
        (checked, violations)
    }
}
