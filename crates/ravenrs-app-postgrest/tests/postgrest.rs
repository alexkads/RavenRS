//! Against a real PostgREST over PostgreSQL, when the addresses are given; skipped otherwise:
//!
//! - `RAVENRS_POSTGREST_URL`: where PostgREST answers, serving the schema `api`, with `anon` as
//!   the anonymous role and `authenticator` as the one it logs in with.
//! - `RAVENRS_POSTGREST_ADMIN_URL`: the database itself, as its owner, to set the schema up
//!   (`tests/setup.sql` and [`SERVER_SQL`]).
//! - `RAVENRS_POSTGREST_TOKEN_A` and `RAVENRS_POSTGREST_TOKEN_B`: tokens for the role
//!   `authenticated`, with a `tenant` claim of `a` and `b`, signed with PostgREST's secret.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::rc::Rc;
use std::sync::Once;
use std::time::Duration;

use ravenrs_app::{
    Criteria, EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, SpaceError, SpaceMaker, Value,
};
use ravenrs_app_postgres::{PostgresBackend, PostgresDatabase, TableMap};
use ravenrs_app_postgrest::{PostgrestDatabase, ResourceMap, SERVER_SQL};
use ravenrs_app_testkit::contract::{self, Adapter, Capabilities, Store};
use ravenrs_criteria::DateTime;

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn url() -> Option<String> {
    var("RAVENRS_POSTGREST_URL")
}

fn token(tenant: &str) -> String {
    var(&format!("RAVENRS_POSTGREST_TOKEN_{}", tenant.to_uppercase())).unwrap()
}

static SET_UP: Once = Once::new();

/// Makes the schema, then waits until PostgREST has read it again.
fn set_up() {
    SET_UP.call_once(|| {
        let mut admin =
            postgres::Client::connect(&var("RAVENRS_POSTGREST_ADMIN_URL").unwrap(), postgres::NoTls).unwrap();
        admin.batch_execute(include_str!("setup.sql")).unwrap();
        admin.batch_execute("SET search_path TO api").unwrap();
        admin.batch_execute(SERVER_SQL).unwrap();
        admin
            .batch_execute(
                "REVOKE EXECUTE ON FUNCTION api.ravenrs_commit(jsonb) FROM PUBLIC; \
                 GRANT EXECUTE ON FUNCTION api.ravenrs_commit(jsonb) TO authenticated; \
                 NOTIFY pgrst, 'reload schema'",
            )
            .unwrap();
        let ready = PostgrestDatabase::builder(url().unwrap())
            .bearer(token("a"))
            .resource(note(), ResourceMap::new("notes"))
            .unwrap()
            .build();
        for _ in 0..50 {
            if ready.space().query("Note", &Query::all()).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        panic!("PostgREST did not read the schema");
    });
}

fn plain() -> EntityInfo {
    EntityInfo::new("Plain")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome"))
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("rate", FieldKind::Decimal(2)))
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("day", FieldKind::Date))
        .field(FieldInfo::new("twice", FieldKind::Integer).calculated("[seats] * 2"))
}

fn database(resource: &str, tenant: Option<&str>) -> PostgrestDatabase {
    let mut builder = PostgrestDatabase::builder(url().unwrap()).page_size(2);
    if let Some(tenant) = tenant {
        builder = builder.bearer(token(tenant));
    }
    builder.resource(plain(), ResourceMap::new(resource).column("active", "is_active")).unwrap().build()
}

fn add(space: &mut impl ObjectSpace, values: &[(&str, Value)]) -> Value {
    let mut object = space.new_object("Plain").unwrap();
    for (field, value) in values {
        object.set(field, value.clone());
    }
    let key = object.get("id");
    space.save(object).unwrap();
    key
}

fn names(database: &PostgrestDatabase, query: &Query) -> Vec<String> {
    database
        .space()
        .query("Plain", query)
        .unwrap()
        .iter()
        .map(|object| match object.get("name") {
            Value::Text(text) => text,
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn records_are_kept_found_filtered_sorted_and_paged_on_the_server() {
    let Some(_) = url() else { return };
    set_up();
    let database = database("plain_q", Some("a"));
    let day = DateTime::from_ymd(2026, 11, 13).unwrap();
    let mut space = database.space();
    for (name, seats, rate, active) in [
        ("Alfa", 1.0, 0.5, true),
        ("Beta", 2.0, 2.5, false),
        ("Gama", 3.0, 3.25, true),
        ("Delta, (4)", 4.0, 4.0, false),
    ] {
        add(
            &mut space,
            &[
                ("name", name.into()),
                ("seats", Value::Number(seats)),
                ("rate", Value::Number(rate)),
                ("active", Value::Bool(active)),
                ("day", Value::Date(day)),
            ],
        );
    }
    // What waits for a commit is read too.
    assert_eq!(space.query("Plain", &Query::all()).unwrap().len(), 4);
    space.commit().unwrap();

    let parsed = |text: &str| Query::all().filter(Criteria::parse(text).unwrap());
    assert_eq!(names(&database, &parsed("[seats] > 1")), ["Beta", "Gama", "Delta, (4)"]);
    assert_eq!(names(&database, &parsed("[rate] = 3.25")), ["Gama"]);
    assert_eq!(names(&database, &parsed("[active] = True")), ["Alfa", "Gama"]);
    assert_eq!(names(&database, &parsed("[active]")), ["Alfa", "Gama"]);
    assert_eq!(names(&database, &parsed("[name] Like 'B%'")), ["Beta"]);
    assert_eq!(names(&database, &parsed("[name] = 'Delta, (4)'")), ["Delta, (4)"], "grammar inside text");
    assert_eq!(names(&database, &parsed("[name] In ('Alfa', 'Gama')")), ["Alfa", "Gama"]);
    assert_eq!(names(&database, &parsed("[seats] Between(2, 3)")), ["Beta", "Gama"]);
    assert_eq!(names(&database, &parsed("[seats] = 1 Or Not [active]")), ["Alfa", "Beta", "Delta, (4)"]);
    assert_eq!(names(&database, &parsed("StartsWith([name], 'Ga')")), ["Gama"]);
    assert_eq!(names(&database, &parsed("[twice] >= 6")), ["Gama", "Delta, (4)"], "calculated, in memory");
    assert_eq!(names(&database, &Query::all().sort("seats", false)), ["Delta, (4)", "Gama", "Beta", "Alfa"]);
    assert_eq!(names(&database, &Query::all().page(1, 2)), ["Beta", "Gama"]);
    assert_eq!(names(&database, &Query::all().page(3, 10)), ["Delta, (4)"]);
    // Pages of 1000 against a server that cuts at 3 (PGRST_DB_MAX_ROWS): nothing is lost.
    let wide = PostgrestDatabase::builder(url().unwrap())
        .bearer(token("a"))
        .resource(plain(), ResourceMap::new("plain_q").column("active", "is_active"))
        .unwrap()
        .build();
    assert_eq!(names(&wide, &Query::all()).len(), 4);
    assert_eq!(names(&wide, &Query::all().page(0, 4)).len(), 4);

    let found = database.space().find("Plain", &Value::Number(3.0)).unwrap().unwrap();
    assert_eq!(found.get("day"), Value::Date(day));
    assert_eq!(found.get("rate"), Value::Number(3.25));
    assert_eq!(found.get("active"), Value::Bool(true));

    let mut space = database.space();
    let mut changed = found.clone();
    changed.set("seats", Value::Number(30.0));
    space.save(changed).unwrap();
    space.delete("Plain", &Value::Number(1.0)).unwrap();
    space.commit().unwrap();
    assert_eq!(names(&database, &Query::all()), ["Beta", "Gama", "Delta, (4)"]);
    let gama = database.space().find("Plain", &Value::Number(3.0)).unwrap().unwrap();
    assert_eq!(gama.get("twice"), Value::Number(60.0));
}

#[test]
fn a_commit_keeps_all_or_nothing_and_says_what_was_refused() {
    let Some(_) = url() else { return };
    set_up();
    let database = database("plain_u", Some("a"));
    let mut space = database.space();
    add(&mut space, &[("name", "Um".into())]);
    space.commit().unwrap();

    let mut space = database.space();
    add(&mut space, &[("name", "Dois".into())]);
    add(&mut space, &[("name", "Um".into())]);
    assert_eq!(
        space.commit().unwrap_err(),
        SpaceError::Refused("There is already a record with this Nome".into())
    );
    assert_eq!(names(&database, &Query::all()), ["Um"], "nothing of it stayed");

    // A record another space took away is a conflict, and the rest of the commit does not stay.
    let mut first = database.space();
    let mut stale = first.find("Plain", &Value::Number(1.0)).unwrap().unwrap();
    stale.set("seats", Value::Number(9.0));
    first.save(stale).unwrap();
    add(&mut first, &[("name", "Tres".into())]);
    let mut second = database.space();
    second.delete("Plain", &Value::Number(1.0)).unwrap();
    second.commit().unwrap();
    assert!(matches!(first.commit(), Err(SpaceError::Conflict(_))), "the record is gone");
    assert!(names(&database, &Query::all()).is_empty(), "the new one did not stay either");
}

fn note() -> EntityInfo {
    EntityInfo::new("Note")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("title", FieldKind::Text))
}

fn notes(tenant: &str) -> PostgrestDatabase {
    PostgrestDatabase::builder(url().unwrap())
        .bearer(token(tenant))
        .resource(note(), ResourceMap::new("notes"))
        .unwrap()
        .build()
}

fn titles(database: &PostgrestDatabase) -> Vec<String> {
    database
        .space()
        .query("Note", &Query::all())
        .unwrap()
        .iter()
        .map(|object| match object.get("title") {
            Value::Text(text) => text,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn write_note(database: &PostgrestDatabase, id: f64, title: &str) -> Result<(), SpaceError> {
    let mut space = database.space();
    let mut object = space.new_object("Note").unwrap();
    object.set("id", Value::Number(id));
    object.set("title", title);
    space.save(object)?;
    space.commit()
}

#[test]
fn each_tenant_sees_and_changes_only_its_own_records() {
    let Some(_) = url() else { return };
    set_up();
    let (a, b) = (notes("a"), notes("b"));
    write_note(&a, 100.0, "de A").unwrap();
    write_note(&b, 200.0, "de B").unwrap();
    assert_eq!(titles(&a), ["de A"]);
    assert_eq!(titles(&b), ["de B"]);
    assert!(
        a.space().find("Note", &Value::Number(200.0)).unwrap().is_none(),
        "B's record is not there for A"
    );

    // A changing B's record by its key: the server finds nothing of A's to change.
    let mut space = a.space();
    let mut forged = space.new_object("Note").unwrap();
    forged.set("id", Value::Number(200.0));
    forged.set("title", "tomado por A");
    space.insert(forged).unwrap_or_else(|error| panic!("{error:?}"));
    let refused = space.commit().unwrap_err();
    assert!(
        matches!(refused, SpaceError::Refused(_)),
        "the key is taken, by a record A cannot see: {refused:?}"
    );
    assert_eq!(titles(&b), ["de B"], "B's record is as it was");

    // Without a token: the anonymous role has no grant on the table.
    let anonymous = PostgrestDatabase::builder(url().unwrap())
        .resource(note(), ResourceMap::new("notes"))
        .unwrap()
        .build();
    assert!(matches!(anonymous.space().query("Note", &Query::all()), Err(SpaceError::Denied(_))));
}

#[test]
fn a_tenant_written_by_hand_is_refused_by_the_server() {
    let Some(_) = url() else { return };
    set_up();
    let entity = note().field(FieldInfo::new("tenant", FieldKind::Text));
    let a = PostgrestDatabase::builder(url().unwrap())
        .bearer(token("a"))
        .resource(entity, ResourceMap::new("notes"))
        .unwrap()
        .build();
    let mut space = a.space();
    let mut object = space.new_object("Note").unwrap();
    object.set("id", Value::Number(300.0));
    object.set("title", "forjada");
    object.set("tenant", "b");
    space.save(object).unwrap();
    let refused = space.commit().unwrap_err();
    assert!(matches!(refused, SpaceError::Denied(_) | SpaceError::Refused(_)), "{refused:?}");
    assert!(notes("b").space().find("Note", &Value::Number(300.0)).unwrap().is_none());
}

/// Tables for a case of the contract, made on the server as an administrator would, in the schema
/// PostgREST serves, each named after the case and the entity.
struct Postgrest;

impl Postgrest {
    fn maker(entities: &[EntityInfo], case: &str) -> Result<SpaceMaker, String> {
        let mut builder = PostgrestDatabase::builder(url().unwrap()).bearer(token("a")).page_size(2);
        for entity in entities {
            let table = format!("{case}_{}", entity.name.to_lowercase());
            builder = builder.resource(entity.clone(), ResourceMap::new(table)).map_err(|e| e.to_string())?;
        }
        let database = builder.build();
        Ok(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>))
    }
}

impl Adapter for Postgrest {
    fn name(&self) -> &str {
        "ravenrs-app-postgrest"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::POSTGREST
    }

    fn open(&self, case: &str, entities: &[EntityInfo]) -> Result<Store, String> {
        let admin = || -> Result<postgres::Client, String> {
            let mut client =
                postgres::Client::connect(&var("RAVENRS_POSTGREST_ADMIN_URL").unwrap(), postgres::NoTls)
                    .map_err(|e| e.to_string())?;
            client.batch_execute("SET search_path TO api").map_err(|e| e.to_string())?;
            Ok(client)
        };
        let mut client = admin()?;
        for entity in entities {
            let table = format!("{case}_{}", entity.name.to_lowercase());
            client.batch_execute(&format!("DROP TABLE IF EXISTS api.{table}")).map_err(|e| e.to_string())?;
        }
        let mut builder = PostgresDatabase::builder(PostgresBackend::from(client));
        for entity in entities {
            let table = format!("{case}_{}", entity.name.to_lowercase());
            builder = builder.managed(entity.clone(), TableMap::new(table)).map_err(|e| e.to_string())?;
        }
        drop(builder.build());
        admin()?
            .batch_execute(
                "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA api TO authenticated; \
                 NOTIFY pgrst, 'reload schema'",
            )
            .map_err(|e| e.to_string())?;
        let space = Self::maker(entities, case)?;
        let first = entities.first().map(|entity| entity.name.clone()).unwrap_or_default();
        for _ in 0..50 {
            if space().query(&first, &Query::all()).is_ok() {
                return Ok(Store { space, again: Self::maker(entities, case)? });
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(format!("PostgREST did not read the tables of {case}"))
    }
}

#[test]
fn postgrest_keeps_the_contract() {
    let Some(_) = url() else { return };
    set_up();
    contract::run(&Postgrest);
}
