//! Moving a tenant from a stamped backup to another database (#243). The same import writes to
//! PostgreSQL in that crate's tests; here both ends are SQLite files.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::rc::Rc;

use ravenrs_app::{
    AuditSettings, AuditTrail, EntityInfo, FieldInfo, FieldKind, Identity, ObjectSpace, Permissions, Query,
    Role, RoleMerge, Rule, SECURITY_AUDIT_ENTRY, SpaceMaker, Value, audit_entity,
};
use ravenrs_app_sqlite::{
    BackupError, BackupStamp, MigrationError, SqliteBackups, SqliteDatabase, TableMap, TenantExport,
    TenantImport,
};
use ravenrs_criteria::DateTime;
use rusqlite::Connection;

fn unit() -> EntityInfo {
    EntityInfo::new("Unit")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
}

fn member() -> EntityInfo {
    EntityInfo::new("Member")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
        .field(FieldInfo::new("unit", FieldKind::Reference("Unit".into())))
        .field(FieldInfo::new("share", FieldKind::Decimal(2)))
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("born", FieldKind::Date))
        .rule(Rule::required("name"))
}

fn folder(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("ravenrs-app-sqlite-migrate-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    folder
}

/// A database of the application, with its audit trail, on `file`. `before` runs on the
/// connection first, to set a table up by hand.
fn open_with(file: &Path, migrations: &[&str], before: &str) -> SqliteDatabase {
    let connection = Rc::new(Connection::open(file).expect("opens"));
    connection.execute_batch(before).expect("set up");
    let mut builder = SqliteDatabase::builder(connection)
        .managed(unit(), TableMap::new("units"))
        .and_then(|b| b.managed(member(), TableMap::new("members")))
        .and_then(|b| b.managed(audit_entity(), TableMap::new("audit")))
        .expect("valid");
    for id in migrations {
        builder = builder.migration(id, "SELECT 1").expect("migrates");
    }
    builder.build()
}

fn open(file: &Path) -> SqliteDatabase {
    open_with(file, &["0001 members"], "")
}

fn put(database: &SqliteDatabase, entity: &str, values: &[(&str, Value)]) {
    let mut space = database.space();
    let mut object = space.new_object(entity).expect("known");
    for (field, value) in values {
        object.set(field, value.clone());
    }
    space.insert(object).expect("inserts");
    space.commit().expect("keeps");
}

/// T1 and T2 have the same keys, on purpose.
fn fill(database: &SqliteDatabase, who: &str) {
    put(database, "Unit", &[("id", Value::Number(1.0)), ("name", Value::Text(format!("{who} U1")))]);
    put(database, "Unit", &[("id", Value::Number(2.0)), ("name", Value::Text(format!("{who} U2")))]);
    for (id, name, unit) in [(1.0, "Ana", 1.0), (2.0, "Bia", 1.0), (3.0, "Caio", 2.0)] {
        put(
            database,
            "Member",
            &[
                ("id", Value::Number(id)),
                ("name", Value::Text(format!("{who} {name}"))),
                ("unit", Value::Number(unit)),
                ("share", Value::Number(12.5 * id)),
                ("active", Value::Bool(id != 2.0)),
                ("born", Value::Date(DateTime::from_ymd(1990, 1, id as u32).expect("a day"))),
            ],
        );
    }
}

fn stamp(tenant: &str) -> BackupStamp {
    BackupStamp::new("Church").tenant(tenant)
}

fn administrator(database: &SqliteDatabase) -> Permissions {
    let identity = Identity::new(1, "admin", vec![Role::new("Admin").administrative()]);
    Permissions::new(identity, database.entities(), RoleMerge::AnyRole)
}

fn operator(database: &SqliteDatabase) -> Permissions {
    Permissions::new(
        Identity::new(2, "operator", vec![Role::new("Operator")]),
        database.entities(),
        RoleMerge::AnyRole,
    )
}

fn count(database: &SqliteDatabase, entity: &str) -> usize {
    database.space().query(entity, &Query::all()).expect("reads").len()
}

/// A backup of T1, saved in `folder`, and the export read from it.
fn exported(folder: &Path, tenant: &str) -> (SqliteDatabase, TenantExport) {
    let source = open(&folder.join(format!("{tenant}.db")));
    fill(&source, tenant);
    let backups = source.backups(stamp(tenant));
    let file = folder.join(format!("{tenant}-backup.db"));
    backups.save(&file).expect("saves");
    let export = backups.export(&file).expect("exports");
    (source, export)
}

#[test]
fn a_tenant_goes_and_comes_back_with_the_same_content() {
    let folder = folder("round-trip");
    let (_source, export) = exported(&folder, "T1");
    assert_eq!(export.counts(), [("Unit".into(), 2), ("Member".into(), 3), (SECURITY_AUDIT_ENTRY.into(), 0)]);

    let middle = open(&folder.join("middle.db"));
    let report =
        TenantImport::new(&middle, stamp("T1")).run(&export, &administrator(&middle)).expect("moves");
    assert!(report.matches());
    assert_eq!(report.rows(), 5);
    assert_eq!(report.references_checked, 3);
    assert_eq!(report.records_validated, 3);
    assert!(report.rule_violations.is_empty());

    // And back, from a backup of the middle one.
    let backups = middle.backups(stamp("T1"));
    backups.save(&folder.join("middle-backup.db")).expect("saves");
    let again = backups.export(&folder.join("middle-backup.db")).expect("exports");
    let last = open(&folder.join("last.db"));
    TenantImport::new(&last, stamp("T1")).run(&again, &administrator(&last)).expect("moves back");
    let back = TenantExport::read(&last, stamp("T1")).expect("reads");
    assert_eq!(back.counts(), export.counts());
    assert_eq!(back.hashes(), export.hashes());
    // Keys go on from what came across.
    put(&last, "Unit", &[("name", Value::Text("new".into()))]);
    assert_eq!(count(&last, "Unit"), 3);
    std::fs::remove_dir_all(&folder).expect("cleans up");
}

#[test]
fn a_failure_in_the_second_table_leaves_the_destination_as_it_was() {
    let folder = folder("fails");
    let (_source, export) = exported(&folder, "T1");
    // The destination's own rule refuses one member, in the second table written.
    let refusing = "CREATE TABLE members (id INTEGER PRIMARY KEY, name TEXT CHECK (name <> 'T1 Bia'), \
                    unit INTEGER, share TEXT, active INTEGER, born TEXT)";
    let target = open_with(&folder.join("target.db"), &["0001 members"], refusing);
    let refused = TenantImport::new(&target, stamp("T1")).run(&export, &administrator(&target));
    match refused {
        Err(MigrationError::Failed { table, .. }) => assert_eq!(table, "members"),
        other => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(count(&target, "Unit"), 0, "the first table was rolled back too");
    assert_eq!(count(&target, "Member"), 0);
    std::fs::remove_dir_all(&folder).expect("cleans up");
}

#[test]
fn another_tenant_or_other_migrations_are_refused_before_writing() {
    let folder = folder("refused");
    let (_source, export) = exported(&folder, "T1");
    // T2 has records with the same keys, and is not touched.
    let t2 = open(&folder.join("T2.db"));
    fill(&t2, "T2");
    let before = TenantExport::read(&t2, stamp("T2")).expect("reads");
    assert_eq!(
        TenantImport::new(&t2, stamp("T2")).run(&export, &administrator(&t2)).err(),
        Some(MigrationError::OtherTenant(Some("T1".into())))
    );
    assert_eq!(TenantExport::read(&t2, stamp("T2")).expect("reads").hashes(), before.hashes());

    let newer = open_with(&folder.join("newer.db"), &["0001 members", "0002 more"], "");
    assert!(matches!(
        TenantImport::new(&newer, stamp("T1")).run(&export, &administrator(&newer)),
        Err(MigrationError::OtherMigrations { .. })
    ));
    assert_eq!(count(&newer, "Unit"), 0);

    // One that already has records is not mixed into.
    let busy = open(&folder.join("busy.db"));
    put(&busy, "Unit", &[("id", Value::Number(9.0)), ("name", Value::Text("there".into()))]);
    assert_eq!(
        TenantImport::new(&busy, stamp("T1")).run(&export, &administrator(&busy)).err(),
        Some(MigrationError::NotEmpty("units".into()))
    );

    // A backup of another tenant does not even export.
    let backups = t2.backups(stamp("T2"));
    assert_eq!(
        backups.export(&folder.join("T1-backup.db")).err(),
        Some(BackupError::OtherTenant(Some("T1".into())))
    );
    std::fs::remove_dir_all(&folder).expect("cleans up");
}

#[test]
fn a_broken_reference_is_refused_with_its_rows() {
    let folder = folder("broken");
    let source = open(&folder.join("T1.db"));
    fill(&source, "T1");
    put(
        &source,
        "Member",
        &[("id", Value::Number(4.0)), ("name", Value::Text("Lost".into())), ("unit", Value::Number(7.0))],
    );
    let export = TenantExport::read(&source, stamp("T1")).expect("reads");
    let target = open(&folder.join("target.db"));
    match TenantImport::new(&target, stamp("T1")).run(&export, &administrator(&target)) {
        Err(MigrationError::BrokenReferences(rows)) => {
            assert_eq!(rows.len(), 1);
            assert_eq!((rows[0].entity.as_str(), rows[0].key.as_str()), ("Member", "4"));
            assert_eq!((rows[0].field.as_str(), rows[0].points_to.as_str()), ("unit", "7"));
        }
        other => panic!("expected broken references, got {other:?}"),
    }
    assert_eq!(count(&target, "Member"), 0);
    std::fs::remove_dir_all(&folder).expect("cleans up");
}

#[test]
fn only_an_administrator_moves_a_tenant_and_it_is_in_the_audit_trail() {
    let folder = folder("audit");
    let (_source, export) = exported(&folder, "T1");
    let target = open(&folder.join("target.db"));
    assert_eq!(
        TenantImport::new(&target, stamp("T1")).run(&export, &operator(&target)).err(),
        Some(MigrationError::NotAllowed)
    );
    assert_eq!(count(&target, "Unit"), 0);

    let trail = Rc::new(AuditTrail::new(AuditSettings::default(), || "admin".to_owned()));
    let writer = target.clone();
    let maker: SpaceMaker = Rc::new(move || Box::new(writer.space()));
    trail.write_with(maker);
    TenantImport::new(&target, stamp("T1"))
        .audit(trail)
        .run(&export, &administrator(&target))
        .expect("moves");
    let lines = target.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].get("author"), Value::Text("admin".into()));
    assert_eq!(lines[0].get("target_key"), Value::Text("T1".into()));
    let Value::Text(what) = lines[0].get("new_value") else { panic!("a note") };
    assert!(what.contains("3 tables"), "{what}");
    assert!(what.contains("5 rows"), "{what}");
    std::fs::remove_dir_all(&folder).expect("cleans up");
}
