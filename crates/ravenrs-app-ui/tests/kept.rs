//! An application whose records and settings stay between runs.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, MemoryStorage, Storage};
use ravenrs_app_ui::App;

fn bank() -> EntityInfo {
    EntityInfo::new("Bank")
        .caption("Bancos")
        .field(FieldInfo::new("code", FieldKind::Text).typed_key())
        .field(FieldInfo::new("name", FieldKind::Text))
}

fn open(storage: &Rc<MemoryStorage>) {
    App::new("Bancos")
        .entity_info(bank())
        .seed(|space| {
            let mut first = space.new_object("Bank").map_err(|e| e.to_string())?;
            first.set("code", "341");
            first.set("name", "Itaú");
            space.save(first).map_err(|e| e.to_string())
        })
        .keep_in(storage.clone())
        .build()
        .expect("it opens");
}

fn banks(storage: &MemoryStorage) -> usize {
    let bytes = storage.read("records").unwrap().expect("records were written");
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["records"]["Bank"].as_array().map_or(0, Vec::len)
}

#[test]
fn the_seed_runs_only_while_nothing_was_kept() {
    let storage = Rc::new(MemoryStorage::default());
    open(&storage);
    assert_eq!(banks(&storage), 1, "the seed is kept");
    open(&storage);
    open(&storage);
    assert_eq!(banks(&storage), 1, "and not put in again on the next runs");
}
