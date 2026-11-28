//! An approved period keeps its document with the hash and freezes its records (#855).
#![allow(clippy::expect_used)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use ravenrs_app::{
    AttachmentStore, ClosingError, EntityInfo, FieldInfo, FieldKind, MemoryAttachmentStore, MemoryDatabase,
    ObjectSpace, PeriodClosing, Value, approve, archived_document, closing_digest, closing_versions,
};

fn term() -> EntityInfo {
    EntityInfo::new("Term")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .period_closing(PeriodClosing::new("TermClosing").freezes("Lesson", "[term_id]").reopen_reason(10))
}

fn lesson() -> EntityInfo {
    EntityInfo::new("Lesson")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("term_id", FieldKind::Reference("Term".into())))
}

fn approved() -> (MemoryDatabase, MemoryAttachmentStore, Value, Value) {
    let database = MemoryDatabase::new(vec![term(), lesson()]).expect("a valid model");
    let files = MemoryAttachmentStore::default();
    let mut space = database.space();
    let record = space.new_object("Term").expect("the entity");
    let key = record.get("id");
    space.insert(record).expect("a term");
    space.commit().expect("kept");
    let version = approve(&mut space, &files, "Term", &key, b"o documento final", "ana").expect("approved");
    (database, files, key, version.get("id"))
}

#[test]
fn the_versions_entity_comes_with_the_model() {
    let (database, _, key, _) = approved();
    let space = database.space();
    let versions = closing_versions(&space, "Term", &key).expect("read");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].text("digest"), closing_digest(b"o documento final"));
    assert_eq!(versions[0].text("status"), "current");
}

/// Files anyone can change underneath, as a disk or a bucket can.
#[derive(Default)]
struct Disk(RefCell<BTreeMap<String, Vec<u8>>>);

impl AttachmentStore for Disk {
    fn put(&self, name: &str, bytes: &[u8]) -> Result<String, String> {
        self.0.borrow_mut().insert(name.to_owned(), bytes.to_vec());
        Ok(name.to_owned())
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.borrow().get(key).cloned())
    }
    fn remove(&self, key: &str) -> Result<(), String> {
        self.0.borrow_mut().remove(key);
        Ok(())
    }
}

#[test]
fn the_document_is_given_only_while_it_matches_its_hash() {
    let database = MemoryDatabase::new(vec![term(), lesson()]).expect("a valid model");
    let disk = Disk::default();
    let mut space = database.space();
    let mut record = space.new_object("Term").expect("the entity");
    let key = record.get("id");
    record.set("id", key.clone());
    space.insert(record).expect("a term");
    space.commit().expect("kept");
    let version = approve(&mut space, &disk, "Term", &key, b"o documento final", "ana").expect("approved");
    let id = version.get("id");
    assert_eq!(archived_document(&space, &disk, "TermClosing", &id).expect("given"), b"o documento final");

    let document = version.text("document");
    disk.0.borrow_mut().get_mut(&document).expect("kept")[0] ^= 1;
    assert_eq!(archived_document(&space, &disk, "TermClosing", &id), Err(ClosingError::Tampered));
}

#[test]
fn a_path_that_does_not_reach_the_period_is_refused() {
    let wrong = EntityInfo::new("Term")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .period_closing(PeriodClosing::new("TermClosing").freezes("Lesson", "[id]"));
    assert!(MemoryDatabase::new(vec![wrong, lesson()]).is_err());
    let unknown = EntityInfo::new("Term")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .period_closing(PeriodClosing::new("TermClosing").requires_none("Lesson", "[x] = 1"));
    assert!(unknown.check().is_err(), "a precondition over an entity it does not freeze");
}
