//! Records kept in a storage between runs, and moved in and out as JSON.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::rc::Rc;

use ravenrs_app::{
    EntityInfo, FieldInfo, FieldKind, MemoryDatabase, MemoryStorage, ModelStore, ObjectSpace, Query,
    SpaceError, Storage, StoredModel, Value,
};
use ravenrs_criteria::DateTime;

fn entities() -> Vec<EntityInfo> {
    vec![
        EntityInfo::new("Bank")
            .field(FieldInfo::new("code", FieldKind::Text).typed_key())
            .field(FieldInfo::new("name", FieldKind::Text)),
        EntityInfo::new("Account")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text))
            .field(FieldInfo::new("bank", FieldKind::Reference("Bank".into())))
            .field(FieldInfo::new("opened", FieldKind::Date))
            .field(FieldInfo::new("cents", FieldKind::Money))
            .field(FieldInfo::new("closed", FieldKind::Boolean))
            .field(
                FieldInfo::new("entries", FieldKind::Collection("Entry".into())).back_reference("account"),
            ),
        EntityInfo::new("Entry")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("account", FieldKind::Reference("Account".into())))
            .field(FieldInfo::new("note", FieldKind::Text)),
    ]
}

fn opened(storage: &Rc<MemoryStorage>) -> MemoryDatabase {
    MemoryDatabase::new(entities()).unwrap().stored_in(storage.clone(), "records").unwrap()
}

fn fill(database: &MemoryDatabase) -> Value {
    let mut space = database.space();
    let mut bank = space.new_object("Bank").unwrap();
    bank.set("code", "341");
    bank.set("name", "Itaú");
    space.save(bank).unwrap();
    let mut account = space.new_object("Account").unwrap();
    let key = account.get("id");
    account.set("name", "Corrente");
    account.set("bank", "341");
    account.set("opened", Value::Date(DateTime { days: 20_400, seconds: 3_600 }));
    account.set("cents", 125_050.0);
    account.set("closed", false);
    space.save(account).unwrap();
    for note in ["salário", "aluguel"] {
        let mut entry = space.new_object("Entry").unwrap();
        entry.set("account", key.clone());
        entry.set("note", note);
        space.save(entry).unwrap();
    }
    space.commit().unwrap();
    key
}

#[test]
fn records_come_back_when_it_opens_again() {
    let storage = Rc::new(MemoryStorage::default());
    let first = opened(&storage);
    assert!(!first.was_restored(), "nothing was kept yet");
    let key = fill(&first);

    let again = opened(&storage);
    assert!(again.was_restored());
    let space = again.space();
    let account = space.find("Account", &key).unwrap().expect("the account is back");
    assert_eq!(account.get("name"), Value::from("Corrente"));
    assert_eq!(account.get("bank"), Value::from("341"), "references");
    assert_eq!(account.get("opened"), Value::Date(DateTime { days: 20_400, seconds: 3_600 }), "dates");
    assert_eq!(account.get("cents"), Value::Number(125_050.0));
    assert_eq!(account.get("closed"), Value::Bool(false));
    let entries = space.query("Entry", &Query::default()).unwrap();
    assert_eq!(entries.len(), 2, "the records of a collection");
    assert!(entries.iter().all(|e| e.get("account") == key));
    assert_eq!(space.query("Bank", &Query::default()).unwrap().len(), 1);

    // Numbering goes on after the last key, never giving one twice.
    let mut space = again.space();
    let next = space.new_object("Entry").unwrap().get("id");
    let used: Vec<Value> = entries.iter().map(|e| e.get("id")).chain([key]).collect();
    assert!(!used.contains(&next), "{next:?} was already used");
}

#[test]
fn every_commit_is_written_and_a_rollback_is_not() {
    let storage = Rc::new(MemoryStorage::default());
    let database = opened(&storage);
    assert_eq!(storage.read("records").unwrap(), None, "nothing until a commit");
    fill(&database);
    let written = storage.read("records").unwrap().expect("written");
    let mut space = database.space();
    let mut bank = space.new_object("Bank").unwrap();
    bank.set("code", "001");
    space.save(bank).unwrap();
    space.rollback();
    assert_eq!(storage.read("records").unwrap(), Some(written));
}

#[test]
fn json_goes_out_and_back_in_whole() {
    let source = MemoryDatabase::new(entities()).unwrap();
    fill(&source);
    let json = source.export_json();
    assert!(json.contains("\"format\": 1"), "{json}");

    let storage = Rc::new(MemoryStorage::default());
    let target = opened(&storage);
    assert_eq!(target.import_json(&json), Ok(4));
    assert_eq!(target.export_json(), json, "the same records");
    assert!(storage.read("records").unwrap().is_some(), "an import is kept like a commit");

    // A wrong import changes nothing.
    let wrong = r#"{"format": 1, "records": {"Nope": [{"id": 1}]}}"#;
    assert!(matches!(target.import_json(wrong), Err(SpaceError::Storage(_))));
    let keyless = r#"{"format": 1, "records": {"Bank": [{"name": "no code"}]}}"#;
    assert!(target.import_json(keyless).is_err());
    assert_eq!(target.export_json(), json);
    let newer = r#"{"format": 9, "records": {}}"#;
    assert!(target.import_json(newer).unwrap_err().to_string().contains("newer"));
}

#[test]
fn records_of_an_entity_that_left_are_set_aside_when_it_opens() {
    let storage = Rc::new(MemoryStorage::default());
    storage
        .write(
            "records",
            br#"{"format": 1, "next_key": 5, "records": {"Gone": [{"id": 1}], "Bank": [{"code": "1"}]}}"#,
        )
        .unwrap();
    let database = opened(&storage);
    assert_eq!(database.space().query("Bank", &Query::default()).unwrap().len(), 1);
}

struct Broken;

impl Storage for Broken {
    fn read(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }
    fn write(&self, _: &str, _: &[u8]) -> Result<(), String> {
        Err("disk full".into())
    }
    fn remove(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn a_failed_write_is_told_and_the_records_stay_in_memory() {
    let database = MemoryDatabase::new(entities()).unwrap().stored_in(Broken, "records").unwrap();
    assert_eq!(database.storage_problem(), None);
    fill(&database);
    assert_eq!(database.storage_problem().as_deref(), Some("disk full"));
    assert_eq!(database.space().query("Bank", &Query::default()).unwrap().len(), 1);
}

#[test]
fn settings_are_kept_beside_the_records() {
    let storage = Rc::new(MemoryStorage::default());
    let model = StoredModel::new(storage.clone(), "settings");
    assert!(model.load().unwrap().is_none());
    let difference = ravenrs_app::Diff::parse(r#"{ "Views": { "Caption": "x" } }"#).unwrap();
    model.save(&difference).unwrap();
    assert_eq!(StoredModel::new(storage, "settings").load().unwrap(), Some(difference));
}
