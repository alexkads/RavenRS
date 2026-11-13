#![allow(clippy::unwrap_used, clippy::panic)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use ravenrs_app::{Criteria, EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query, SpaceError, Value};

use super::*;

/// Stands in for PostgREST: answers in the order given, and keeps what it was asked.
#[derive(Clone, Default)]
struct Fake {
    answers: Rc<RefCell<VecDeque<Result<HttpResponse, HttpError>>>>,
    asked: Rc<RefCell<Vec<HttpRequest>>>,
}

impl Fake {
    fn answer(&self, status: u16, body: &str) -> &Self {
        self.answers.borrow_mut().push_back(Ok(HttpResponse::new(status, body)));
        self
    }

    fn fail(&self, error: HttpError) -> &Self {
        self.answers.borrow_mut().push_back(Err(error));
        self
    }

    fn asked(&self) -> Vec<HttpRequest> {
        self.asked.borrow().clone()
    }
}

impl Http for Fake {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.asked.borrow_mut().push(request.clone());
        self.answers.borrow_mut().pop_front().unwrap_or_else(|| Ok(HttpResponse::new(200, "[]")))
    }
}

fn plain() -> EntityInfo {
    EntityInfo::new("Plain")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome"))
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("day", FieldKind::Date))
        .field(FieldInfo::new("twice", FieldKind::Integer).calculated("[seats] * 2"))
}

fn database(fake: &Fake) -> PostgrestDatabase {
    PostgrestDatabase::builder("https://example.supabase.co/rest/v1/")
        .api_key("public-key")
        .bearer("the-token")
        .schema("api")
        .http(fake.clone())
        .page_size(2)
        .resource(plain(), ResourceMap::new("plain").column("active", "is_active"))
        .unwrap()
        .build()
}

#[test]
fn a_filter_postgrest_can_say_goes_to_it_with_order_page_and_names() {
    let fake = Fake::default();
    fake.answer(200, r#"[{"id":2,"name":"Beta","seats":2,"active":false,"day":"2026-11-13"}]"#);
    let space = database(&fake).space();
    let query = Query::all()
        .filter(Criteria::parse("[seats] > 1 And [name] Like 'B%'").unwrap())
        .sort("active", false)
        .page(0, 1);
    let found = space.query("Plain", &query).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("active"), Value::Bool(false));
    assert_eq!(found[0].get("day"), Value::Date(DateTime::from_ymd(2026, 11, 13).unwrap()));
    assert_eq!(found[0].get("twice"), Value::Number(4.0), "calculated after reading");

    let asked = fake.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(
        asked[0].url,
        "https://example.supabase.co/rest/v1/plain?select=id,name,seats,active:is_active,day\
         &and=(seats.gt.1,name.like.%22B*%22)&order=is_active.desc,id.asc&limit=1&offset=0"
    );
    assert_eq!(asked[0].header("apikey"), Some("public-key"));
    assert_eq!(asked[0].header("Authorization"), Some("Bearer the-token"));
    assert_eq!(asked[0].header("Accept-Profile"), Some("api"));
}

#[test]
fn reading_goes_page_by_page_until_an_empty_one() {
    let fake = Fake::default();
    // The second page is short, as the server's max-rows makes it; there is more after it.
    fake.answer(200, r#"[{"id":1},{"id":2}]"#).answer(200, r#"[{"id":3}]"#).answer(200, r#"[{"id":4}]"#);
    let found = database(&fake).space().query("Plain", &Query::all()).unwrap();
    assert_eq!(found.len(), 4);
    let asked: Vec<String> =
        fake.asked().iter().map(|request| request.url.rsplit('?').next().unwrap().to_owned()).collect();
    assert_eq!(asked.len(), 4, "the last one came back empty");
    assert!(asked[1].ends_with("limit=2&offset=2") && asked[2].ends_with("limit=2&offset=3"), "{asked:?}");

    let fake = Fake::default();
    fake.answer(200, r#"[{"id":1},{"id":2}]"#).answer(200, r#"[{"id":3}]"#);
    let page = database(&fake).space().query("Plain", &Query::all().page(0, 3)).unwrap();
    assert_eq!(page.len(), 3);
    assert_eq!(fake.asked().len(), 2, "nothing asked past the limit");
}

#[test]
fn what_postgrest_cannot_say_is_filtered_in_memory() {
    let fake = Fake::default();
    fake.answer(200, r#"[{"id":1,"seats":1},{"id":2,"seats":3}]"#).answer(200, "[]");
    let space = database(&fake).space();
    let found = space.query("Plain", &Query::all().filter(Criteria::parse("[twice] > 4").unwrap())).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].get("id"), Value::Number(2.0));
    assert!(
        fake.asked().iter().all(|request| !request.url.contains("and=")),
        "nothing filtered on the server"
    );
}

#[test]
fn a_commit_is_one_call_with_every_change_by_column() {
    let fake = Fake::default();
    // The next key, whether the new key exists, whether the one to delete does, then the commit.
    fake.answer(200, r#"[{"id":7}]"#)
        .answer(200, "[]")
        .answer(200, r#"[{"id":3}]"#)
        .answer(200, r#"{"applied":2}"#);
    let mut space = database(&fake).space();
    let mut object = space.new_object("Plain").unwrap();
    assert_eq!(object.get("id"), Value::Number(8.0));
    object.set("name", "Oito");
    object.set("active", true);
    space.save(object).unwrap();
    space.delete("Plain", &Value::Number(3.0)).unwrap();
    space.commit().unwrap();
    assert!(!space.is_modified());

    let commit = fake.asked().pop().unwrap();
    assert_eq!(commit.method, "POST");
    assert_eq!(commit.url, "https://example.supabase.co/rest/v1/rpc/ravenrs_commit");
    assert_eq!(commit.header("Content-Profile"), Some("api"));
    let body: Json = serde_json::from_str(commit.body.as_deref().unwrap()).unwrap();
    let changes = body["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["op"], "insert");
    assert_eq!(changes[0]["resource"], "plain");
    assert_eq!(changes[0]["key_column"], "id");
    assert_eq!(changes[0]["key"], 8);
    assert_eq!(changes[0]["values"]["is_active"], true, "by column");
    assert_eq!(changes[0]["values"]["name"], "Oito");
    assert!(changes[0]["values"].get("twice").is_none(), "a calculated field has no column");
    assert_eq!(changes[1]["op"], "delete");
    assert_eq!(changes[1]["key"], 3);
}

fn commit_with(fake: &Fake) -> Result<(), SpaceError> {
    let mut space = database(fake).space();
    let mut object = space.new_object("Plain").unwrap();
    object.set("name", "Um");
    space.save(object).unwrap();
    space.commit()
}

#[test]
fn what_the_server_refuses_comes_back_in_words_for_people() {
    let refused = |status: u16, body: &str| {
        let fake = Fake::default();
        fake.answer(200, "[]").answer(200, "[]").answer(status, body);
        commit_with(&fake).unwrap_err()
    };
    assert_eq!(
        refused(
            409,
            r#"{"code":"23505","message":"duplicate key","details":"Key (name)=(Um) already exists."}"#
        ),
        SpaceError::Refused("There is already a record with this Nome".into())
    );
    assert!(matches!(refused(409, r#"{"code":"23503","message":"fk"}"#), SpaceError::Refused(_)));
    assert!(matches!(
        refused(409, r#"{"code":"PT409","message":"a record of plain is gone"}"#),
        SpaceError::Conflict(_)
    ));
    assert!(matches!(
        refused(403, r#"{"code":"42501","message":"permission denied"}"#),
        SpaceError::Denied(_)
    ));
    assert!(matches!(refused(401, r#"{"code":"PGRST301","message":"JWT expired"}"#), SpaceError::Denied(_)));
    assert!(matches!(
        refused(404, r#"{"code":"PGRST202","message":"no function"}"#),
        SpaceError::Unavailable(_)
    ));
    assert!(matches!(
        refused(503, r#"{"code":"PGRST001","message":"no connection"}"#),
        SpaceError::Temporary(_)
    ));
}

#[test]
fn a_commit_with_no_answer_may_have_happened_and_is_not_sent_again() {
    let fake = Fake::default();
    fake.answer(200, "[]").answer(200, "[]").fail(HttpError::NoAnswer("connection reset".into()));
    let error = commit_with(&fake).unwrap_err();
    assert!(error.may_have_happened(), "{error:?}");
    assert_eq!(fake.asked().iter().filter(|request| request.method == "POST").count(), 1);

    let fake = Fake::default();
    fake.answer(200, "[]").answer(200, "[]").fail(HttpError::NotSent("connection refused".into()));
    let error = commit_with(&fake).unwrap_err();
    assert!(error.is_temporary() && !error.may_have_happened(), "{error:?}");
}

#[test]
fn the_token_and_the_key_stay_out_of_what_is_shown() {
    let fake = Fake::default();
    fake.answer(200, "[]");
    let database = database(&fake);
    database.space().query("Plain", &Query::all()).unwrap();
    let shown = format!("{database:?} {:?}", fake.asked());
    assert!(!shown.contains("the-token") && !shown.contains("public-key"), "{shown}");
}

#[test]
fn the_token_is_asked_for_at_each_request() {
    let fake = Fake::default();
    let turn = Rc::new(RefCell::new(0));
    let counted = turn.clone();
    let database = PostgrestDatabase::builder("http://localhost:3000")
        .http(fake.clone())
        .token_source(Rc::new(move || {
            *counted.borrow_mut() += 1;
            Some(format!("token-{}", counted.borrow()))
        }))
        .resource(plain(), ResourceMap::new("plain"))
        .unwrap()
        .build();
    let space = database.space();
    space.query("Plain", &Query::all()).unwrap();
    space.query("Plain", &Query::all()).unwrap();
    let tokens: Vec<String> =
        fake.asked().iter().map(|request| request.header("Authorization").unwrap().to_owned()).collect();
    assert_eq!(tokens, ["Bearer token-1", "Bearer token-2"]);
}
