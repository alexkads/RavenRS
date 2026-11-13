//! Records in SQLite, on tables it made and on tables it did not.
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::*;
use ravenrs_app_sqlite::*;
use rusqlite::Connection;

fn account() -> EntityInfo {
    EntityInfo::new("Account")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome"))
        .field(FieldInfo::new("opening", FieldKind::Money))
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("opened", FieldKind::Date))
        .field(FieldInfo::new("rate", FieldKind::Decimal(2)))
        .field(FieldInfo::new("double", FieldKind::Money).calculated("[opening] * 2"))
}

fn add(space: &mut SqliteObjectSpace, name: &str, cents: f64) -> Value {
    let mut object = space.new_object("Account").expect("the entity");
    object.set("name", name);
    object.set("opening", cents);
    object.set("active", true);
    let key = object.get("id");
    space.save(object).expect("saves");
    key
}

fn existing() -> (Rc<Connection>, SqliteDatabase) {
    let connection = Rc::new(Connection::open_in_memory().expect("opens"));
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE accounts (
                 id INTEGER PRIMARY KEY,
                 full_name TEXT NOT NULL UNIQUE CHECK (length(trim(full_name)) > 0),
                 opening_cents INTEGER NOT NULL
             );
             CREATE TABLE entries (id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL REFERENCES accounts (id), cents INTEGER);
             INSERT INTO accounts VALUES (1, 'Conta', 100);
             INSERT INTO entries VALUES (1, 1, 50);",
        )
        .expect("the schema");
    let entity = EntityInfo::new("Account")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome"))
        .field(FieldInfo::new("opening", FieldKind::Money))
        .field(FieldInfo::new("balance", FieldKind::Money).read_only());
    let map = TableMap::new("accounts")
        .column("name", "full_name")
        .column("opening", "opening_cents")
        .computed(
            "balance",
            "accounts.opening_cents + COALESCE((SELECT SUM(cents) FROM entries e WHERE e.account_id = accounts.id), 0)",
        )
        .before_save(|object| {
            if let Value::Text(name) = object.get("name") {
                object.set("name", name.trim());
            }
            if object.get("opening") == Value::Null {
                object.set("opening", 0.0);
            }
        });
    let db = SqliteDatabase::builder(connection.clone()).mapped(entity, map).expect("valid").build();
    (connection, db)
}

#[test]
fn a_table_that_exists_is_read_and_written_under_other_names() {
    let (connection, db) = existing();
    let mut space = db.space();
    let found = space.find("Account", &Value::Number(1.0)).expect("reads").expect("there");
    assert_eq!(found.get("name"), Value::Text("Conta".into()));
    assert_eq!(found.get("balance"), Value::Number(150.0), "worked out by the database");
    let filter = Criteria::parse("[balance] = 150").expect("parses");
    assert_eq!(space.query("Account", &Query::all().filter(filter)).expect("reads").len(), 1);
    let mut changed = found;
    changed.set("name", "  Nova  ");
    changed.set("opening", Value::Null);
    space.save(changed).expect("saves");
    space.commit().expect("commits");
    let (name, opening): (String, i64) = connection
        .query_row("SELECT full_name, opening_cents FROM accounts WHERE id = 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("reads");
    assert_eq!((name.as_str(), opening), ("Nova", 0), "what the table asks for");
}

#[test]
fn what_the_database_refuses_is_said_in_words_and_keeps_nothing() {
    let connection = Rc::new(Connection::open_in_memory().expect("opens"));
    drop(connection);
    let (connection, db) = existing();
    let mut space = db.space();
    let mut object = space.new_object("Account").expect("the entity");
    object.set("name", "Conta");
    object.set("opening", 1.0);
    space.save(object).expect("saves");
    let Err(SpaceError::Refused(unique)) = space.commit() else { panic!("refused") };
    assert_eq!(unique, "There is already a record with this Nome");
    assert!(space.is_modified(), "it waits for a decision");
    space.rollback();

    let mut space = db.space();
    space.delete("Account", &Value::Number(1.0)).expect("marks");
    let Err(SpaceError::Refused(in_use)) = space.commit() else { panic!("refused") };
    assert!(in_use.contains("in use"), "{in_use}");
    space.rollback();

    let mut space = db.space();
    let mut good = space.new_object("Account").expect("the entity");
    good.set("name", "Boa");
    good.set("opening", 1.0);
    space.save(good).expect("saves");
    let mut bad = space.new_object("Account").expect("the entity");
    bad.set("name", "   ");
    bad.set("opening", 1.0);
    space.save(bad).expect("saves");
    let Err(SpaceError::Refused(rule)) = space.commit() else { panic!("refused") };
    assert!(rule.contains("rules"), "{rule}");
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0)).expect("counts");
    assert_eq!(count, 1, "the good one did not get in either");
}

#[test]
fn inserting_does_not_take_over_a_record() {
    let (_connection, db) = existing();
    let mut space = db.space();
    let mut object = space.new_object("Account").expect("the entity");
    object.set("id", 1.0);
    object.set("name", "Outra");
    assert!(matches!(space.insert(object), Err(SpaceError::DuplicateKey { .. })));
}

#[test]
fn a_watcher_can_refuse_and_rules_run_on_it() {
    let (_connection, db) = existing();
    let mut space = db.space();
    space.on_event(Box::new(|event| match event {
        SpaceEvent::Committing(_) => Err("not today".to_owned()),
        _ => Ok(()),
    }));
    space.delete("Account", &Value::Number(1.0)).expect("marks");
    assert_eq!(space.commit(), Err(SpaceError::Refused("not today".into())));
    let rules = account().rule(Rule::unique("name"));
    let connection = Rc::new(Connection::open_in_memory().expect("opens"));
    let db = SqliteDatabase::builder(connection)
        .managed(rules.clone(), TableMap::new("a"))
        .expect("valid")
        .build();
    let mut space = db.space();
    add(&mut space, "A", 1.0);
    space.commit().expect("commits");
    let twin = Object::from_values(
        "Account",
        [("id".to_owned(), Value::Number(99.0)), ("name".to_owned(), Value::Text("A".into()))]
            .into_iter()
            .collect(),
    );
    assert_eq!(validate(&rules, &twin, CONTEXT_SAVE, &space, &RuleWords::default()).len(), 1);
}

#[test]
fn a_database_held_by_another_connection_is_a_temporary_failure() {
    let path = std::env::temp_dir().join(format!("ravenrs-busy-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let holder = Connection::open(&path).expect("opens");
    let db = {
        let connection = Connection::open(&path).expect("opens");
        connection.busy_timeout(std::time::Duration::ZERO).expect("no waiting");
        SqliteDatabase::builder(Rc::new(connection))
            .managed(account(), TableMap::new("accounts"))
            .expect("valid")
            .build()
    };
    let mut space = db.space();
    let mut record = space.new_object("Account").expect("known");
    record.set("id", 1.0);
    record.set("name", "Cash");
    holder.execute_batch("BEGIN EXCLUSIVE").expect("holds the database");
    let reading = space.new_object("Account");
    assert!(matches!(&reading, Err(e) if e.is_temporary()), "got {reading:?}");
    let result = space.save(record).and_then(|()| space.commit());
    assert!(matches!(&result, Err(e) if e.is_temporary()), "got {result:?}");
    holder.execute_batch("ROLLBACK").expect("lets go");
    drop(holder);
    let _ = std::fs::remove_file(&path);
}
