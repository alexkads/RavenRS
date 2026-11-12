//! Backups and restores on real files.
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, Value};
use ravenrs_app_sqlite::{BackupError, BackupStamp, Backups, SqliteDatabase, TableMap};
use rusqlite::Connection;

fn member() -> EntityInfo {
    EntityInfo::new("Member")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
}

/// A folder of its own for each test, empty.
fn folder(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("ravenrs-app-sqlite-backup-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    folder
}

fn open(file: &Path, migrations: &[(&str, &str)]) -> SqliteDatabase {
    let connection = Rc::new(Connection::open(file).expect("opens"));
    let mut builder =
        SqliteDatabase::builder(connection).managed(member(), TableMap::new("members")).expect("valid");
    for (id, sql) in migrations {
        builder = builder.migration(id, sql).expect("migrates");
    }
    builder.build()
}

fn add(database: &SqliteDatabase, name: &str) {
    let mut space = database.space();
    let mut object = space.new_object("Member").expect("known");
    object.set("name", name);
    space.save(object).expect("saves");
    space.commit().expect("keeps");
}

fn names(database: &SqliteDatabase) -> Vec<String> {
    let mut found: Vec<String> = database
        .space()
        .query("Member", &Query::all())
        .expect("reads")
        .into_iter()
        .map(|object| match object.get("name") {
            Value::Text(text) => text,
            other => format!("{other:?}"),
        })
        .collect();
    found.sort();
    found
}

fn church(database: &SqliteDatabase, tenant: &str, at: &Path) -> Backups {
    database.backups(BackupStamp::new("Church").tenant(tenant)).copies_in(at.join("copies"))
}

#[test]
fn a_backup_comes_back_as_it_was_and_new_keys_follow_it() {
    let at = folder("round-trip");
    let database = open(&at.join("central.db"), &[]);
    add(&database, "Ana");
    add(&database, "Bia");
    let backups = church(&database, "central", &at);
    let info = backups.save(&at.join("backup.db")).expect("saved");
    assert_eq!(info.stamp.tenant.as_deref(), Some("central"));
    assert!(info.rows.contains(&("members".to_owned(), 2)));

    add(&database, "Caio");
    add(&database, "Duda");
    let (_, copy) = backups.restore(&at.join("backup.db")).expect("restored");
    assert_eq!(names(&database), ["Ana", "Bia"]);
    let copy = copy.expect("a copy of what was there");
    assert!(backups.inspect(&copy).expect("a backup too").rows.contains(&("members".to_owned(), 4)));

    // Keys counted before the restore would collide with the rows it brought back.
    add(&database, "Eva");
    add(&database, "Fábio");
    assert_eq!(names(&database), ["Ana", "Bia", "Eva", "Fábio"]);
}

#[test]
fn it_never_overwrites_a_file_nor_backs_up_onto_itself() {
    let at = folder("overwrite");
    let file = at.join("central.db");
    let database = open(&file, &[]);
    let backups = church(&database, "central", &at);
    std::fs::write(at.join("taken.db"), b"something else").expect("written");
    assert_eq!(backups.save(&at.join("taken.db")), Err(BackupError::Exists(at.join("taken.db"))));
    assert_eq!(std::fs::read(at.join("taken.db")).expect("still there"), b"something else");
    assert_eq!(backups.save(&file).map(|_| ()), Err(BackupError::SameFile));
    assert_eq!(backups.inspect(&file).map(|_| ()), Err(BackupError::SameFile));
}

#[test]
fn another_tenant_or_application_is_refused_and_nothing_changes() {
    let at = folder("tenants");
    let central = open(&at.join("central.db"), &[]);
    add(&central, "From central");
    church(&central, "central", &at).save(&at.join("central-backup.db")).expect("saved");
    central.backups(BackupStamp::new("Shop")).save(&at.join("shop-backup.db")).expect("saved");

    let north = open(&at.join("north.db"), &[]);
    add(&north, "From north");
    let backups = church(&north, "north", &at);
    assert_eq!(
        backups.restore(&at.join("central-backup.db")).map(|_| ()),
        Err(BackupError::OtherTenant(Some("central".into())))
    );
    assert_eq!(
        backups.restore(&at.join("shop-backup.db")).map(|_| ()),
        Err(BackupError::OtherApplication("Shop".into()))
    );
    let untenanted = north.backups(BackupStamp::new("Church"));
    assert_eq!(
        untenanted.inspect(&at.join("central-backup.db")).map(|_| ()),
        Err(BackupError::OtherTenant(Some("central".into())))
    );
    assert_eq!(names(&north), ["From north"]);
    assert!(backups.copies().is_empty(), "nothing to undo, so no copy");
}

#[test]
fn what_is_not_a_backup_or_is_damaged_says_so() {
    let at = folder("not-a-backup");
    let database = open(&at.join("central.db"), &[]);
    add(&database, "Ana");
    let backups = church(&database, "central", &at);

    std::fs::write(at.join("photo.png"), b"\x89PNG not a database").expect("written");
    assert_eq!(backups.inspect(&at.join("photo.png")).map(|_| ()), Err(BackupError::NotABackup));
    // A database, but not a backup: it has no stamp.
    let _plain = open(&at.join("plain.db"), &[]);
    assert_eq!(backups.inspect(&at.join("plain.db")).map(|_| ()), Err(BackupError::NotABackup));

    let good = at.join("good.db");
    backups.save(&good).expect("saved");
    let mut bytes = std::fs::read(&good).expect("read");
    let middle = bytes.len() / 2;
    for byte in &mut bytes[middle..middle + 512] {
        *byte = 0xA5;
    }
    std::fs::write(at.join("damaged.db"), bytes).expect("written");
    let damaged = backups.inspect(&at.join("damaged.db")).map(|_| ());
    assert!(matches!(damaged, Err(BackupError::Damaged | BackupError::NotABackup)), "{damaged:?}");
    assert_eq!(names(&database), ["Ana"]);
}

#[test]
fn a_backup_of_other_migrations_is_refused() {
    let at = folder("migrations");
    let old = open(&at.join("old.db"), &[("2026-01-a", "CREATE TABLE notes (id INTEGER PRIMARY KEY)")]);
    church(&old, "central", &at).save(&at.join("old-backup.db")).expect("saved");
    let new = open(
        &at.join("new.db"),
        &[
            ("2026-01-a", "CREATE TABLE notes (id INTEGER PRIMARY KEY)"),
            ("2026-02-b", "ALTER TABLE notes ADD COLUMN body TEXT"),
        ],
    );
    let backups = church(&new, "central", &at);
    assert_eq!(backups.inspect(&at.join("old-backup.db")).map(|_| ()), Err(BackupError::OtherMigrations));
    church(&new, "central", &at).save(&at.join("new-backup.db")).expect("saved");
    assert_eq!(
        church(&old, "central", &at).inspect(&at.join("new-backup.db")).map(|_| ()),
        Err(BackupError::OtherMigrations)
    );
}

#[test]
fn a_restore_that_does_not_hold_together_changes_nothing() {
    let at = folder("broken");
    let schema = [(
        "2026-01-families",
        "CREATE TABLE families (id INTEGER PRIMARY KEY);
         CREATE TABLE people (id INTEGER PRIMARY KEY, family INTEGER REFERENCES families(id));
         INSERT INTO families (id) VALUES (1);
         INSERT INTO people (id, family) VALUES (1, 1);",
    )];
    let database = open(&at.join("central.db"), &schema);
    add(&database, "Ana");
    let backups = church(&database, "central", &at);
    backups.save(&at.join("backup.db")).expect("saved");
    // Someone took the family out of the backup and left the person pointing at it.
    let edited = Connection::open(at.join("backup.db")).expect("opens");
    edited.pragma_update(None, "foreign_keys", false).expect("unchecked");
    edited.execute("DELETE FROM families", []).expect("edited");
    drop(edited);

    add(&database, "Bia");
    assert_eq!(backups.restore(&at.join("backup.db")).map(|_| ()), Err(BackupError::Broken));
    assert_eq!(names(&database), ["Ana", "Bia"], "everything as it was");
}

#[test]
fn copies_are_kept_up_to_a_number_newest_first() {
    let at = folder("copies");
    let database = open(&at.join("central.db"), &[]);
    let backups = church(&database, "central", &at).keep(2);
    let first = backups.keep_copy("daily").expect("kept");
    let second = backups.keep_copy("daily").expect("kept");
    let third = backups.keep_copy("before erase").expect("kept");
    let copies = backups.copies();
    assert_eq!(copies.len(), 2);
    assert!(!first.exists(), "the oldest went");
    assert!(second.exists() && third.exists());
    assert!(copies.iter().any(|copy| copy.reason == "before-erase"));
}

#[test]
fn a_backup_has_what_was_committed_while_the_database_is_in_use() {
    let at = folder("wal");
    let file = at.join("central.db");
    let database = open(&file, &[]);
    let other = Connection::open(&file).expect("another connection");
    other.pragma_update(None, "journal_mode", "WAL").expect("wal");
    add(&database, "Ana");
    other.execute("INSERT INTO members (id, name) VALUES (100, 'From elsewhere')", []).expect("written");
    // Something under way and not committed stays out of the backup.
    other.execute_batch("BEGIN; INSERT INTO members (id, name) VALUES (101, 'Not yet');").expect("begun");
    let info = church(&database, "central", &at).save(&at.join("backup.db")).expect("saved");
    assert!(info.rows.contains(&("members".to_owned(), 2)), "{:?}", info.rows);
    other.execute_batch("ROLLBACK").expect("rolled back");
}
