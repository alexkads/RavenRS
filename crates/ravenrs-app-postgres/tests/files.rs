//! Files kept in PostgreSQL, as `BYTEA`, and an approved document read back against its hash
//! (#855), when `RAVENRS_POSTGRES_URL` says where a database is.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod support;

use ravenrs_app::{
    AttachmentStore, ClosingError, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace,
    PeriodClosing, approve, archived_document,
};
use ravenrs_app_postgres::{PostgresAttachmentStore, PostgresBackend};

fn store(url: &str, schema: &str) -> PostgresAttachmentStore {
    let mut admin = support::raw(url);
    admin
        .batch_execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}"))
        .expect("a schema");
    let backend = PostgresBackend::connect(url).expect("connects");
    ravenrs_app_sql::SqlBackend::batch(&backend, &format!("SET search_path TO {schema}"))
        .expect("the schema");
    PostgresAttachmentStore::new(backend).expect("the table")
}

#[test]
fn a_file_is_kept_read_and_removed_as_bytes() {
    let Ok(url) = std::env::var("RAVENRS_POSTGRES_URL") else { return };
    let files = store(&url, "files_kept");
    let bytes: Vec<u8> = (0..=255).collect();
    let key = files.put("todos-os-bytes.bin", &bytes).expect("kept");
    assert_eq!(files.get(&key).expect("read"), Some(bytes));
    files.remove(&key).expect("removed");
    assert_eq!(files.get(&key).expect("read"), None);
    assert_eq!(files.get("not a key").expect("read"), None);
}

#[test]
fn an_approved_document_kept_in_postgres_is_not_given_once_changed() {
    let Ok(url) = std::env::var("RAVENRS_POSTGRES_URL") else { return };
    let files = store(&url, "files_closing");
    let term = EntityInfo::new("Term")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .period_closing(PeriodClosing::new("TermClosing").reopen_reason(10));
    let database = MemoryDatabase::new(vec![term]).expect("a valid model");
    let mut space = database.space();
    let record = space.new_object("Term").expect("the entity");
    let key = record.get("id");
    space.insert(record).expect("a term");
    space.commit().expect("kept");
    let version =
        approve(&mut space, &files, "Term", &key, b"%PDF o documento final", "ana").expect("approved");
    let id = version.get("id");
    assert_eq!(
        archived_document(&space, &files, "TermClosing", &id).expect("given"),
        b"%PDF o documento final"
    );
    let mut admin = support::raw(&url);
    let document: i64 = version.text("document").parse().expect("a number");
    admin
        .execute(
            "UPDATE files_closing.ravenrs_app_files SET content = set_byte(content, 0, 0) WHERE id = $1",
            &[&document],
        )
        .expect("changed underneath");
    assert_eq!(archived_document(&space, &files, "TermClosing", &id), Err(ClosingError::Tampered));
}
