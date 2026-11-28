//! Bills, settlements, reversals and installments, the same in memory, in SQLite and, when
//! `RAVENRS_POSTGRES_URL` says where one is, in PostgreSQL, each run in a schema of its own (#859).
#![allow(clippy::expect_used)]

use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ravenrs_app::{MemoryDatabase, Object, ObjectSpace, Query, SpaceError, SpaceEvent, Value};
use ravenrs_app_library::finance::{
    self, COMMITMENT, FINANCE_CATEGORY, LEDGER_ENTRY, SETTLEMENT, SettleInput, cancel, plan, reverse, settle,
    split,
};
use ravenrs_app_postgres::{PostgresBackend, PostgresDatabase};
use ravenrs_app_sql::SqlBackend;
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
use ravenrs_criteria::DateTime;
use rusqlite::Connection;

type Maker = Rc<dyn Fn() -> Box<dyn ObjectSpace>>;

fn memory() -> Maker {
    let database = MemoryDatabase::new(finance::entities()).expect("entities fit");
    Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
}

fn sqlite() -> Maker {
    let connection = Rc::new(Connection::open_in_memory().expect("a database"));
    let mut builder = SqliteDatabase::builder(connection);
    for entity in finance::entities() {
        let table = entity.name.to_lowercase();
        builder = builder.managed(entity, TableMap::new(table)).expect("a table");
    }
    let database = builder.build();
    Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
}

fn postgres(url: &str) -> Maker {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let schema = format!("finance_{}_{}", std::process::id(), RUNS.fetch_add(1, Ordering::Relaxed));
    let backend = PostgresBackend::connect(url).expect("connects");
    backend
        .batch(&format!(
            "DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}; SET search_path TO {schema}"
        ))
        .expect("a schema");
    let mut builder = PostgresDatabase::builder(backend);
    for entity in finance::entities() {
        let table = entity.name.to_lowercase();
        builder = builder.managed(entity, TableMap::new(table)).expect("a table");
    }
    let database = builder.build();
    Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
}

/// In memory, in SQLite and, when there is one, in PostgreSQL.
fn both(test: impl Fn(&Maker)) {
    test(&memory());
    test(&sqlite());
    if let Ok(url) = std::env::var("RAVENRS_POSTGRES_URL") {
        test(&postgres(&url));
    }
}

fn day(year: i32, month: u32, day: u32) -> DateTime {
    DateTime::from_ymd(year, month, day).expect("a date")
}

const TODAY: (i32, u32, u32) = (2026, 11, 21);

fn today() -> DateTime {
    day(TODAY.0, TODAY.1, TODAY.2)
}

fn bill(maker: &Maker, cents: f64) -> Value {
    let mut space = maker();
    let mut category = space.new_object(FINANCE_CATEGORY).expect("category");
    category.set("name", "Energia");
    category.set("kind", "expense");
    let category_key = category.get("id");
    space.insert(category).expect("kept");
    let mut bill = space.new_object(COMMITMENT).expect("bill");
    bill.set("description", "Luz");
    bill.set("kind", "payable");
    bill.set("category_id", category_key);
    bill.set("amount", cents);
    bill.set("due_on", Value::Date(today()));
    let key = bill.get("id");
    space.insert(bill).expect("kept");
    space.commit().expect("kept");
    key
}

fn read(maker: &Maker, entity: &str, key: &Value) -> Object {
    maker().find(entity, key).expect("reads").expect("there")
}

fn all(maker: &Maker, entity: &str) -> Vec<Object> {
    maker().query(entity, &Query::all()).expect("reads")
}

#[test]
fn a_settlement_with_interest_fine_and_discount_writes_the_entry_in_one_commit() {
    both(|maker| {
        let key = bill(maker, 10_000.0);
        let mut space = maker();
        let input = SettleInput::new(today(), 10_000).interest(300).fine(200).discount(100);
        settle(&mut *space, &key, input, today()).expect("settled");
        assert_eq!(read(maker, COMMITMENT, &key).text("status"), "paid");
        let entries = all(maker, LEDGER_ENTRY);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cents("amount"), Some(10_400));
        assert_eq!(entries[0].text("kind"), "expense");
        assert_eq!(all(maker, SETTLEMENT).len(), 1);
    });
}

#[test]
fn a_commit_that_fails_leaves_nothing_of_the_settlement() {
    both(|maker| {
        let key = bill(maker, 10_000.0);
        let mut space = maker();
        space.on_event(Box::new(|event| match event {
            SpaceEvent::Committing(_) => Err("falhou ao gravar o lançamento".to_owned()),
            _ => Ok(()),
        }));
        assert!(settle(&mut *space, &key, SettleInput::new(today(), 10_000), today()).is_err());
        assert_eq!(read(maker, COMMITMENT, &key).text("status"), "open");
        assert!(all(maker, LEDGER_ENTRY).is_empty() && all(maker, SETTLEMENT).is_empty());
    });
}

#[test]
fn a_partial_settlement_leaves_the_balance_and_a_second_pays_it() {
    both(|maker| {
        let key = bill(maker, 10_000.0);
        settle(&mut *maker(), &key, SettleInput::new(today(), 4_000), today()).expect("part");
        let partial = read(maker, COMMITMENT, &key);
        assert_eq!(partial.text("status"), "partial");
        assert_eq!(partial.cents("balance"), Some(6_000));
        settle(&mut *maker(), &key, SettleInput::new(today(), 6_000), today()).expect("the rest");
        assert_eq!(read(maker, COMMITMENT, &key).text("status"), "paid");
    });
}

#[test]
fn what_cannot_be_settled_is_refused_and_writes_nothing() {
    both(|maker| {
        let key = bill(maker, 10_000.0);
        let tomorrow = day(2026, 11, 22);
        let refused = |input: SettleInput| settle(&mut *maker(), &key, input, today());
        let future = refused(SettleInput::new(tomorrow, 10_000)).expect_err("refused");
        assert_eq!(future, SpaceError::Refused("the settlement date cannot be after today".into()));
        assert!(refused(SettleInput::new(today(), 1_000).discount(1_000)).is_err(), "a discount that zeroes");
        assert!(refused(SettleInput::new(today(), 10_001)).is_err(), "more than what is open");
        assert!(refused(SettleInput::new(today(), 1_000).interest(-1)).is_err(), "below zero");
        assert_eq!(read(maker, COMMITMENT, &key).text("status"), "open");
        assert!(all(maker, LEDGER_ENTRY).is_empty() && all(maker, SETTLEMENT).is_empty());
    });
}

#[test]
fn a_reversal_needs_a_reason_and_writes_the_opposite_entry_without_deleting() {
    both(|maker| {
        let key = bill(maker, 10_000.0);
        let settlement =
            settle(&mut *maker(), &key, SettleInput::new(today(), 10_000), today()).expect("paid");
        let id = settlement.get("id");
        assert!(reverse(&mut *maker(), &id, "  ", today()).is_err(), "a reversal with no reason");
        reverse(&mut *maker(), &id, "pago em duplicidade", today()).expect("reversed");
        assert_eq!(read(maker, COMMITMENT, &key).text("status"), "open");
        let entries = all(maker, LEDGER_ENTRY);
        assert_eq!(entries.len(), 2, "nothing was deleted");
        let back =
            entries.iter().find(|entry| entry.get("reverses_id") != Value::Null).expect("the reversal");
        assert_eq!(back.cents("amount"), Some(-10_000));
        assert_eq!(read(maker, SETTLEMENT, &id).text("reverse_reason"), "pago em duplicidade");
        assert!(reverse(&mut *maker(), &id, "de novo", today()).is_err(), "reversed twice");
    });
}

#[test]
fn a_paid_bill_and_an_entry_of_a_settlement_do_not_change_directly() {
    both(|maker| {
        let key = bill(maker, 10_000.0);
        settle(&mut *maker(), &key, SettleInput::new(today(), 10_000), today()).expect("paid");
        let mut space = maker();
        let mut paid = read(maker, COMMITMENT, &key);
        paid.set("amount", 5_000.0);
        let refused = space.save(paid).and_then(|()| space.commit());
        assert!(matches!(refused, Err(SpaceError::Refused(_))), "{refused:?}");
        let mut space = maker();
        let mut entry = all(maker, LEDGER_ENTRY).remove(0);
        entry.set("amount", 1.0);
        assert!(space.save(entry).and_then(|()| space.commit()).is_err(), "an entry of a settlement changed");
        let mut space = maker();
        let mut reopened = read(maker, COMMITMENT, &key);
        reopened.set("status", "open");
        assert!(
            space.save(reopened).and_then(|()| space.commit()).is_err(),
            "paid went back without a reversal"
        );
    });
}

#[test]
fn a_split_adds_up_to_the_cent_in_month_ends_and_cancelling_takes_only_open_parts() {
    let first = day(2026, 1, 31);
    let planned = plan(10_000, 3, first);
    assert_eq!(
        planned,
        vec![(day(2026, 1, 31), 3_333), (day(2026, 2, 28), 3_333), (day(2026, 3, 31), 3_334)]
    );
    both(|maker| {
        let key = bill(maker, 10_000.0);
        let parts = split(&mut *maker(), &key, 3, first).expect("split");
        let amounts: Vec<Option<i64>> = parts.iter().map(|part| part.cents("amount")).collect();
        assert_eq!(amounts, vec![Some(3_333), Some(3_333), Some(3_334)]);
        assert_eq!(read(maker, COMMITMENT, &key).text("status"), "split");
        assert!(split(&mut *maker(), &key, 2, first).is_err(), "split twice");
        let first_part = parts[0].get("id");
        settle(&mut *maker(), &first_part, SettleInput::new(today(), 3_333), today()).expect("one paid");
        cancel(&mut *maker(), &key).expect("cancelled");
        let statuses: Vec<String> = all(maker, COMMITMENT)
            .iter()
            .filter(|part| part.key("origin_id") == ravenrs_app::key_text(&key))
            .map(|part| part.text("status"))
            .collect();
        assert_eq!(statuses, vec!["paid", "cancelled", "cancelled"]);
        assert_eq!(read(maker, COMMITMENT, &key).text("status"), "cancelled");
    });
    assert!(split(&mut *memory()(), &Value::Number(1.0), 1, first).is_err(), "one installment");
}
