//! Decimals come back as they were written, rounded to their places (#337).
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{Criteria, EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, SpaceError, Value};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
use rusqlite::Connection;

const EXACT: [(f64, u8); 5] = [(0.1, 2), (5.49, 2), (16.47, 2), (123_456_789.123_456, 6), (0.000_001, 6)];

fn rates() -> EntityInfo {
    EntityInfo::new("Rate")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("two", FieldKind::Decimal(2)))
        .field(FieldInfo::new("six", FieldKind::Decimal(6)))
}

fn field(places: u8) -> &'static str {
    if places == 2 { "two" } else { "six" }
}

fn add(space: &mut impl ObjectSpace, field: &str, number: f64) {
    let mut object = space.new_object("Rate").expect("known");
    object.set(field, Value::Number(number));
    space.save(object).expect("saves");
}

fn filtered(database: &SqliteDatabase, filter: &str) -> usize {
    let query = Query::all().filter(Criteria::parse(filter).expect("a filter"));
    database.space().query("Rate", &query).expect("reads").len()
}

fn exact(connection: Rc<Connection>) {
    let database =
        SqliteDatabase::builder(connection).managed(rates(), TableMap::new("rates")).expect("valid").build();
    let mut space = database.space();
    for (number, places) in EXACT {
        add(&mut space, field(places), number);
    }
    add(&mut space, "two", 0.1 + 0.2);
    space.commit().expect("keeps");
    let found = database.space().query("Rate", &Query::all()).expect("reads");
    for (at, (number, places)) in EXACT.iter().enumerate() {
        assert_eq!(found[at].get(field(*places)), Value::Number(*number), "{number}");
    }
    assert_eq!(found[5].get("two"), Value::Number(0.3), "rounded to its places");
    assert_eq!(filtered(&database, "[two] = 16.47"), 1);
    assert_eq!(filtered(&database, "[two] = 0.3"), 1);
    assert_eq!(filtered(&database, "[two] > 5.49"), 1);
    assert_eq!(filtered(&database, "[six] = 0.000001"), 1);
}

#[test]
fn a_decimal_comes_back_exactly_and_filters_find_it_in_memory() {
    exact(Rc::new(Connection::open_in_memory().expect("a database")));
}

#[test]
fn a_decimal_comes_back_exactly_and_filters_find_it_in_a_file() {
    let folder = std::env::temp_dir().join(format!("ravenrs-decimal-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a folder");
    let path = folder.join("rates.db");
    let _ = std::fs::remove_file(&path);
    exact(Rc::new(Connection::open(&path).expect("a database")));
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_decimal_with_more_digits_than_a_number_keeps_is_refused() {
    let connection = Rc::new(Connection::open_in_memory().expect("a database"));
    let database =
        SqliteDatabase::builder(connection).managed(rates(), TableMap::new("rates")).expect("valid").build();
    let mut space = database.space();
    add(&mut space, "six", 1_234_567_890.123_456);
    assert!(matches!(space.commit(), Err(SpaceError::Refused(why)) if why.contains("six")));
    assert!(database.space().query("Rate", &Query::all()).expect("reads").is_empty());
}

#[test]
fn an_old_table_has_its_decimals_rounded_once() {
    let connection = Rc::new(Connection::open_in_memory().expect("a database"));
    connection
        .execute_batch(
            "CREATE TABLE rates (id INTEGER PRIMARY KEY, two REAL, six REAL);
             INSERT INTO rates VALUES (1, 0.30000000000000004, 0.0000014999), (2, 16.47, NULL);",
        )
        .expect("an old table");
    let open = || {
        SqliteDatabase::builder(connection.clone())
            .managed(rates(), TableMap::new("rates"))
            .expect("valid")
            .build()
    };
    let database = open();
    let found = database.space().query("Rate", &Query::all()).expect("reads");
    assert_eq!(found[0].get("two"), Value::Number(0.3));
    assert_eq!(found[0].get("six"), Value::Number(0.000_001));
    assert_eq!(found[1].get("two"), Value::Number(16.47));
    let ran = || -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM ravenrs_app_migrations", [], |row| row.get(0))
            .expect("counts")
    };
    assert_eq!(ran(), 2);
    drop(open());
    assert_eq!(ran(), 2, "once");
}
