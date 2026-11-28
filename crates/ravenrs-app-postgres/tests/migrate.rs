//! Moving a tenant from SQLite to PostgreSQL and back (#243), against a real PostgreSQL when
//! `RAVENRS_POSTGRES_URL` says where one is. Each tenant has a schema of its own, with the same
//! keys as the other, and the migration of one leaves the other as it was (scenario C04 of #253).
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::rc::Rc;

use ravenrs_app::{
    EntityInfo, FieldInfo, FieldKind, Identity, ObjectSpace, Permissions, Query, Role, RoleMerge, Rule, Value,
};
use ravenrs_app_postgres::{
    BackupStamp, MigrationError, PostgresBackend, PostgresDatabase, TenantExport, TenantImport,
};
use ravenrs_app_sqlite::{SqliteBackups, SqliteDatabase, TableMap};
use ravenrs_criteria::DateTime;

mod support;

fn url() -> Option<String> {
    std::env::var("RAVENRS_POSTGRES_URL").ok()
}

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

/// The tenant's own schema, empty.
fn postgres(schema: &str) -> PostgresDatabase {
    let address = url().unwrap();
    let mut admin = support::raw(&address);
    admin.batch_execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}")).unwrap();
    let mut client = support::raw(&address);
    client.batch_execute(&format!("SET search_path TO {schema}")).unwrap();
    PostgresDatabase::builder(PostgresBackend::from(client))
        .managed(unit(), TableMap::new("units"))
        .and_then(|b| b.managed(member(), TableMap::new("members")))
        .and_then(|b| b.migration("0001 members", "SELECT 1"))
        .unwrap()
        .build()
}

fn drop_schema(schema: &str) {
    support::raw(&url().unwrap()).batch_execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).unwrap();
}

fn sqlite(file: &Path) -> SqliteDatabase {
    let connection = Rc::new(rusqlite::Connection::open(file).unwrap());
    SqliteDatabase::builder(connection)
        .managed(unit(), TableMap::new("units"))
        .and_then(|b| b.managed(member(), TableMap::new("members")))
        .and_then(|b| b.migration("0001 members", "SELECT 1"))
        .unwrap()
        .build()
}

fn put(space: &mut impl ObjectSpace, entity: &str, values: &[(&str, Value)]) {
    let mut object = space.new_object(entity).unwrap();
    for (field, value) in values {
        object.set(field, value.clone());
    }
    space.insert(object).unwrap();
    space.commit().unwrap();
}

fn fill(space: &mut impl ObjectSpace, who: &str) {
    put(space, "Unit", &[("id", Value::Number(1.0)), ("name", Value::Text(format!("{who} U1")))]);
    put(space, "Unit", &[("id", Value::Number(2.0)), ("name", Value::Text(format!("{who} U2")))]);
    for (id, name, unit) in [(1.0, "Ana", 1.0), (2.0, "Bia", 1.0), (3.0, "Caio", 2.0)] {
        put(
            space,
            "Member",
            &[
                ("id", Value::Number(id)),
                ("name", Value::Text(format!("{who} {name}"))),
                ("unit", Value::Number(unit)),
                ("share", Value::Number(12.25 * id)),
                ("active", Value::Bool(id != 2.0)),
                ("born", Value::Date(DateTime::from_ymd(1990, 1, id as u32).unwrap())),
            ],
        );
    }
}

fn stamp(tenant: &str) -> BackupStamp {
    BackupStamp::new("Church").tenant(tenant)
}

fn administrator(entities: Vec<EntityInfo>) -> Permissions {
    Permissions::new(
        Identity::new(1, "admin", vec![Role::new("Admin").administrative()]),
        entities,
        RoleMerge::AnyRole,
    )
}

fn folder(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("ravenrs-app-postgres-migrate-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// T1 on SQLite, saved to a stamped backup and read from it.
fn t1_export(folder: &Path) -> TenantExport {
    let source = sqlite(&folder.join("t1.db"));
    fill(&mut source.space(), "T1");
    let backups = source.backups(stamp("T1"));
    backups.save(&folder.join("t1-backup.db")).unwrap();
    backups.export(&folder.join("t1-backup.db")).unwrap()
}

#[test]
fn sqlite_to_postgres_and_back_keeps_every_row_and_leaves_the_other_tenant_alone() {
    if url().is_none() {
        return;
    }
    let folder = folder("round-trip");
    let export = t1_export(&folder);

    // T2 is already on the server, with the same keys.
    let t2 = postgres("raven_migrate_t2");
    fill(&mut t2.space(), "T2");
    let t2_before = TenantExport::read(&t2, stamp("T2")).unwrap();

    let t1 = postgres("raven_migrate_t1");
    let report = TenantImport::new(&t1, stamp("T1")).run(&export, &administrator(t1.entities())).unwrap();
    assert!(report.matches(), "{:?}", report.tables);
    assert_eq!(report.rows(), 5);
    let on_server = TenantExport::read(&t1, stamp("T1")).unwrap();
    assert_eq!(on_server.hashes(), export.hashes());
    assert_eq!(TenantExport::read(&t2, stamp("T2")).unwrap().hashes(), t2_before.hashes());
    assert_ne!(t2_before.hashes(), export.hashes());

    // And back to a new SQLite file.
    let back = sqlite(&folder.join("back.db"));
    TenantImport::new(&back, stamp("T1")).run(&on_server, &administrator(back.entities())).unwrap();
    let again = TenantExport::read(&back, stamp("T1")).unwrap();
    assert_eq!(again.counts(), export.counts());
    assert_eq!(again.hashes(), export.hashes());

    // T1 on the server goes on with keys from what came across.
    put(&mut t1.space(), "Unit", &[("name", Value::Text("new".into()))]);
    assert_eq!(t1.space().query("Unit", &Query::all()).unwrap().len(), 3);
    drop_schema("raven_migrate_t1");
    drop_schema("raven_migrate_t2");
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn a_failure_on_the_server_rolls_everything_back() {
    if url().is_none() {
        return;
    }
    let folder = folder("fails");
    let export = t1_export(&folder);
    // The destination's own rule refuses one member, in the second table written.
    let t1 = postgres("raven_migrate_fails");
    support::raw(&url().unwrap())
        .batch_execute(
            "ALTER TABLE raven_migrate_fails.members ADD CONSTRAINT refuse_bia CHECK (name <> 'T1 Bia')",
        )
        .unwrap();
    match TenantImport::new(&t1, stamp("T1")).run(&export, &administrator(t1.entities())) {
        Err(MigrationError::Failed { table, .. }) => assert_eq!(table, "members"),
        other => panic!("expected a failure, got {other:?}"),
    }
    assert!(t1.space().query("Unit", &Query::all()).unwrap().is_empty());
    assert!(t1.space().query("Member", &Query::all()).unwrap().is_empty());
    // The same import, once the rule is gone, goes through on the same connection.
    support::raw(&url().unwrap())
        .batch_execute("ALTER TABLE raven_migrate_fails.members DROP CONSTRAINT refuse_bia")
        .unwrap();
    assert!(
        TenantImport::new(&t1, stamp("T1")).run(&export, &administrator(t1.entities())).unwrap().matches()
    );
    drop_schema("raven_migrate_fails");
    std::fs::remove_dir_all(&folder).unwrap();
}
