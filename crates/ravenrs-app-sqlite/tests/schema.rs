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

fn secured_database() -> SqliteDatabase {
    let mut entities = ravenrs_app::security_entities();
    entities.push(ravenrs_app::audit_entity());
    entities.push(account(false));
    let builder = entities.into_iter().fold(SqliteDatabase::builder(connection()), |builder, entity| {
        let table = TableMap::new(entity.name.to_lowercase());
        builder.managed(entity, table).expect("valid")
    });
    builder.build()
}

#[test]
fn users_roles_and_the_trail_live_in_the_database_with_the_records() {
    use ravenrs_app::{
        AuditMode, AuditSettings, AuditTrail, AuditedObjectSpace, Authenticator, Credentials, RoleMerge,
        SECURITY_AUDIT_ENTRY, SECURITY_USER, SecuredObjectSpace, SecurityContext, SpaceMaker, UserDirectory,
    };
    let database = secured_database();
    let source = database.clone();
    let maker: SpaceMaker = Rc::new(move || Box::new(source.space()) as Box<dyn ObjectSpace>);
    let directory = UserDirectory::new(maker, Vec::new()).expect("roles");
    directory.create_first_administrator("ana", "correct horse").expect("created");
    let identity = directory.authenticate(&Credentials::new("ana", "correct horse")).expect("signs in");
    for _ in 0..5 {
        assert!(directory.authenticate(&Credentials::new("nobody", "wrong")).is_err());
    }
    let entities = database.space().entities();
    let context = Rc::new(SecurityContext::new(identity, directory.clone(), entities, RoleMerge::AnyRole));
    let trail = Rc::new(AuditTrail::new(AuditSettings::new(AuditMode::Full), || "ana".to_owned()));
    let audited = AuditedObjectSpace::new(Box::new(database.space()), trail);
    let mut space = SecuredObjectSpace::new(Box::new(audited), context);

    let mut record = space.new_object("Account").expect("known");
    record.set("name", "Corrente");
    space.save(record.clone()).expect("saves");
    space.commit().expect("keeps");
    record.set("name", "Poupança");
    space.save(record).expect("saves");
    space.commit().expect("keeps");
    let lines = database.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads");
    assert!(lines.iter().any(|line| line.get("new_value") == Value::Text("Poupança".into())
        && line.get("old_value") == Value::Text("Corrente".into())
        && line.get("author") == Value::Text("ana".into())));
    assert!(matches!(lines[0].get("happened_at"), Value::Date(_)));

    let user = space.query(SECURITY_USER, &Query::all()).expect("reads").remove(0);
    assert_eq!(user.get("password_hash"), Value::Null);
    let stored = database.space().query(SECURITY_USER, &Query::all()).expect("reads").remove(0);
    assert!(matches!(stored.get("password_hash"), Value::Text(hash) if hash.starts_with("$argon2id$")));
    // A commit the database refuses takes its trail with it: a change and its trail stay together.
    let count = lines.len();
    let mut twin = space.new_object("Account").expect("known");
    twin.set("name", "Outra");
    space.insert(twin).expect("waits");
    space.rollback();
    assert_eq!(database.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).expect("reads").len(), count);
}

#[test]
fn a_way_that_is_not_declared_is_refused_in_the_database_too() {
    use ravenrs_app::{SpaceError, StateMachine};
    let entity = EntityInfo::new("Task")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("state", FieldKind::Text))
        .state_machine(
            StateMachine::new("state")
                .state("draft", "Draft")
                .state("done", "Done")
                .start("draft")
                .transition("draft", "done", "Finish"),
        );
    let database =
        SqliteDatabase::builder(connection()).managed(entity, TableMap::new("tasks")).expect("valid").build();
    let mut space = database.space();
    let mut other = space.new_object("Task").expect("known");
    other.set("state", "done");
    space.save(other).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))), "a new record starts in draft");
    space.rollback();
    let mut record = space.new_object("Task").expect("known");
    let key = record.get("id");
    record.set("state", "draft");
    space.save(record).expect("waits");
    space.commit().expect("keeps");
    let mut stored = space.find("Task", &key).expect("reads").expect("there");
    stored.set("state", "done");
    space.save(stored.clone()).expect("waits");
    space.commit().expect("a way");
    stored.set("state", "draft");
    space.save(stored).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))), "there is none back");
    space.rollback();
    let found = database.space().find("Task", &key).expect("reads").expect("there");
    assert_eq!(found.get("state"), Value::Text("done".into()), "and nothing changed");
}
