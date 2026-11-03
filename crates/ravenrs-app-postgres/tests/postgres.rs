//! Against a real PostgreSQL, when `RAVENRS_POSTGRES_URL` says where one is. Each test works in a
//! schema of its own, and leaves nothing. With no database, there is nothing to run them against.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::rc::Rc;

use postgres::{Client, NoTls};
use ravenrs_app::{
    Access, Criteria, EntityInfo, EnumOption, FieldInfo, FieldKind, ObjectSpace, Operation, PermissionPolicy,
    Query, Role, Rule, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SpaceError, SpaceMaker,
    StateMachine, UserDirectory, Value, hash_password, security_entities,
};
use ravenrs_app_postgres::{PostgresDatabase, SqlWords, TableMap};
use ravenrs_app_server::{ApiRequest, ApiService};
use ravenrs_criteria::DateTime;
use serde_json::json;

fn url() -> Option<String> {
    std::env::var("RAVENRS_POSTGRES_URL").ok()
}

/// A connection whose tables go in a schema nobody else uses, dropped at the start of the next run.
fn client(schema: &str) -> Client {
    let mut admin = Client::connect(&url().unwrap(), NoTls).unwrap();
    admin.batch_execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}")).unwrap();
    let mut client = Client::connect(&url().unwrap(), NoTls).unwrap();
    client.batch_execute(&format!("SET search_path TO {schema}")).unwrap();
    client
}

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("rate", FieldKind::Decimal(2)))
        .field(FieldInfo::new("cost", FieldKind::Money))
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("day", FieldKind::Date))
        .field(FieldInfo::new("twice", FieldKind::Integer).calculated("[seats] * 2"))
        .field(FieldInfo::new(
            "state",
            FieldKind::Enum(vec![EnumOption::new("o", "Open"), EnumOption::new("c", "Closed")]),
        ))
        .rule(Rule::required("name"))
        .state_machine(
            StateMachine::new("state")
                .state("o", "Open")
                .state("c", "Closed")
                .start("o")
                .transition("o", "c", "Close"),
        )
}

fn simple() -> EntityInfo {
    EntityInfo::new("Plain")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome"))
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("rate", FieldKind::Decimal(2)))
        .field(FieldInfo::new("active", FieldKind::Boolean))
}

fn add(space: &mut impl ObjectSpace, entity: &str, values: &[(&str, Value)]) -> Value {
    let mut object = space.new_object(entity).unwrap();
    for (field, value) in values {
        object.set(field, value.clone());
    }
    let key = object.get("id");
    space.save(object).unwrap();
    key
}

#[test]
fn what_is_kept_comes_back_as_it_was() {
    if url().is_none() {
        return;
    }
    let database =
        PostgresDatabase::builder(client("kept")).managed(order(), TableMap::new("orders")).unwrap().build();
    let day = DateTime::parse("2026-11-03").unwrap();
    let mut space = database.space();
    let key = add(
        &mut space,
        "Order",
        &[
            ("name", "Um".into()),
            ("seats", Value::Number(3.0)),
            ("rate", Value::Number(1.5)),
            ("cost", Value::Number(12_345.0)),
            ("active", Value::Bool(true)),
            ("day", Value::Date(day)),
            ("state", "o".into()),
        ],
    );
    space.commit().unwrap();
    let found = database.space().find("Order", &key).unwrap().unwrap();
    assert_eq!(found.get("name"), Value::Text("Um".into()));
    assert_eq!(found.get("seats"), Value::Number(3.0));
    assert_eq!(found.get("rate"), Value::Number(1.5));
    assert_eq!(found.get("cost"), Value::Number(12_345.0));
    assert_eq!(found.get("active"), Value::Bool(true));
    assert_eq!(found.get("day"), Value::Date(day));
    assert_eq!(found.get("twice"), Value::Number(6.0), "calculated");
}

#[test]
fn filters_order_and_pages_run_in_the_database() {
    if url().is_none() {
        return;
    }
    let database = PostgresDatabase::builder(client("queries"))
        .managed(simple(), TableMap::new("plain"))
        .unwrap()
        .build();
    let mut space = database.space();
    for (name, seats, rate, active) in
        [("Alfa", 1.0, 0.5, true), ("Beta", 2.0, 2.5, false), ("Gama", 3.0, 3.0, true)]
    {
        add(
            &mut space,
            "Plain",
            &[
                ("name", name.into()),
                ("seats", Value::Number(seats)),
                ("rate", Value::Number(rate)),
                ("active", Value::Bool(active)),
            ],
        );
    }
    space.commit().unwrap();
    let names = |query: &Query| -> Vec<String> {
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
    };
    let parsed = |text: &str| Query::all().filter(Criteria::parse(text).unwrap());
    assert_eq!(names(&parsed("[seats] > 1")), ["Beta", "Gama"]);
    assert_eq!(names(&parsed("[rate] > 2")), ["Beta", "Gama"], "a whole number against a decimal column");
    assert_eq!(names(&parsed("[rate] = 2.5")), ["Beta"]);
    assert_eq!(names(&parsed("[active] = True")), ["Alfa", "Gama"]);
    assert_eq!(names(&parsed("[name] Like 'B%'")), ["Beta"]);
    assert_eq!(names(&parsed("[name] In ('Alfa', 'Gama')")), ["Alfa", "Gama"]);
    assert_eq!(names(&parsed("[seats] Between(2, 3)")), ["Beta", "Gama"]);
    assert_eq!(names(&Query::all().sort("seats", false)), ["Gama", "Beta", "Alfa"]);
    let page = Query { limit: Some(1), offset: 1, ..Query::all() };
    assert_eq!(names(&page), ["Beta"]);
    let rest = Query { offset: 2, ..Query::all() };
    assert_eq!(names(&rest), ["Gama"]);
}

#[test]
fn a_commit_keeps_all_or_nothing_and_says_what_the_table_refused() {
    if url().is_none() {
        return;
    }
    let mut client = client("refused");
    client
        .batch_execute("CREATE TABLE plain (id BIGINT PRIMARY KEY, name TEXT UNIQUE, seats BIGINT, rate FLOAT8, active BOOLEAN)")
        .unwrap();
    let database = PostgresDatabase::builder(client)
        .words(SqlWords::default())
        .mapped(simple(), TableMap::new("plain"))
        .unwrap()
        .build();
    let mut space = database.space();
    add(&mut space, "Plain", &[("name", "Um".into())]);
    space.commit().unwrap();

    let mut space = database.space();
    add(&mut space, "Plain", &[("name", "Dois".into())]);
    add(&mut space, "Plain", &[("name", "Um".into())]);
    let error = space.commit().unwrap_err();
    assert_eq!(error, SpaceError::Refused("There is already a record with this Nome".into()));
    assert_eq!(database.space().query("Plain", &Query::all()).unwrap().len(), 1, "nothing of it stayed");
    // The connection goes on working after a transaction that failed.
    let mut space = database.space();
    add(&mut space, "Plain", &[("name", "Tres".into())]);
    space.commit().unwrap();
    assert_eq!(database.space().query("Plain", &Query::all()).unwrap().len(), 2);
}

#[test]
fn a_table_that_came_first_is_met_with_the_types_it_has() {
    if url().is_none() {
        return;
    }
    let mut client = client("mapped");
    client
        .batch_execute(
            "CREATE TABLE contas (
                id INTEGER PRIMARY KEY,
                codigo UUID,
                nome VARCHAR(40),
                saldo NUMERIC(12, 2),
                aberta DATE,
                visto TIMESTAMP,
                pequeno SMALLINT
            );
            INSERT INTO contas VALUES
                (1, 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11', 'Banco', 1234.50, '2026-11-03', '2026-11-03 14:30:00', 7)",
        )
        .unwrap();
    let entity = EntityInfo::new("Conta")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("code", FieldKind::Guid))
        .field(FieldInfo::new("name", FieldKind::Text))
        .field(FieldInfo::new("balance", FieldKind::Decimal(2)))
        .field(FieldInfo::new("opened", FieldKind::Date))
        .field(FieldInfo::new("seen", FieldKind::DateTime))
        .field(FieldInfo::new("small", FieldKind::Integer));
    let map = TableMap::new("contas")
        .column("code", "codigo")
        .column("name", "nome")
        .column("balance", "saldo")
        .column("opened", "aberta")
        .column("seen", "visto")
        .column("small", "pequeno");
    let database = PostgresDatabase::builder(client).mapped(entity, map).unwrap().build();
    let found = database.space().find("Conta", &Value::Number(1.0)).unwrap().unwrap();
    assert_eq!(found.get("code"), Value::Text("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11".into()));
    assert_eq!(found.get("name"), Value::Text("Banco".into()));
    assert_eq!(found.get("balance"), Value::Number(1234.5));
    assert_eq!(found.get("opened"), Value::Date(DateTime::parse("2026-11-03").unwrap()));
    assert_eq!(found.get("seen"), Value::Date(DateTime::parse("2026-11-03 14:30:00").unwrap()));
    assert_eq!(found.get("small"), Value::Number(7.0));

    let mut space = database.space();
    let mut changed = found;
    changed.set("balance", Value::Number(-9876.25));
    changed.set("opened", Value::Date(DateTime::parse("2027-01-02").unwrap()));
    changed.set("seen", Value::Date(DateTime::parse("1999-12-31 23:59:59").unwrap()));
    changed.set("code", "b0eebc99-9c0b-4ef8-bb6d-6bb9bd380a22");
    space.save(changed).unwrap();
    space.commit().unwrap();
    let again = database.space().find("Conta", &Value::Number(1.0)).unwrap().unwrap();
    assert_eq!(again.get("balance"), Value::Number(-9876.25));
    assert_eq!(again.get("opened"), Value::Date(DateTime::parse("2027-01-02").unwrap()));
    assert_eq!(again.get("seen"), Value::Date(DateTime::parse("1999-12-31 23:59:59").unwrap()));
    assert_eq!(again.get("code"), Value::Text("b0eebc99-9c0b-4ef8-bb6d-6bb9bd380a22".into()));
    let filtered = Query::all().filter(Criteria::parse("[balance] < 0").unwrap());
    assert_eq!(database.space().query("Conta", &filtered).unwrap().len(), 1);
}

#[test]
fn a_migration_runs_once_and_one_that_fails_leaves_nothing() {
    if url().is_none() {
        return;
    }
    let builder = PostgresDatabase::builder(client("migrations"))
        .migration("first", "CREATE TABLE a (id BIGINT PRIMARY KEY); INSERT INTO a VALUES (1)")
        .unwrap()
        .migration("first", "INSERT INTO a VALUES (2)")
        .unwrap();
    let failed = builder.migration("broken", "CREATE TABLE b (id BIGINT); SELECT nothing FROM nowhere");
    assert!(failed.is_err());
    let mut check = Client::connect(&url().unwrap(), NoTls).unwrap();
    check.batch_execute("SET search_path TO migrations").unwrap();
    assert_eq!(check.query("SELECT id FROM a", &[]).unwrap().len(), 1);
    let left = check.query(
        "SELECT 1 FROM information_schema.tables WHERE table_schema = 'migrations' AND table_name = 'b'",
        &[],
    );
    assert!(left.unwrap().is_empty());
}

#[test]
fn a_field_added_to_the_model_gets_its_column() {
    if url().is_none() {
        return;
    }
    let mut first = client("added");
    first.batch_execute("CREATE TABLE plain (id BIGINT PRIMARY KEY, name TEXT)").unwrap();
    let database =
        PostgresDatabase::builder(first).managed(simple(), TableMap::new("plain")).unwrap().build();
    let mut space = database.space();
    add(&mut space, "Plain", &[("name", "Um".into()), ("seats", Value::Number(2.0))]);
    space.commit().unwrap();
    let found = database.space().query("Plain", &Query::all()).unwrap();
    assert_eq!(found[0].get("seats"), Value::Number(2.0));
}

#[test]
fn the_web_api_works_over_it_with_permissions_rules_and_states() {
    if url().is_none() {
        return;
    }
    let mut builder =
        PostgresDatabase::builder(client("service")).managed(order(), TableMap::new("orders")).unwrap();
    for entity in security_entities() {
        let table = format!("t_{}", entity.name.to_lowercase());
        builder = builder.managed(entity, TableMap::new(table)).unwrap();
    }
    let database = builder.build();
    let making = database.clone();
    let space: SpaceMaker = Rc::new(move || Box::new(making.space()) as Box<dyn ObjectSpace>);
    let roles = vec![
        Role::new("Administradores").administrative(),
        Role::new("Sales").policy(PermissionPolicy::AllowAll).member_permission(
            "Order",
            ["cost"],
            None,
            [Operation::Read, Operation::Write],
            Access::Deny,
        ),
    ];
    UserDirectory::new(space.clone(), roles.clone()).unwrap();
    let mut records = database.space();
    let found = records.query(SECURITY_ROLE, &Query::all()).unwrap();
    let role = found.iter().find(|role| role.get("name") == Value::Text("Sales".into())).unwrap().get("id");
    let user = add(
        &mut records,
        SECURITY_USER,
        &[
            ("name", "bia".into()),
            ("password_hash", hash_password("uma-senha-de-teste").unwrap().into()),
            ("must_change_password", Value::Bool(false)),
        ],
    );
    add(&mut records, SECURITY_USER_ROLE, &[("user_id", user), ("role_id", role)]);
    records.commit().unwrap();

    let mut api = ApiService::new(space, roles).unwrap();
    let body = json!({ "name": "bia", "password": "uma-senha-de-teste" });
    let token = api.handle(ApiRequest::new("POST", "/api/auth/token").json(&body));
    assert_eq!(token.status, 200, "{token:?}");
    let token = token.body["token"].as_str().unwrap().to_owned();
    let empty = ApiRequest::new("POST", "/api/data/Order").bearer(&token).json(&json!({ "seats": 1 }));
    assert_eq!(api.handle(empty).status, 422);
    let made = api.handle(
        ApiRequest::new("POST", "/api/data/Order")
            .bearer(&token)
            .json(&json!({ "name": "Um", "state": "o" })),
    );
    assert_eq!(made.status, 201, "{made:?}");
    let key = made.body["id"].as_i64().unwrap();
    let path = format!("/api/data/Order/{key}");
    let cost = api.handle(ApiRequest::new("PATCH", &path).bearer(&token).json(&json!({ "cost": 1 })));
    assert_eq!(cost.status, 403);
    let close = api.handle(ApiRequest::new("PATCH", &path).bearer(&token).json(&json!({ "state": "c" })));
    assert_eq!(close.status, 200, "{close:?}");
    let back = api.handle(ApiRequest::new("PATCH", &path).bearer(&token).json(&json!({ "state": "o" })));
    assert_eq!(back.status, 422, "{back:?}");
    let list =
        api.handle(ApiRequest::new("GET", "/api/data/Order").bearer(&token).query("filter", "[name] = 'Um'"));
    assert_eq!(list.body["items"][0]["state"], "c");
}
