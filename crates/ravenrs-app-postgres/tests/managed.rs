//! A database as a hosted one offers it: TLS with an authority of its own, a pooler in transaction
//! mode in front, and connections that drop. Each test runs when its address is given, and skips
//! otherwise:
//!
//! - `RAVENRS_POSTGRES_TLS_URL`: `sslmode=require`, a certificate no public authority signed, and
//!   `RAVENRS_POSTGRES_CA` the file of the authority that did.
//! - `RAVENRS_POSTGRES_POOLER_URL`: PgBouncer in transaction mode with `max_prepared_statements = 0`,
//!   as Supavisor on 6543 is.
//!
//! Tables go in `public` under names no other test uses, dropped before each run.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use ravenrs_app::{Criteria, EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, Value};
use ravenrs_app_postgres::{
    ConnectError, PostgresBackend, PostgresDatabase, PostgresOptions, Statements, TableMap,
};
use ravenrs_app_sql::{SqlBackend, SqlCell};

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn tls() -> Option<(String, String)> {
    Some((var("RAVENRS_POSTGRES_TLS_URL")?, var("RAVENRS_POSTGRES_CA")?))
}

fn with_authority(url: &str, ca: &str) -> PostgresBackend {
    let pem = std::fs::read(ca).unwrap();
    PostgresBackend::connect_with(url, PostgresOptions::new().root_certificate_pem(pem)).unwrap()
}

fn one(backend: &PostgresBackend, sql: &str) -> SqlCell {
    backend.query(sql, &[]).unwrap().rows.remove(0).remove(0)
}

fn count(backend: &PostgresBackend, table: &str) -> i64 {
    match one(backend, &format!("SELECT count(*) FROM {table}")) {
        SqlCell::Integer(count) => count,
        other => panic!("{other:?}"),
    }
}

fn terminate(admin: &PostgresBackend, victim: &PostgresBackend) {
    let SqlCell::Integer(pid) = one(victim, "SELECT pg_backend_pid()") else { panic!("no pid") };
    admin.query("SELECT pg_terminate_backend($1::int)", &[Value::Number(pid as f64)]).unwrap();
}

#[test]
fn tls_trusts_only_an_authority_it_was_given() {
    let Some((url, ca)) = tls() else { return };
    match PostgresBackend::connect(&url) {
        Err(ConnectError::Connect(_)) => {}
        Err(other) => panic!("refused for another reason: {other}"),
        Ok(_) => panic!("a certificate nobody vouched for was taken"),
    }

    let told = with_authority(&url, &ca);
    assert_eq!(one(&told, "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()"), SqlCell::Bool(true));

    let separator = if url.contains('?') { '&' } else { '?' };
    let in_address = PostgresBackend::connect(&format!("{url}{separator}sslrootcert={ca}")).unwrap();
    assert_eq!(one(&in_address, "SELECT 1"), SqlCell::Integer(1));

    let text = format!("{told:?}");
    assert!(
        !text.contains("password") && !url.split('@').next().is_some_and(|login| text.contains(login)),
        "{text}"
    );
}

#[test]
fn a_connection_that_dropped_is_opened_again_but_a_transaction_on_it_is_lost_whole() {
    let Some((url, ca)) = tls() else { return };
    let admin = with_authority(&url, &ca);
    let backend = with_authority(&url, &ca);
    backend
        .batch("DROP TABLE IF EXISTS dropped_rows; CREATE TABLE dropped_rows (id BIGINT PRIMARY KEY)")
        .unwrap();

    // Outside a transaction: the next read finds a new connection.
    terminate(&admin, &backend);
    assert_eq!(count(&backend, "dropped_rows"), 0);

    // Inside one: what was sent is not sent again, and nothing of it stays.
    backend.batch("BEGIN").unwrap();
    backend.execute("INSERT INTO dropped_rows VALUES ($1)", &[Value::Number(1.0)]).unwrap();
    terminate(&admin, &backend);
    assert!(backend.execute("INSERT INTO dropped_rows VALUES ($1)", &[Value::Number(2.0)]).is_err());
    assert!(backend.batch("COMMIT").is_err());
    assert_eq!(count(&backend, "dropped_rows"), 0, "nothing of the lost transaction");

    // And BEGIN on a connection that dropped meanwhile goes on a new one.
    terminate(&admin, &backend);
    backend.batch("BEGIN").unwrap();
    backend.execute("INSERT INTO dropped_rows VALUES ($1)", &[Value::Number(3.0)]).unwrap();
    backend.batch("COMMIT").unwrap();
    assert_eq!(count(&backend, "dropped_rows"), 1);
    backend.batch("DROP TABLE dropped_rows").unwrap();
}

fn plain() -> EntityInfo {
    EntityInfo::new("Plain")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome"))
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("rate", FieldKind::Decimal(2)))
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("day", FieldKind::Date))
}

/// Two clients through the pooler. One holds a transaction, so the other's next statement lands
/// on a server connection where what it prepared before does not exist.
fn after_a_switch(mode: Statements) -> Result<(), String> {
    let url = var("RAVENRS_POSTGRES_POOLER_URL").unwrap();
    let options = || PostgresOptions::new().statements(mode);
    let first = PostgresBackend::connect_with(&url, options()).unwrap();
    let other = PostgresBackend::connect_with(&url, options()).unwrap();
    let sql = "SELECT $1::bigint + 1";
    first.query(sql, &[Value::Number(1.0)]).map_err(|error| format!("{error:?}"))?;
    other.batch("BEGIN").unwrap();
    other.query("SELECT 1", &[]).unwrap();
    let done = first.query(sql, &[Value::Number(2.0)]).map_err(|error| format!("{error:?}"));
    other.batch("ROLLBACK").unwrap();
    done.map(|_| ())
}

/// One test, so nothing else on the pooler moves the connections while the switch is set up.
#[test]
fn behind_a_pooler_statements_go_unprepared_and_records_are_kept_whole() {
    let Some(url) = var("RAVENRS_POSTGRES_POOLER_URL") else { return };
    assert!(
        after_a_switch(Statements::Prepared).is_err(),
        "the pooler should have lost the prepared statement"
    );
    after_a_switch(Statements::Unprepared).unwrap();

    let backend = PostgresBackend::connect(&url).unwrap();
    assert_eq!(backend.statement_mode(), Statements::Unprepared, "pgbouncer=true in the address");
    backend.batch("DROP TABLE IF EXISTS pooled_plain").unwrap();
    let database =
        PostgresDatabase::builder(backend).managed(plain(), TableMap::new("pooled_plain")).unwrap().build();
    // Others on the pooler, so statements move between server connections.
    let others: Vec<PostgresBackend> = (0..3).map(|_| PostgresBackend::connect(&url).unwrap()).collect();

    let day = ravenrs_criteria::DateTime::from_ymd(2026, 11, 13).unwrap();
    let mut space = database.space();
    for (name, seats, rate, active) in
        [("Alfa", 1.0, 0.5, true), ("Beta", 2.0, 2.5, false), ("Gama", 3.0, 3.25, true)]
    {
        let mut object = space.new_object("Plain").unwrap();
        object.set("name", name);
        object.set("seats", Value::Number(seats));
        object.set("rate", Value::Number(rate));
        object.set("active", Value::Bool(active));
        object.set("day", Value::Date(day));
        space.save(object).unwrap();
        for other in &others {
            other.query("SELECT 1", &[]).unwrap();
        }
    }
    space.commit().unwrap();

    let names = |text: &str| -> Vec<String> {
        let query = Query::all().filter(Criteria::parse(text).unwrap()).sort("seats", true);
        database
            .space()
            .query("Plain", &query)
            .unwrap()
            .iter()
            .map(|object| match object.get("name") {
                Value::Text(text) => text,
                other => panic!("{other:?}"),
            })
            .collect()
    };
    assert_eq!(names("[seats] > 1"), ["Beta", "Gama"]);
    assert_eq!(names("[rate] = 3.25"), ["Gama"]);
    assert_eq!(names("[active] = True"), ["Alfa", "Gama"]);
    assert_eq!(names("[name] Like 'B%'"), ["Beta"]);
    assert_eq!(names("[name] In ('Alfa', 'Gama')"), ["Alfa", "Gama"]);
    let found = database
        .space()
        .query("Plain", &Query::all().filter(Criteria::parse("[name] = 'Gama'").unwrap()))
        .unwrap();
    assert_eq!(found[0].get("day"), Value::Date(day));
    assert_eq!(found[0].get("rate"), Value::Number(3.25));

    // A commit that fails halfway leaves nothing behind, through the pooler too.
    let mut space = database.space();
    let mut fine = space.new_object("Plain").unwrap();
    fine.set("name", "Delta");
    space.save(fine).unwrap();
    let mut wrong = space.new_object("Plain").unwrap();
    wrong.set("name", "Epsilon");
    wrong.set("seats", "not a number");
    space.save(wrong).unwrap();
    assert!(space.commit().is_err());
    assert_eq!(database.space().query("Plain", &Query::all()).unwrap().len(), 3, "nothing of it stayed");
}
