//! Decimals come back as they were written, rounded to their places (#337).
#![allow(clippy::expect_used)]

use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, Value};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
use rusqlite::Connection;

fn rates() -> EntityInfo {
    EntityInfo::new("Rate")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("two", FieldKind::Decimal(2)))
        .field(FieldInfo::new("six", FieldKind::Decimal(6)))
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
