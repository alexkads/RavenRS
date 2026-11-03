//! Databases that change, and databases that came first.
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{
    AttachmentStore, Diff, EntityInfo, FieldInfo, FieldKind, ModelStore, ObjectSpace, Query, Value,
};
use ravenrs_app_sqlite::{
    SqliteAttachmentStore, SqliteDatabase, SqliteModelStore, TableMap, entities_from_schema,
};
use rusqlite::Connection;

fn connection() -> Rc<Connection> {
    Rc::new(Connection::open_in_memory().expect("a database"))
}

fn account(extra: bool) -> EntityInfo {
    let entity = EntityInfo::new("Account")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text));
    if extra { entity.field(FieldInfo::new("notes", FieldKind::Text)) } else { entity }
}

#[test]
fn a_field_added_later_becomes_a_column_and_what_was_there_stays() {
    let connection = connection();
    let first = SqliteDatabase::builder(connection.clone())
        .managed(account(false), TableMap::new("accounts"))
        .expect("valid")
        .build();
    let mut space = first.space();
    let mut object = space.new_object("Account").expect("known");
    object.set("name", "Corrente");
    space.save(object).expect("saves");
    space.commit().expect("keeps");

    let second = SqliteDatabase::builder(connection)
        .managed(account(true), TableMap::new("accounts"))
        .expect("the same table takes the new field")
        .build();
    let found = second.space().query("Account", &Query::all()).expect("reads");
    assert_eq!(found[0].get("name"), Value::Text("Corrente".into()));
    assert_eq!(found[0].get("notes"), Value::Null);
}

#[test]
fn a_migration_runs_once_and_a_failure_changes_nothing() {
    let connection = connection();
    let add = "CREATE TABLE a (id INTEGER PRIMARY KEY); INSERT INTO a (id) VALUES (1);";
    let builder = SqliteDatabase::builder(connection.clone()).migration("first", add).expect("runs");
    // It would fail on the table that exists, if it ran again.
    let builder = builder.migration("first", add).expect("does not run again");
    let bad = "INSERT INTO a (id) VALUES (2); INSERT INTO nowhere (id) VALUES (3);";
    assert!(builder.migration("bad", bad).is_err_and(|e| e.0.contains("bad")));
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM a", [], |row| row.get(0)).expect("counts");
    assert_eq!(count, 1, "what failed left nothing behind");
}

#[test]
fn entities_come_from_the_tables_of_a_database_that_came_first() {
    let connection = connection();
    connection
        .execute_batch(
            "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, born DATE, active BOOLEAN, photo BLOB);
             CREATE TABLE order_items (
                 id INTEGER PRIMARY KEY, price REAL, customer_id INTEGER REFERENCES customers (id)
             );
             CREATE TABLE pairs (a INTEGER, b INTEGER, PRIMARY KEY (a, b));",
        )
        .expect("a schema");
    let found = entities_from_schema(&connection).expect("reads");
    let names: Vec<&str> = found.iter().map(|(entity, _)| entity.name.as_str()).collect();
    assert_eq!(names, ["Customers", "OrderItems"], "a table with no single key stays out");
    let customers = &found[0].0;
    assert!(customers.key_field().is_some_and(|key| key.name == "id"));
    assert_eq!(customers.default_property().map(|f| f.name.as_str()), Some("name"));
    assert_eq!(customers.field_named("born").map(|f| &f.kind), Some(&FieldKind::Date));
    assert_eq!(customers.field_named("active").map(|f| &f.kind), Some(&FieldKind::Boolean));
    assert!(customers.field_named("photo").is_none());
    assert_eq!(customers.rules.len(), 1, "a column that asks for a value");
    let items = &found[1].0;
    assert_eq!(
        items.field_named("customer_id").map(|f| &f.kind),
        Some(&FieldKind::Reference("Customers".into()))
    );

    let database = found
        .into_iter()
        .fold(SqliteDatabase::builder(connection), |builder, (entity, map)| {
            builder.mapped(entity, map).expect("valid")
        })
        .build();
    assert!(database.space().query("OrderItems", &Query::all()).expect("reads").is_empty());
}

#[test]
fn settings_of_the_model_are_kept_in_the_database() {
    let connection = connection();
    let store = SqliteModelStore::new(connection.clone(), "ana").expect("a store");
    assert_eq!(store.load().expect("reads"), None);
    let difference = Diff::parse(r#"{"Views":{"A":{"Caption":"B"}}}"#).expect("parses");
    store.save(&difference).expect("keeps");
    store.save(&difference).expect("keeps over itself");
    assert_eq!(store.load().expect("reads"), Some(difference));
    let other = SqliteModelStore::new(connection, "bia").expect("a store");
    assert_eq!(other.load().expect("reads"), None, "each under its own name");
}

#[test]
fn the_content_of_files_is_kept_apart() {
    let store = SqliteAttachmentStore::new(connection()).expect("a store");
    let key = store.put("nota.pdf", &[1, 2, 3]).expect("keeps");
    assert_eq!(store.get(&key).expect("reads"), Some(vec![1, 2, 3]));
    store.remove(&key).expect("forgets");
    assert_eq!(store.get(&key).expect("reads"), None);
}

#[test]
fn a_guid_key_is_made_up_and_kept_as_text() {
    let entity = EntityInfo::new("Note")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(FieldInfo::new("text", FieldKind::Text));
    let database =
        SqliteDatabase::builder(connection()).managed(entity, TableMap::new("notes")).expect("valid").build();
    let mut space = database.space();
    let (first, second) =
        (space.new_object("Note").expect("known"), space.new_object("Note").expect("known"));
    assert_ne!(first.get("id"), second.get("id"), "no two are the same");
    let key = first.get("id");
    let Value::Text(text) = &key else { panic!("text: {key:?}") };
    assert_eq!(text.len(), 36);
    space.save(first).expect("saves");
    space.commit().expect("keeps");
    let found = database.space().find("Note", &key).expect("reads");
    assert_eq!(found.map(|note| note.get("id")), Some(key));
}

#[test]
fn a_query_for_some_fields_reads_only_those_and_the_key() {
    let database = SqliteDatabase::builder(connection())
        .managed(account(true), TableMap::new("accounts"))
        .expect("valid")
        .build();
    let mut space = database.space();
    let mut object = space.new_object("Account").expect("known");
    object.set("name", "Corrente");
    object.set("notes", "long");
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    let found = database.space().query("Account", &Query::all().only(["name"])).expect("reads");
    assert_eq!(found[0].get("name"), Value::Text("Corrente".into()));
    assert_eq!(found[0].get("id"), Value::Number(1.0));
    assert_eq!(found[0].values().len(), 2, "and not the rest: {:?}", found[0].values());
}

#[test]
fn a_field_a_layer_added_gets_a_column() {
    let application =
        ravenrs_app::Application::builder("Test").entity(account(false)).build().expect("valid");
    let mut application = application;
    let difference = Diff::parse(
        r#"{ "Entities": { "Account": { "Members": { "city": { "$new": true, "Kind": "Text", "Caption": "City" } } } } }"#,
    )
    .expect("reads");
    application.layers_mut().put(&ravenrs_app::ModelLayer::Administrator, difference);
    application.refresh_model().expect("valid");
    let entity = application.entities()[0].clone();
    let database = SqliteDatabase::builder(connection())
        .managed(entity, TableMap::new("accounts"))
        .expect("valid")
        .build();
    let mut space = database.space();
    let mut object = space.new_object("Account").expect("known");
    object.set("city", "Recife");
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    let found = database.space().query("Account", &Query::all()).expect("reads");
    assert_eq!(found[0].get("city"), Value::Text("Recife".into()));
}
