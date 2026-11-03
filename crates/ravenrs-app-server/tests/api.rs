//! The API as a client sees it: what each person can do, and what never leaves.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::rc::Rc;
use std::time::Duration;

use ravenrs_app::{
    Access, AuditMode, AuditSettings, EntityInfo, EnumOption, FieldInfo, FieldKind, MemoryDatabase,
    ObjectSpace, Operation, PermissionPolicy, Query, Role, RoleMerge, Rule, SECURITY_API_KEY, SECURITY_ROLE,
    SECURITY_USER, SECURITY_USER_ROLE, SpaceMaker, StateMachine, UserDirectory, Value, audit_entity,
    hash_password, security_entities,
};
use ravenrs_app_server::{ApiRequest, ApiResponse, ApiService, Failure, SettingAccess, serve};
use serde_json::{Value as Json, json};

const PASSWORD: &str = "uma-senha-de-teste";

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .caption("Pedido")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("owner", FieldKind::Text))
        .field(FieldInfo::new("cost", FieldKind::Money))
        .field(FieldInfo::new("token", FieldKind::Text).secret())
        .field(FieldInfo::new("twice", FieldKind::Integer).calculated("[seats] * 2"))
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("on", FieldKind::Date))
        .field(FieldInfo::new(
            "state",
            FieldKind::Enum(vec![EnumOption::new("o", "Open"), EnumOption::new("c", "Closed")]),
        ))
        .field(
            FieldInfo::new("items", FieldKind::Collection("Item".into()))
                .back_reference("order_id")
                .aggregated(),
        )
        .field(FieldInfo::new("tags", FieldKind::Collection("Item".into())).back_reference("order_id"))
        .rule(Rule::required("name"))
        .state_machine(
            StateMachine::new("state")
                .state("o", "Open")
                .state("c", "Closed")
                .start("o")
                .transition("o", "c", "Close"),
        )
}

fn item() -> EntityInfo {
    EntityInfo::new("Item")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("order_id", FieldKind::Reference("Order".into())))
}

fn roles() -> Vec<Role> {
    vec![
        Role::new("Administradores").administrative(),
        Role::new("Sales")
            .policy(PermissionPolicy::AllowAll)
            .member_permission("Order", ["cost"], None, [Operation::Read, Operation::Write], Access::Deny)
            .object_permission(
                "Order",
                "[owner] <> ?CurrentUserName",
                [Operation::Write, Operation::Delete],
                Access::Deny,
            ),
        Role::new("Reader").policy(PermissionPolicy::ReadOnlyAll).object_permission(
            "Order",
            "[owner] = 'ana'",
            [Operation::Read],
            Access::Deny,
        ),
    ]
}

struct Api {
    service: ApiService,
    database: MemoryDatabase,
}

fn user(space: &mut dyn ObjectSpace, name: &str, role: &str, must_change: bool) {
    let roles = space.query(SECURITY_ROLE, &Query::all()).unwrap();
    let role = roles.iter().find(|found| found.get("name") == Value::Text(role.into())).unwrap().get("id");
    let mut record = space.new_object(SECURITY_USER).unwrap();
    let key = record.get("id");
    record.set("name", name);
    record.set("password_hash", hash_password(PASSWORD).unwrap());
    record.set("must_change_password", must_change);
    space.save(record).unwrap();
    let mut link = space.new_object(SECURITY_USER_ROLE).unwrap();
    link.set("user_id", key);
    link.set("role_id", role);
    space.save(link).unwrap();
}

fn database() -> MemoryDatabase {
    let mut entities = vec![order(), item(), audit_entity()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).unwrap();
    UserDirectory::new(maker(&database), roles()).unwrap();
    let mut space = database.space();
    user(&mut space, "ana", "Administradores", false);
    user(&mut space, "bia", "Sales", false);
    user(&mut space, "caio", "Reader", false);
    user(&mut space, "dani", "Sales", true);
    let rows = [(1.0, "Da ana", "ana", 900.0), (2.0, "Da bia", "bia", 500.0)];
    for (id, name, owner, cost) in rows {
        let mut record = space.new_object("Order").unwrap();
        record.set("id", Value::Number(id));
        record.set("name", name);
        record.set("owner", owner);
        record.set("cost", Value::Number(cost));
        record.set("token", "segredo");
        record.set("seats", Value::Number(3.0));
        record.set("state", "o");
        space.save(record).unwrap();
    }
    for (id, name, owner) in [(10.0, "Parafuso", 1.0), (20.0, "Prego", 2.0)] {
        let mut record = space.new_object("Item").unwrap();
        record.set("id", Value::Number(id));
        record.set("name", name);
        record.set("order_id", Value::Number(owner));
        space.save(record).unwrap();
    }
    space.commit().unwrap();
    database
}

fn maker(database: &MemoryDatabase) -> SpaceMaker {
    let database = database.clone();
    Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
}

impl Api {
    fn new() -> Self {
        let database = database();
        let service = ApiService::new(maker(&database), roles()).unwrap();
        Self { service, database }
    }

    fn token(&mut self, name: &str) -> String {
        let answer = self.send(
            ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": name, "password": PASSWORD })),
        );
        assert_eq!(answer.status, 200, "{answer:?}");
        answer.body["token"].as_str().unwrap().to_owned()
    }

    fn send(&mut self, request: ApiRequest) -> ApiResponse {
        self.service.handle(request)
    }

    fn stored(&self, entity: &str, key: f64, field: &str) -> Value {
        self.database.space().find(entity, &Value::Number(key)).unwrap().unwrap().get(field)
    }
}

#[test]
fn nothing_opens_without_a_token_and_a_wrong_password_gets_none() {
    let mut api = Api::new();
    for path in ["/api/data/Order", "/api/openapi.json", "/api/me", "/api/data/Order/1"] {
        assert_eq!(api.send(ApiRequest::new("GET", path)).status, 401, "{path}");
        assert_eq!(api.send(ApiRequest::new("GET", path).bearer("not-a-token")).status, 401, "{path}");
    }
    let wrong = api.send(
        ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": "ana", "password": "errada" })),
    );
    assert_eq!(wrong.status, 401);
    assert!(wrong.body.get("token").is_none());
    let nobody = api.send(
        ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": "ninguem", "password": "errada" })),
    );
    assert_eq!(nobody.body, wrong.body, "it does not say which of them was wrong");
}

#[test]
fn a_token_stops_when_it_is_revoked_when_it_expires_and_a_user_turned_off_loses_everything() {
    let mut api = Api::new();
    let token = api.token("bia");
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&token)).status, 200);
    api.send(ApiRequest::new("POST", "/api/auth/revoke").bearer(&token));
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&token)).status, 401);

    let database = database();
    let mut short = Api {
        service: ApiService::new(maker(&database), roles()).unwrap().session_lifetime(Duration::ZERO),
        database,
    };
    let token = short.token("bia");
    assert_eq!(short.send(ApiRequest::new("GET", "/api/data/Order").bearer(&token)).status, 401);

    let mut api = Api::new();
    let token = api.token("bia");
    let mut space = api.database.space();
    let users = space.query(SECURITY_USER, &Query::all()).unwrap();
    let mut bia = users.into_iter().find(|user| user.get("name") == Value::Text("bia".into())).unwrap();
    bia.set("active", false);
    space.save(bia).unwrap();
    space.commit().unwrap();
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&token)).status, 401);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/me").bearer(&token)).status, 401, "the session is over");
}

#[test]
fn tries_to_sign_in_are_limited_by_address_and_nonsense_does_not_use_them() {
    let database = database();
    let mut api =
        Api { service: ApiService::new(maker(&database), roles()).unwrap().sign_ins_a_minute(2), database };
    let attempt = ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": "x", "password": "y" }));
    assert_eq!(api.send(attempt.clone()).status, 401);
    assert_eq!(api.send(attempt.clone()).status, 401);
    assert_eq!(api.send(attempt.clone()).status, 429);
    assert_eq!(api.send(attempt.clone().client("10.0.0.2")).status, 401, "another address is not held up");

    let database = crate::database();
    let mut api =
        Api { service: ApiService::new(maker(&database), roles()).unwrap().sign_ins_a_minute(2), database };
    for _ in 0..10 {
        let mut nonsense = ApiRequest::new("POST", "/api/auth/token");
        nonsense.body = b"not json".to_vec();
        assert_eq!(api.send(nonsense).status, 400);
    }
    assert_eq!(api.token("ana").len(), 64, "nonsense used nobody's turn");
}

#[test]
fn someone_who_has_to_change_their_password_can_do_only_that_and_other_tokens_end() {
    let mut api = Api::new();
    let token = api.token("dani");
    let list = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&token));
    assert_eq!(list.status, 403);
    assert_eq!(list.body["error"], "password_change_required");
    let other = api.token("dani");
    let change = api.send(
        ApiRequest::new("POST", "/api/auth/password")
            .bearer(&token)
            .json(&json!({ "current": PASSWORD, "new": "uma-senha-bem-nova" })),
    );
    assert_eq!(change.status, 200, "{change:?}");
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&token)).status, 200);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&other)).status, 401);
}

#[test]
fn what_cannot_be_seen_does_not_leave_and_cannot_be_found_out() {
    let mut api = Api::new();
    let bia = api.token("bia");
    let caio = api.token("caio");

    // A field denied to a role comes empty, and a secret field never comes.
    let list = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&bia));
    let first = &list.body["items"][0];
    assert_eq!(first["cost"], Json::Null);
    assert!(first.get("token").is_none());
    assert_eq!(first["twice"], 6, "calculated");

    // A record denied to a role is as if it were not there.
    let list = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&caio).query("count", "true"));
    assert_eq!(list.body["items"].as_array().unwrap().len(), 1);
    assert_eq!(list.body["count"], 1);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Order/1").bearer(&caio)).status, 404);

    // Asking by a hidden value says nothing about it.
    for filter in ["[cost] > 800", "[cost] = 900"] {
        let found = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&bia).query("filter", filter));
        assert_eq!(found.body["items"].as_array().unwrap().len(), 0, "{filter}");
    }
    let sorted = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&bia).query("order", "-cost"));
    assert_eq!(sorted.body["items"][0]["id"], 1, "going by the empty values, and not by what is hidden");
    let secret = api
        .send(ApiRequest::new("GET", "/api/data/Order").bearer(&bia).query("filter", "[token] = 'segredo'"));
    assert_eq!(secret.status, 400);
    let secret = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&bia).query("order", "token"));
    assert_eq!(secret.status, 400);
}

#[test]
fn who_can_sign_in_and_what_was_done_are_not_records_for_clients_even_for_an_administrator() {
    let mut api = Api::new();
    let ana = api.token("ana");
    for entity in [SECURITY_USER, SECURITY_USER_ROLE, "SecurityAuditEntry"] {
        let path = format!("/api/data/{entity}");
        assert_eq!(api.send(ApiRequest::new("GET", &path).bearer(&ana)).status, 404, "{entity}");
        let write = api.send(ApiRequest::new("POST", &path).bearer(&ana).json(&json!({ "name": "x" })));
        assert_eq!(write.status, 404, "{entity}");
    }
    let description = api.send(ApiRequest::new("GET", "/api/openapi.json").bearer(&ana));
    assert!(!description.body.to_string().contains("password_hash"));
    assert!(
        !description.body["paths"].as_object().unwrap().contains_key(&format!("/api/data/{SECURITY_USER}"))
    );
}

#[test]
fn writing_follows_the_same_permissions_as_the_screens() {
    let mut api = Api::new();
    let bia = api.token("bia");
    let caio = api.token("caio");

    // Someone else's record.
    let other =
        api.send(ApiRequest::new("PATCH", "/api/data/Order/1").bearer(&bia).json(&json!({ "name": "Meu" })));
    assert_eq!(other.status, 403);
    assert_eq!(api.stored("Order", 1.0, "name"), Value::Text("Da ana".into()));
    assert_eq!(api.send(ApiRequest::new("DELETE", "/api/data/Order/1").bearer(&bia)).status, 403);

    // A field denied.
    let cost =
        api.send(ApiRequest::new("PATCH", "/api/data/Order/2").bearer(&bia).json(&json!({ "cost": 1 })));
    assert_eq!(cost.status, 403);
    assert_eq!(api.stored("Order", 2.0, "cost"), Value::Number(500.0));

    // Their own, keeping what they cannot see.
    let mine =
        api.send(ApiRequest::new("PATCH", "/api/data/Order/2").bearer(&bia).json(&json!({ "name": "Novo" })));
    assert_eq!(mine.status, 200, "{mine:?}");
    assert_eq!(api.stored("Order", 2.0, "name"), Value::Text("Novo".into()));
    assert_eq!(api.stored("Order", 2.0, "cost"), Value::Number(500.0), "kept");

    // Only reading.
    let reader =
        api.send(ApiRequest::new("POST", "/api/data/Order").bearer(&caio).json(&json!({ "name": "x" })));
    assert_eq!(reader.status, 403);
    assert_eq!(api.send(ApiRequest::new("DELETE", "/api/data/Order/2").bearer(&caio)).status, 403);
}

#[test]
fn only_what_can_be_written_is_taken() {
    let mut api = Api::new();
    let ana = api.token("ana");
    for (body, why) in [
        (json!({ "name": "x", "nothing": 1 }), "a field that does not exist"),
        (json!({ "name": "x", "twice": 1 }), "a calculated field"),
        (json!({ "name": "x", "token": "mine" }), "a secret field"),
        (json!({ "name": "x", "id": 1 }), "a numbered key"),
        (json!({ "name": "x", "seats": "muitos" }), "the wrong kind of value"),
        (json!({ "name": "x", "state": "nada" }), "an option that does not exist"),
        (json!({ "name": "x", "on": "ontem" }), "not a date"),
        (json!({ "name": "x", "tags": [] }), "a collection that is not part of it"),
    ] {
        let answer = api.send(ApiRequest::new("POST", "/api/data/Order").bearer(&ana).json(&body));
        assert_eq!(answer.status, 400, "{why}: {answer:?}");
    }
    let not_an_object = ApiRequest::new("POST", "/api/data/Order").bearer(&ana).json(&json!([1]));
    assert_eq!(api.send(not_an_object).status, 400);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Nothing").bearer(&ana)).status, 404);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/data/Order/abc").bearer(&ana)).status, 404);
    assert_eq!(api.send(ApiRequest::new("PUT", "/api/data/Order").bearer(&ana)).status, 404);
}

#[test]
fn rules_and_states_hold_here_too() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let empty =
        api.send(ApiRequest::new("POST", "/api/data/Order").bearer(&ana).json(&json!({ "seats": 1 })));
    assert_eq!(empty.status, 422);
    assert_eq!(empty.body["violations"][0]["field"], "name");
    let would =
        api.send(ApiRequest::new("POST", "/api/validate/Order").bearer(&ana).json(&json!({ "seats": 1 })));
    assert_eq!(would.status, 200);
    assert_eq!(would.body["violations"][0]["stops"], true);
    assert_eq!(api.database.space().query("Order", &Query::all()).unwrap().len(), 2, "nothing was kept");

    let closed =
        api.send(ApiRequest::new("PATCH", "/api/data/Order/1").bearer(&ana).json(&json!({ "state": "c" })));
    assert_eq!(closed.status, 200, "{closed:?}");
    let back =
        api.send(ApiRequest::new("PATCH", "/api/data/Order/1").bearer(&ana).json(&json!({ "state": "o" })));
    assert_eq!(back.status, 422, "{back:?}");
    assert_eq!(api.stored("Order", 1.0, "state"), Value::Text("c".into()));
}

#[test]
fn filters_order_and_pages_are_what_was_asked() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let found = api
        .send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("filter", "[name] Like 'Da a%'"));
    assert_eq!(found.body["items"].as_array().unwrap().len(), 1);
    assert_eq!(found.body["items"][0]["cost"], 900);
    let order = api.send(
        ApiRequest::new("GET", "/api/data/Order")
            .bearer(&ana)
            .query("order", "-id")
            .query("top", "1")
            .query("count", "true"),
    );
    assert_eq!(order.body["items"].as_array().unwrap().len(), 1);
    assert_eq!(order.body["items"][0]["id"], 2);
    assert_eq!(order.body["count"], 2);
    let skipped = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("skip", "1"));
    assert_eq!(skipped.body["items"][0]["id"], 2);
    let broken = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("filter", "[name"));
    assert_eq!(broken.status, 400);
    let unknown =
        api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("filter", "[nothing] = 1"));
    assert_eq!(unknown.status, 400);
    let long = "x".repeat(5000);
    let too_long = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("filter", &long));
    assert_eq!(too_long.status, 400);
    let page = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("top", "abc"));
    assert_eq!(page.status, 400);
}

#[test]
fn a_record_and_what_belongs_to_it_go_together() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let created = api.send(ApiRequest::new("POST", "/api/data/Order").bearer(&ana).json(&json!({
        "name": "Com itens", "owner": "ana", "items": [{ "name": "A" }, { "name": "B" }],
    })));
    assert_eq!(created.status, 201, "{created:?}");
    let key = created.body["id"].as_i64().unwrap();
    let path = format!("/api/data/Order/{key}");
    let read = api.send(ApiRequest::new("GET", &path).bearer(&ana).query("expand", "items"));
    let items = read.body["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 2);
    let (a, b) = (items[0]["id"].as_i64().unwrap(), items[1]["id"].as_i64().unwrap());

    let changed = api.send(ApiRequest::new("PATCH", &path).bearer(&ana).json(&json!({
        "items": [{ "id": a, "name": "A2" }, { "$delete": b }, { "name": "C" }],
    })));
    assert_eq!(changed.status, 200, "{changed:?}");
    let read = api.send(ApiRequest::new("GET", &path).bearer(&ana).query("expand", "items"));
    let names: Vec<&str> =
        read.body["items"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["A2", "C"]);

    // What belongs to another record is not reached through this one, and the owner stays.
    for body in [
        json!({ "items": [{ "id": 20, "name": "roubado" }] }),
        json!({ "items": [{ "$delete": 20 }] }),
        json!({ "items": [{ "id": a, "order_id": 2 }] }),
        json!({ "items": [{ "name": "fora", "order_id": 2 }] }),
    ] {
        let answer = api.send(ApiRequest::new("PATCH", &path).bearer(&ana).json(&body));
        assert!(matches!(answer.status, 400 | 404), "{body}: {answer:?}");
    }
    assert_eq!(api.stored("Item", 20.0, "name"), Value::Text("Prego".into()));
    assert_eq!(api.stored("Item", 20.0, "order_id"), Value::Number(2.0));
}

#[test]
fn a_batch_stays_or_goes_whole() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let good = api.send(ApiRequest::new("POST", "/api/batch").bearer(&ana).json(&json!({ "operations": [
        { "method": "POST", "entity": "Order", "body": { "name": "Lote" } },
        { "method": "PATCH", "entity": "Order", "key": 2, "body": { "name": "Mudado" } },
    ] })));
    assert_eq!(good.status, 200, "{good:?}");
    assert_eq!(good.body["results"][0]["name"], "Lote");
    let count = api.database.space().query("Order", &Query::all()).unwrap().len();

    let broken = api.send(ApiRequest::new("POST", "/api/batch").bearer(&ana).json(&json!({ "operations": [
        { "method": "DELETE", "entity": "Order", "key": 1 },
        { "method": "POST", "entity": "Order", "body": {} },
    ] })));
    assert_eq!(broken.status, 422, "{broken:?}");
    assert_eq!(broken.body["operation"], 1);
    assert_eq!(api.database.space().query("Order", &Query::all()).unwrap().len(), count, "nothing happened");

    // Someone who can do only part of it does none of it.
    let bia = api.token("bia");
    let partly = api.send(ApiRequest::new("POST", "/api/batch").bearer(&bia).json(&json!({ "operations": [
        { "method": "PATCH", "entity": "Order", "key": 2, "body": { "name": "Da bia" } },
        { "method": "DELETE", "entity": "Order", "key": 1 },
    ] })));
    assert_eq!(partly.status, 403);
    assert_eq!(api.stored("Order", 2.0, "name"), Value::Text("Mudado".into()));

    let many: Vec<Json> =
        (0..201).map(|_| json!({ "method": "DELETE", "entity": "Order", "key": 1 })).collect();
    let answer =
        api.send(ApiRequest::new("POST", "/api/batch").bearer(&ana).json(&json!({ "operations": many })));
    assert_eq!(answer.status, 400);
}

#[test]
fn an_endpoint_of_the_application_runs_with_the_permissions_of_who_calls() {
    let database = database();
    let service = ApiService::new(maker(&database), roles()).unwrap().endpoint("close-day", |cx, input| {
        let mut space = cx.space.borrow_mut();
        let mut record = space
            .find("Order", &Value::Number(2.0))
            .map_err(Failure::from)?
            .ok_or_else(Failure::not_found)?;
        record.set("name", format!("{} por {}", input["motivo"].as_str().unwrap_or(""), cx.name));
        space.save(record).map_err(Failure::from)?;
        space.commit().map_err(Failure::from)?;
        Ok(json!({ "feito": true }))
    });
    let mut api = Api { service, database };
    let bia = api.token("bia");
    let done = api
        .send(ApiRequest::new("POST", "/api/run/close-day").bearer(&bia).json(&json!({ "motivo": "Fim" })));
    assert_eq!(done.status, 200, "{done:?}");
    assert_eq!(api.stored("Order", 2.0, "name"), Value::Text("Fim por bia".into()));
    let caio = api.token("caio");
    let refused = api.send(ApiRequest::new("POST", "/api/run/close-day").bearer(&caio).json(&json!({})));
    assert_eq!(refused.status, 403);
    let none = api.send(ApiRequest::new("POST", "/api/run/nothing").bearer(&bia));
    assert_eq!(none.status, 404);
}

#[test]
fn what_is_done_here_goes_to_the_trail_under_the_name_of_who_did_it() {
    let database = database();
    let settings = AuditSettings::new(AuditMode::Full);
    let service = ApiService::new(maker(&database), roles()).unwrap().audit(settings);
    let mut api = Api { service, database };
    let bia = api.token("bia");
    let answer = api.send(
        ApiRequest::new("PATCH", "/api/data/Order/2").bearer(&bia).json(&json!({ "name": "Auditado" })),
    );
    assert_eq!(answer.status, 200, "{answer:?}");
    let trail = api.database.space().query("SecurityAuditEntry", &Query::all()).unwrap();
    assert!(!trail.is_empty());
    assert!(
        trail.iter().any(|entry| entry.values().values().any(|value| *value == Value::Text("bia".into())))
    );
}

#[test]
fn the_description_is_of_what_this_person_can_reach() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let description = api.send(ApiRequest::new("GET", "/api/openapi.json").bearer(&ana));
    assert_eq!(description.status, 200);
    assert_eq!(description.body["openapi"], "3.0.3");
    assert!(description.body["paths"].as_object().unwrap().contains_key("/api/data/Order/{key}"));
    let order = &description.body["components"]["schemas"]["Order"]["properties"];
    assert_eq!(order["seats"]["type"], "integer");
    assert_eq!(order["twice"]["readOnly"], true);
    assert!(order.get("token").is_none());
    assert_eq!(order["state"]["enum"], json!(["o", "c"]));
}

#[test]
fn it_answers_over_real_http() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let address = listener.local_addr().unwrap();
    runtime.spawn(serve(listener, || {
        let database = database();
        ApiService::new(maker(&database), roles()).map_err(|error| error.to_string())
    }));
    let url = |path: &str| format!("http://{address}{path}");
    let health = ureq::get(url("/health")).call().unwrap().into_body().read_to_string().unwrap();
    assert!(health.contains("ok"));

    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let denied = agent.get(url("/api/data/Order")).call().unwrap();
    assert_eq!(denied.status(), 401);
    assert_eq!(denied.headers().get("www-authenticate").unwrap(), "Bearer");

    let sign_in = json!({ "name": "ana", "password": PASSWORD }).to_string();
    let mut answer = agent.post(url("/api/auth/token")).send(sign_in).unwrap();
    let body: Json = serde_json::from_str(&answer.body_mut().read_to_string().unwrap()).unwrap();
    let bearer = format!("Bearer {}", body["token"].as_str().unwrap());

    let mut found = agent
        .get(url("/api/data/Order?filter=%5Bname%5D%20Like%20%27Da%20a%25%27&top=5"))
        .header("Authorization", &bearer)
        .call()
        .unwrap();
    assert_eq!(found.status(), 200);
    let body: Json = serde_json::from_str(&found.body_mut().read_to_string().unwrap()).unwrap();
    assert_eq!(body["items"][0]["name"], "Da ana");

    let large = vec![b'x'; 5 * 1024 * 1024];
    let too_large = agent.post(url("/api/data/Order")).header("Authorization", &bearer).send(&large[..]);
    assert!(too_large.is_err() || too_large.is_ok_and(|answer| answer.status() == 413));
    let unsigned = agent.post(url("/api/data/Order")).send(&large[..]);
    assert!(unsigned.is_err() || unsigned.is_ok_and(|answer| answer.status() == 401));
    runtime.shutdown_background();
}

#[test]
fn a_hostile_filter_gets_an_answer_and_the_service_goes_on() {
    let mut api = Api::new();
    let bia = api.token("bia");
    let deep = format!("{}1{}", format!("{}[a][", "(".repeat(127)).repeat(30), "]".repeat(30));
    let between = format!("[seats] {}", "0 between(".repeat(409));
    let long_chain = format!("[seats]{}", " + 1".repeat(1000));
    for filter in [
        deep.as_str(),
        between.as_str(),
        long_chain.as_str(),
        "FormatString('{0:D70000}',1)='x'",
        "FormatString('{0:F70000}',1)='x'",
        "FormatString('{0:X70000}',1)='x'",
        "PadLeft('a',9999999999999999999999999999999999999999)='x'",
        "AddDays(Now(),9999999999999999999999999)='x'",
        "AddYears(Now(),9999999999999999999999999)='x'",
    ] {
        let answer = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&bia).query("filter", filter));
        assert!(matches!(answer.status, 200 | 400), "{filter}: {answer:?}");
    }
    assert_eq!(api.send(ApiRequest::new("GET", "/api/me").bearer(&bia)).status, 200);
}

#[test]
fn sorting_values_of_mixed_kinds_does_not_panic() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let mut space = api.database.space();
    for index in 0..600 {
        let mut record = space.new_object("Item").unwrap();
        let reference = match index % 3 {
            0 => Value::Number(1.0),
            1 => Value::Text("texto".into()),
            _ => Value::Null,
        };
        record.set("order_id", reference);
        space.save(record).unwrap();
    }
    space.commit().unwrap();
    let answer = api.send(ApiRequest::new("GET", "/api/data/Item").bearer(&ana).query("order", "order_id"));
    assert_eq!(answer.status, 200);
}

#[test]
fn guessing_the_value_of_a_field_gets_the_same_answer_as_any_other_value() {
    let mut api = Api::new();
    let bia = api.token("bia");
    for guess in [500, 501] {
        let body = json!({ "cost": guess });
        let answer = api.send(ApiRequest::new("PATCH", "/api/data/Order/2").bearer(&bia).json(&body));
        assert_eq!(answer.status, 403, "{guess}");
    }
    assert_eq!(api.stored("Order", 2.0, "cost"), Value::Number(500.0));
}

#[test]
fn a_session_left_open_does_not_get_to_go_on_guessing_the_password() {
    let mut api = Api::new();
    let bia = api.token("bia");
    for _ in 0..5 {
        let body = json!({ "current": "errada", "new": "uma-senha-bem-nova" });
        let answer = api.send(ApiRequest::new("POST", "/api/auth/password").bearer(&bia).json(&body));
        assert_eq!(answer.status, 403);
    }
    let body = json!({ "current": PASSWORD, "new": "uma-senha-bem-nova" });
    let locked = api.send(ApiRequest::new("POST", "/api/auth/password").bearer(&bia).json(&body));
    assert_eq!(locked.status, 403, "locked as signing in would be");
    let same =
        ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": "bia", "password": PASSWORD }));
    assert_eq!(api.send(same).status, 401);
}

#[test]
fn choosing_a_new_password_means_another_one() {
    let mut api = Api::new();
    let dani = api.token("dani");
    let body = json!({ "current": PASSWORD, "new": PASSWORD });
    let same = api.send(ApiRequest::new("POST", "/api/auth/password").bearer(&dani).json(&body));
    assert_eq!(same.status, 422);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/me").bearer(&dani)).status, 403);
}

fn hidden_state_and_rule() -> (EntityInfo, Vec<Role>) {
    let entity = EntityInfo::new("Ticket")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
        .field(FieldInfo::new("code", FieldKind::Text))
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("twice", FieldKind::Integer).calculated("[seats] * 2"))
        .field(FieldInfo::new(
            "state",
            FieldKind::Enum(vec![EnumOption::new("o", "Open"), EnumOption::new("c", "Closed")]),
        ))
        .rule(Rule::required("code"))
        .state_machine(
            StateMachine::new("state")
                .state("o", "Open")
                .state("c", "Closed")
                .start("o")
                .transition("o", "c", "Close"),
        );
    let role = Role::new("Sales").policy(PermissionPolicy::AllowAll).member_permission(
        "Ticket",
        ["code", "state", "twice"],
        None,
        [Operation::Read],
        Access::Deny,
    );
    (entity, vec![Role::new("Administradores").administrative(), role])
}

#[test]
fn rules_see_the_whole_record_and_what_comes_back_is_only_what_can_be_seen() {
    let (entity, roles) = hidden_state_and_rule();
    let mut entities = vec![entity, audit_entity()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).unwrap();
    UserDirectory::new(maker(&database), roles.clone()).unwrap();
    let mut space = database.space();
    user(&mut space, "bia", "Sales", false);
    let mut record = space.new_object("Ticket").unwrap();
    let key = record.get("id");
    record.set("name", "Um");
    record.set("code", "A1");
    record.set("seats", Value::Number(3.0));
    space.save(record.clone()).unwrap();
    space.commit().unwrap();
    record.set("state", "c");
    space.save(record).unwrap();
    space.commit().unwrap();
    let Value::Number(number) = key else { panic!("a number") };
    let path = format!("/api/data/Ticket/{number}");
    let mut api = Api { service: ApiService::new(maker(&database), roles).unwrap(), database };
    let bia = api.token("bia");

    // A required field they cannot see is still there, so the rule has nothing to say.
    let changed = api.send(ApiRequest::new("PATCH", &path).bearer(&bia).json(&json!({ "name": "Dois" })));
    assert_eq!(changed.status, 200, "{changed:?}");
    assert_eq!(changed.body["name"], "Dois");
    // What comes back is what a read gives: no calculated field that is denied to them.
    assert_eq!(changed.body["twice"], Json::Null);
    assert_eq!(changed.body["code"], Json::Null);
    // A state they cannot see does not show in what refusing it would say.
    let state = api.send(ApiRequest::new("PATCH", &path).bearer(&bia).json(&json!({ "state": "o" })));
    assert_eq!(state.status, 403, "{state:?}");
    assert!(!state.body.to_string().contains("Closed"));
}

#[test]
fn what_is_too_much_for_one_request_is_refused() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let names: Vec<String> = (0..9).map(|n| format!("c{n}")).collect();
    let answer =
        api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("expand", &names.join(",")));
    assert_eq!(answer.status, 400);
    let repeated = ["items"; 5000].join(",");
    let answer = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&ana).query("expand", &repeated));
    assert_eq!(answer.status, 200, "the same one, once");

    let items: Vec<Json> = (0..2001).map(|n| json!({ "name": format!("{n}") })).collect();
    let body = json!({ "name": "Grande", "items": items });
    let answer = api.send(ApiRequest::new("POST", "/api/data/Order").bearer(&ana).json(&body));
    assert_eq!(answer.status, 400);
}

#[test]
fn numbers_stay_exact_and_references_go_somewhere() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let large = api.send(
        ApiRequest::new("POST", "/api/data/Order")
            .bearer(&ana)
            .json(&json!({ "name": "x", "seats": 9_007_199_254_740_993_u64 })),
    );
    assert_eq!(large.status, 400);
    for reference in [json!(999), json!("texto")] {
        let body = json!({ "name": "x", "order_id": reference });
        let answer = api.send(ApiRequest::new("POST", "/api/data/Item").bearer(&ana).json(&body));
        assert_eq!(answer.status, 400, "{answer:?}");
    }
    let good = api.send(
        ApiRequest::new("POST", "/api/data/Item").bearer(&ana).json(&json!({ "name": "x", "order_id": 1 })),
    );
    assert_eq!(good.status, 201, "{good:?}");
}

#[test]
fn only_the_origins_named_can_read_answers_from_a_browser() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = runtime.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let address = listener.local_addr().unwrap();
    let options = ravenrs_app_server::ApiOptions::new().allow_origin("https://bom.example");
    runtime.spawn(ravenrs_app_server::serve_with(listener, options, || {
        let database = database();
        ApiService::new(maker(&database), roles()).map_err(|error| error.to_string())
    }));
    let url = format!("http://{address}/api/data/Order");
    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let allowed = |origin: &str| {
        let asked = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .allow_non_standard_methods(true)
            .build()
            .new_agent()
            .run(ureq::http::Request::options(&url).header("Origin", origin).body(()).unwrap())
            .unwrap();
        (asked.status().as_u16(), asked.headers().get("access-control-allow-origin").cloned())
    };
    let (status, header) = allowed("https://bom.example");
    assert_eq!(status, 204);
    assert_eq!(header.unwrap(), "https://bom.example");
    assert_eq!(allowed("https://mau.example").1, None);
    let plain = agent.get(&url).header("Origin", "https://mau.example").call().unwrap();
    assert_eq!(plain.status(), 401);
    assert!(plain.headers().get("access-control-allow-origin").is_none());
    runtime.shutdown_background();
}

fn key_of(answer: &ApiResponse) -> String {
    assert_eq!(answer.status, 201, "{answer:?}");
    answer.body["key"].as_str().unwrap().to_owned()
}

#[test]
fn a_key_acts_as_its_user_and_shows_once() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let made = api.send(
        ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&json!({ "name": "ERP", "user": "bia" })),
    );
    let key = key_of(&made);
    let id = made.body["id"].as_str().unwrap().to_owned();

    let list = api.send(ApiRequest::new("GET", "/api/data/Order").bearer(&key));
    assert_eq!(list.status, 200, "{list:?}");
    assert_eq!(list.body["items"][0]["cost"], Json::Null, "as bia sees it");
    let other = api.send(ApiRequest::new("DELETE", "/api/data/Order/1").bearer(&key));
    assert_eq!(other.status, 403, "as bia can do");

    let keys = api.send(ApiRequest::new("GET", "/api/auth/keys").bearer(&ana));
    assert_eq!(keys.body["items"][0]["name"], "ERP");
    let listed = keys.body.to_string();
    assert!(!listed.contains(key.rsplit('_').next().unwrap()), "the secret does not show again");
    assert!(!listed.contains("secret_hash"));
    let stored = api.database.space().find(SECURITY_API_KEY, &Value::Text(id)).unwrap().unwrap();
    assert_ne!(
        stored.get("secret_hash"),
        Value::Text(key.rsplit('_').next().unwrap().into()),
        "only a hash is kept"
    );
}

#[test]
fn a_key_stops_when_it_is_revoked_rotated_or_expired_and_with_its_user() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let make = |api: &mut Api, body: Json| {
        api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&body))
    };
    let works = |api: &mut Api, key: &str| api.send(ApiRequest::new("GET", "/api/me").bearer(key)).status;

    let made = make(&mut api, json!({ "name": "Um", "user": "bia" }));
    let (key, id) = (key_of(&made), made.body["id"].as_str().unwrap().to_owned());
    let wrong = format!("{}x", &key[..key.len() - 1]);
    assert_eq!(works(&mut api, &wrong), 401);
    assert_eq!(works(&mut api, "rvk_nada"), 401);
    assert_eq!(works(&mut api, &key), 200);

    let rotated = api.send(ApiRequest::new("POST", &format!("/api/auth/keys/{id}/rotate")).bearer(&ana));
    assert_eq!(rotated.status, 200);
    assert_eq!(works(&mut api, &key), 401, "the old one is over");
    let new = rotated.body["key"].as_str().unwrap().to_owned();
    assert_eq!(works(&mut api, &new), 200);
    assert_eq!(api.send(ApiRequest::new("DELETE", &format!("/api/auth/keys/{id}")).bearer(&ana)).status, 200);
    assert_eq!(works(&mut api, &new), 401, "at once");

    let made = make(&mut api, json!({ "name": "Dois", "user": "bia" }));
    let key = key_of(&made);
    let mut space = api.database.space();
    let users = space.query(SECURITY_USER, &Query::all()).unwrap();
    let mut bia = users.into_iter().find(|user| user.get("name") == Value::Text("bia".into())).unwrap();
    bia.set("active", false);
    space.save(bia).unwrap();
    space.commit().unwrap();
    assert_eq!(works(&mut api, &key), 401, "their keys end with the user");

    let made = make(&mut api, json!({ "name": "Tres", "user": "caio", "expires_in_days": 1 }));
    let (key, id) = (key_of(&made), made.body["id"].as_str().unwrap().to_owned());
    assert_eq!(works(&mut api, &key), 200);
    let mut space = api.database.space();
    let mut stored = space.find(SECURITY_API_KEY, &Value::Text(id)).unwrap().unwrap();
    stored.set("expires", Value::Date(ravenrs_criteria::DateTime::parse("2020-01-01").unwrap()));
    space.save(stored).unwrap();
    space.commit().unwrap();
    assert_eq!(works(&mut api, &key), 401);
}

#[test]
fn a_key_never_reaches_past_its_roles_or_what_its_user_has_now() {
    let mut api = Api::new();
    let ana = api.token("ana");
    let beyond = api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&json!({
        "name": "Demais", "user": "bia", "roles": ["Administradores"],
    })));
    assert_eq!(beyond.status, 400, "bia does not have that role");
    let mine = api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&json!({
        "name": "Vendas", "user": "bia", "roles": ["Sales"],
    })));
    let key = key_of(&mine);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/me").bearer(&key)).body["roles"], json!(["Sales"]));

    // A person who is not an administrator makes no keys, and a key makes none, nor changes passwords.
    let bia = api.token("bia");
    let body = json!({ "name": "Minha" });
    assert_eq!(api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&bia).json(&body)).status, 403);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/auth/keys").bearer(&bia)).status, 403);
    assert_eq!(api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&key).json(&body)).status, 403);
    let change = json!({ "current": PASSWORD, "new": "uma-senha-bem-nova" });
    assert_eq!(
        api.send(ApiRequest::new("POST", "/api/auth/password").bearer(&key).json(&change)).status,
        403
    );
    assert_eq!(
        api.send(ApiRequest::new("POST", "/api/auth/revoke").bearer(&key)).status,
        200,
        "says nothing"
    );
    assert_eq!(api.send(ApiRequest::new("GET", "/api/me").bearer(&key)).status, 200, "and ends nothing");

    for wrong in [json!({}), json!({ "name": "  " }), json!({ "name": "x", "expires_in_days": 0 })] {
        assert_eq!(api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&wrong)).status, 400);
    }
    // The entity itself is not there for anyone.
    assert_eq!(
        api.send(ApiRequest::new("GET", &format!("/api/data/{SECURITY_API_KEY}")).bearer(&ana)).status,
        404
    );
}

#[test]
fn settings_are_reached_only_as_declared_and_each_person_has_their_own() {
    let mut entities = vec![order(), item(), audit_entity(), ravenrs_app::settings_entity()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).unwrap();
    UserDirectory::new(maker(&database), roles()).unwrap();
    let mut space = database.space();
    user(&mut space, "ana", "Administradores", false);
    user(&mut space, "bia", "Sales", false);
    space.commit().unwrap();
    let service = ApiService::new(maker(&database), roles())
        .unwrap()
        .setting("theme", SettingAccess::PerPerson)
        .setting("banner", SettingAccess::Shared);
    let mut api = Api { service, database };
    let (ana, bia) = (api.token("ana"), api.token("bia"));
    let put = |value: Json| json!({ "value": value });

    assert_eq!(api.send(ApiRequest::new("GET", "/api/settings/theme")).status, 401);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/settings/nada").bearer(&bia)).status, 404);
    let saved = api
        .send(ApiRequest::new("PUT", "/api/settings/theme").bearer(&bia).json(&put(json!({ "base": "x" }))));
    assert_eq!(saved.status, 200, "{saved:?}");
    assert_eq!(
        api.send(ApiRequest::new("GET", "/api/settings/theme").bearer(&bia)).body["value"]["base"],
        "x"
    );
    assert_eq!(
        api.send(ApiRequest::new("GET", "/api/settings/theme").bearer(&ana)).body["value"],
        Json::Null,
        "hers"
    );
    assert_eq!(api.send(ApiRequest::new("DELETE", "/api/settings/theme").bearer(&bia)).status, 200);
    assert_eq!(
        api.send(ApiRequest::new("GET", "/api/settings/theme").bearer(&bia)).body["value"],
        Json::Null
    );

    assert_eq!(
        api.send(ApiRequest::new("PUT", "/api/settings/banner").bearer(&bia).json(&put(json!("x")))).status,
        403
    );
    assert_eq!(
        api.send(ApiRequest::new("PUT", "/api/settings/banner").bearer(&ana).json(&put(json!("Oi")))).status,
        200
    );
    assert_eq!(api.send(ApiRequest::new("GET", "/api/settings/banner").bearer(&bia)).body["value"], "Oi");
    let large = put(json!("x".repeat(70_000)));
    assert_eq!(api.send(ApiRequest::new("PUT", "/api/settings/theme").bearer(&bia).json(&large)).status, 400);
    assert_eq!(
        api.send(ApiRequest::new("GET", &format!("/api/data/{}", ravenrs_app::APP_SETTING)).bearer(&ana))
            .status,
        404
    );
}

#[test]
fn when_every_role_has_to_allow_a_key_cannot_leave_one_out() {
    let database = database();
    let roles = vec![
        Role::new("Administradores").administrative(),
        Role::new("Sales").policy(PermissionPolicy::AllowAll),
        Role::new("Reader").policy(PermissionPolicy::ReadOnlyAll),
    ];
    let service = ApiService::new(maker(&database), roles).unwrap().role_merge(RoleMerge::AllRoles);
    let mut api = Api { service, database };
    let ana = api.token("ana");
    let body = json!({ "name": "Estreita", "roles": ["Administradores"] });
    assert_eq!(api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&body)).status, 400);

    // One that says so all the same keeps every role of its user.
    let made = api.send(
        ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&json!({ "name": "x", "user": "bia" })),
    );
    let (key, id) = (key_of(&made), made.body["id"].as_str().unwrap().to_owned());
    let mut space = api.database.space();
    let mut stored = space.find(SECURITY_API_KEY, &Value::Text(id)).unwrap().unwrap();
    stored.set("roles", "Outro");
    space.save(stored).unwrap();
    space.commit().unwrap();
    assert_eq!(api.send(ApiRequest::new("GET", "/api/me").bearer(&key)).body["roles"], json!(["Sales"]));
}

#[test]
fn a_key_with_a_comma_or_a_date_nobody_can_read_gets_nothing() {
    let mut api = Api::new();
    let ana = api.token("ana");
    for roles in [json!(["Sales,Administradores"]), json!([","]), json!([])] {
        let body = json!({ "name": "x", "user": "bia", "roles": roles });
        assert_eq!(
            api.send(ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&body)).status,
            400,
            "{roles}"
        );
    }
    let made = api.send(
        ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&json!({ "name": "x", "user": "bia" })),
    );
    let (key, id) = (key_of(&made), made.body["id"].as_str().unwrap().to_owned());
    let mut space = api.database.space();
    let mut stored = space.find(SECURITY_API_KEY, &Value::Text(id)).unwrap().unwrap();
    stored.set("expires", "amanhã");
    space.save(stored).unwrap();
    space.commit().unwrap();
    assert_eq!(api.send(ApiRequest::new("GET", "/api/me").bearer(&key)).status, 401);
}

#[test]
fn someone_turned_off_loses_settings_too_and_a_key_has_no_preferences() {
    let mut entities = vec![order(), item(), audit_entity(), ravenrs_app::settings_entity()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).unwrap();
    UserDirectory::new(maker(&database), roles()).unwrap();
    let mut space = database.space();
    user(&mut space, "ana", "Administradores", false);
    user(&mut space, "bia", "Sales", false);
    space.commit().unwrap();
    let service = ApiService::new(maker(&database), roles())
        .unwrap()
        .setting("theme", SettingAccess::PerPerson)
        .setting("banner", SettingAccess::Shared);
    let mut api = Api { service, database };
    let (ana, bia) = (api.token("ana"), api.token("bia"));

    let made = api.send(
        ApiRequest::new("POST", "/api/auth/keys").bearer(&ana).json(&json!({ "name": "x", "user": "bia" })),
    );
    let key = key_of(&made);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/settings/theme").bearer(&key)).status, 403);
    assert_eq!(api.send(ApiRequest::new("GET", "/api/settings/banner").bearer(&key)).status, 200);

    let mut space = api.database.space();
    let users = space.query(SECURITY_USER, &Query::all()).unwrap();
    let mut off = users.into_iter().find(|one| one.get("name") == Value::Text("bia".into())).unwrap();
    off.set("active", false);
    space.save(off).unwrap();
    space.commit().unwrap();
    for path in ["/api/settings/banner", "/api/settings/theme", "/api/me"] {
        assert_eq!(api.send(ApiRequest::new("GET", path).bearer(&bia)).status, 401, "{path}");
    }
}
