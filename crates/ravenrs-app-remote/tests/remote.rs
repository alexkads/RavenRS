//! The client against the real service, with nothing but the network left out.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use ravenrs_app::{
    Access, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, ObjectSpace, Operation, PermissionPolicy,
    Query, Role, Rule, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SpaceError, SpaceMaker,
    UserDirectory, Value, hash_password, security_entities,
};
use ravenrs_app_remote::{RemoteApp, RemoteClient, RemoteDatabase, RemoteRequest, RemoteResponse, Transport};
use ravenrs_app_server::{ApiRequest, ApiService};
use ravenrs_app_ui::{App, Language};
use ravenrs_core::{AccessAction, Event, Key, Modifiers, Role as AccessRole, Size};
use ravenrs_criteria::DateTime;
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

const PASSWORD: &str = "uma-senha-de-teste";

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .caption("Pedido")
        .navigation("Geral")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(FieldInfo::new("owner", FieldKind::Text))
        .field(FieldInfo::new("cost", FieldKind::Money))
        .field(FieldInfo::new("day", FieldKind::Date))
        .rule(Rule::required("name"))
}

fn roles() -> Vec<Role> {
    vec![
        Role::new("Administradores").administrative(),
        Role::new("Sales")
            .policy(PermissionPolicy::AllowAll)
            .member_permission("Order", ["cost"], None, [Operation::Read, Operation::Write], Access::Deny)
            .object_permission("Order", "[owner] <> ?CurrentUserName", [Operation::Delete], Access::Deny),
    ]
}

fn server() -> (ApiService, MemoryDatabase) {
    let mut entities = vec![order()];
    entities.extend(security_entities());
    let database = MemoryDatabase::new(entities).unwrap();
    let making = database.clone();
    let space: SpaceMaker = Rc::new(move || Box::new(making.space()) as Box<dyn ObjectSpace>);
    UserDirectory::new(space.clone(), roles()).unwrap();
    let mut records = database.space();
    let found = records.query(SECURITY_ROLE, &Query::all()).unwrap();
    for (name, role, must_change) in [("bia", "Sales", false), ("dani", "Sales", true)] {
        let role = found.iter().find(|one| one.get("name") == Value::Text(role.into())).unwrap().get("id");
        let mut user = records.new_object(SECURITY_USER).unwrap();
        let key = user.get("id");
        user.set("name", name);
        user.set("password_hash", hash_password(PASSWORD).unwrap());
        user.set("must_change_password", must_change);
        records.save(user).unwrap();
        let mut link = records.new_object(SECURITY_USER_ROLE).unwrap();
        link.set("user_id", key);
        link.set("role_id", role);
        records.save(link).unwrap();
    }
    for (name, owner, cost) in [("Da ana", "ana", 900.0), ("Da bia", "bia", 500.0)] {
        let mut record = records.new_object("Order").unwrap();
        record.set("name", name);
        record.set("owner", owner);
        record.set("cost", Value::Number(cost));
        records.save(record).unwrap();
    }
    records.commit().unwrap();
    (ApiService::new(space, roles()).unwrap(), database)
}

fn asked(service: &Rc<RefCell<ApiService>>, request: &RemoteRequest) -> RemoteResponse {
    let address = request.url.split_once("://").map_or(request.url.as_str(), |(_, rest)| rest);
    let target = address.split_once('/').map_or("", |(_, rest)| rest);
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut made = ApiRequest::new(&request.method, path);
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap();
        made = made.query(name, value);
    }
    if let Some(token) = &request.bearer {
        made = made.bearer(token);
    }
    if let Some(body) = &request.body {
        made.body = body.clone().into_bytes();
    }
    let answer = service.borrow_mut().handle(made);
    RemoteResponse::new(answer.status, answer.body.to_string())
}

/// Answers before returning, as on the desktop.
struct AtOnce(Rc<RefCell<ApiService>>);

impl Transport for AtOnce {
    fn send(&self, request: RemoteRequest, done: Box<dyn FnOnce(Result<RemoteResponse, String>)>) {
        done(Ok(asked(&self.0, &request)));
    }
}

type Waiting = VecDeque<(RemoteRequest, Box<dyn FnOnce(Result<RemoteResponse, String>)>)>;

/// Answers when told to, as a browser does.
#[derive(Clone)]
struct Later(Rc<RefCell<ApiService>>, Rc<RefCell<Waiting>>);

impl Transport for Later {
    fn send(&self, request: RemoteRequest, done: Box<dyn FnOnce(Result<RemoteResponse, String>)>) {
        self.1.borrow_mut().push_back((request, done));
    }
}

impl Later {
    fn answer_all(&self) {
        loop {
            let next = self.1.borrow_mut().pop_front();
            let Some((request, done)) = next else { return };
            done(Ok(asked(&self.0, &request)));
        }
    }
}

fn entities() -> Vec<EntityInfo> {
    let mut all = vec![order()];
    all.extend(security_entities());
    all
}

fn signed_in(client: &RemoteClient, name: &str) -> RemoteDatabase {
    client.sign_in(name, PASSWORD, |result| {
        result.unwrap();
    });
    let database = RemoteDatabase::new(client.clone(), entities()).unwrap();
    database.load(|result| result.unwrap());
    database
}

fn by_name(database: &RemoteDatabase, name: &str) -> ravenrs_app::Object {
    let all = database.space().query("Order", &Query::all()).unwrap();
    all.into_iter().find(|one| one.get("name") == Value::Text(name.into())).unwrap()
}

fn desktop() -> (RemoteClient, MemoryDatabase) {
    let (service, stored) = server();
    (RemoteClient::new("http://server", AtOnce(Rc::new(RefCell::new(service)))), stored)
}

#[test]
fn a_wrong_password_gets_nothing_and_says_so() {
    let (client, _) = desktop();
    let said = Rc::new(RefCell::new(None));
    let keep = said.clone();
    client.sign_in("bia", "errada", move |result| *keep.borrow_mut() = Some(result));
    assert!(said.borrow().as_ref().unwrap().is_err());
    let database = RemoteDatabase::new(client, entities()).unwrap();
    let loaded = Rc::new(RefCell::new(None));
    let keep = loaded.clone();
    database.load(move |result| *keep.borrow_mut() = Some(result));
    assert!(loaded.borrow().as_ref().unwrap().is_err(), "without a token it reads nothing");
}

#[test]
fn the_copy_holds_only_what_the_person_can_see() {
    let (client, _) = desktop();
    let database = signed_in(&client, "bia");
    let found = database.space().query("Order", &Query::all()).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].get("cost"), Value::Null, "denied");
    assert!(database.space().query(SECURITY_USER, &Query::all()).unwrap().is_empty(), "not for clients");
}

#[test]
fn a_commit_that_waits_comes_back_with_what_the_server_kept() {
    let (client, stored) = desktop();
    let database = signed_in(&client, "bia");
    let mut space = database.space();
    let mut record = space.new_object("Order").unwrap();
    record.set("name", "Novo");
    record.set("owner", "bia");
    record.set("day", Value::Date(parse_day()));
    space.insert(record).unwrap();
    space.commit().unwrap();
    assert!(!space.is_modified());
    let kept = stored.space().query("Order", &Query::all()).unwrap();
    assert_eq!(kept.len(), 3);
    let seen = by_name(&database, "Novo");
    let server_key = kept.iter().find(|one| one.get("name") == Value::Text("Novo".into())).unwrap().get("id");
    assert_eq!(seen.get("id"), server_key, "with the key the server gave");
    assert_eq!(seen.get("day"), Value::Date(parse_day()));
}

fn parse_day() -> DateTime {
    DateTime::parse("2026-11-03").unwrap()
}

#[test]
fn changing_a_record_sends_only_what_changed_so_what_is_hidden_stays() {
    let (client, stored) = desktop();
    let database = signed_in(&client, "bia");
    let mut record = by_name(&database, "Da ana");
    let key = record.get("id");
    record.set("name", "Mudado");
    let mut space = database.space();
    space.save(record).unwrap();
    space.commit().unwrap();
    let kept = stored.space().find("Order", &key).unwrap().unwrap();
    assert_eq!(kept.get("name"), Value::Text("Mudado".into()));
    assert_eq!(kept.get("cost"), Value::Number(900.0), "it never saw it, and it stays");
}

#[test]
fn what_the_server_refuses_is_refused_there_and_then_and_can_be_put_right() {
    let (client, stored) = desktop();
    let database = signed_in(&client, "bia");

    // The server's rule, with the screens not asked.
    let mut space = database.space();
    let mut empty = space.new_object("Order").unwrap();
    empty.set("owner", "bia");
    space.insert(empty).unwrap();
    let error = space.commit().unwrap_err();
    assert!(matches!(error, SpaceError::Refused(_)), "{error:?}");
    assert!(space.is_modified(), "what was typed is still there");
    space.rollback();

    // Someone else's record, and a field denied.
    let of_ana = by_name(&database, "Da ana");
    let mut space = database.space();
    space.delete("Order", &of_ana.get("id")).unwrap();
    assert!(matches!(space.commit().unwrap_err(), SpaceError::Denied(_)));
    assert_eq!(stored.space().query("Order", &Query::all()).unwrap().len(), 2);
    assert_eq!(database.space().query("Order", &Query::all()).unwrap().len(), 2);

    let mut mine = by_name(&database, "Da bia");
    mine.set("cost", Value::Number(1.0));
    let mut space = database.space();
    space.save(mine).unwrap();
    assert!(matches!(space.commit().unwrap_err(), SpaceError::Denied(_)));
}

#[test]
fn a_change_of_nothing_asks_nothing() {
    let (client, _) = desktop();
    let database = signed_in(&client, "bia");
    let same = by_name(&database, "Da bia");
    let mut space = database.space();
    space.save(same).unwrap();
    space.commit().unwrap();
    assert!(!space.is_modified());
}

#[test]
fn records_past_one_page_all_come() {
    let (service, stored) = server();
    let mut records = stored.space();
    for index in 0..1100 {
        let mut record = records.new_object("Order").unwrap();
        record.set("name", format!("N{index}"));
        records.save(record).unwrap();
    }
    records.commit().unwrap();
    let client = RemoteClient::new("http://server", AtOnce(Rc::new(RefCell::new(service))));
    let database = signed_in(&client, "bia");
    assert_eq!(database.space().query("Order", &Query::all()).unwrap().len(), 1102);
}

fn browser() -> (RemoteClient, Later, MemoryDatabase) {
    let (service, stored) = server();
    let later = Later(Rc::new(RefCell::new(service)), Rc::default());
    (RemoteClient::new("http://server", later.clone()), later, stored)
}

#[test]
fn in_a_browser_the_screen_goes_on_and_the_answer_sets_it_right() {
    let (client, later, stored) = browser();
    client.sign_in("bia", PASSWORD, |result| {
        result.unwrap();
    });
    later.answer_all();
    let database = RemoteDatabase::new(client, entities()).unwrap();
    database.load(|result| result.unwrap());
    later.answer_all();
    let changed = Rc::new(RefCell::new(0));
    let keep = changed.clone();
    database.on_changed(move || *keep.borrow_mut() += 1);

    let mut space = database.space();
    let mut record = space.new_object("Order").unwrap();
    record.set("name", "Agora");
    space.insert(record).unwrap();
    space.commit().unwrap();
    assert_eq!(by_name(&database, "Agora").get("name"), Value::Text("Agora".into()), "at once");
    assert_eq!(stored.space().query("Order", &Query::all()).unwrap().len(), 2, "not there yet");

    later.answer_all();
    assert_eq!(stored.space().query("Order", &Query::all()).unwrap().len(), 3);
    assert_eq!(*changed.borrow(), 1);
    let kept = stored.space().query("Order", &Query::all()).unwrap();
    let key = kept.iter().find(|one| one.get("name") == Value::Text("Agora".into())).unwrap().get("id");
    assert_eq!(by_name(&database, "Agora").get("id"), key);
}

#[test]
fn in_a_browser_what_the_server_refuses_goes_away_and_whoever_watches_is_told() {
    let (client, later, stored) = browser();
    client.sign_in("bia", PASSWORD, |result| {
        result.unwrap();
    });
    later.answer_all();
    let database = RemoteDatabase::new(client, entities()).unwrap();
    database.load(|result| result.unwrap());
    later.answer_all();
    let told = Rc::new(RefCell::new(Vec::new()));
    let keep = told.clone();
    database.on_problem(move |why| keep.borrow_mut().push(why.to_owned()));

    let of_ana = by_name(&database, "Da ana");
    let mut space = database.space();
    space.delete("Order", &of_ana.get("id")).unwrap();
    space.commit().unwrap();
    assert_eq!(database.space().query("Order", &Query::all()).unwrap().len(), 1, "gone from the screen");
    later.answer_all();
    assert_eq!(told.borrow().len(), 1, "{told:?}");
    assert_eq!(stored.space().query("Order", &Query::all()).unwrap().len(), 2);
    assert_eq!(database.space().query("Order", &Query::all()).unwrap().len(), 2, "back, as it never went");
}

#[test]
fn signing_out_ends_the_token() {
    let (client, _) = desktop();
    let database = signed_in(&client, "bia");
    client.sign_out();
    let loaded = Rc::new(RefCell::new(None));
    let keep = loaded.clone();
    database.load(move |result| *keep.borrow_mut() = Some(result));
    assert!(loaded.borrow().as_ref().unwrap().is_err());
}

struct Window(Ui);

impl Window {
    fn settle(&mut self) {
        for _ in 0..4 {
            self.0.update();
        }
    }

    fn says(&mut self, text: &str) -> bool {
        self.settle();
        self.0.access_tree().nodes.iter().any(|node| {
            node.name.as_deref().is_some_and(|name| name.contains(text))
                || node.value.as_deref().is_some_and(|value| value.contains(text))
        })
    }

    fn press(&mut self, name: &str) {
        self.settle();
        let tree = self.0.access_tree();
        let found = tree
            .nodes
            .iter()
            .find(|node| node.role == AccessRole::Button && node.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no {name}")).id;
        self.0.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn type_into(&mut self, name: &str, text: &str) {
        self.settle();
        let tree = self.0.access_tree();
        let found = tree
            .nodes
            .iter()
            .find(|node| node.role == AccessRole::TextInput && node.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no {name}")).id;
        self.0.perform_action(id, AccessAction::Focus);
        self.settle();
        self.0.handle_event(&Event::KeyDown {
            key: Key::Character(text.to_owned()),
            modifiers: Modifiers::default(),
        });
        self.settle();
    }
}

fn window(client: RemoteClient) -> Window {
    let view = RemoteApp::view("Pela rede", Language::BrazilianPortuguese, client, entities(), |database| {
        let records = database.clone();
        App::new("Pela rede")
            .language(Language::BrazilianPortuguese)
            .entity_info(order())
            .storage(move || records.space())
    })
    .unwrap();
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    Window(ui)
}

#[test]
fn the_window_asks_who_it_is_and_opens_on_what_the_server_has() {
    let (client, _) = desktop();
    let mut window = window(client);
    window.type_into("Nome", "bia");
    window.type_into("Senha", "errada");
    window.press("Entrar");
    assert!(window.says("O nome ou a senha"), "in the words of the screens");
    assert!(!window.says("Da bia"));

    let mut opened = self::window(desktop().0);
    opened.type_into("Nome", "bia");
    opened.type_into("Senha", PASSWORD);
    opened.press("Entrar");
    assert!(opened.says("Da bia"));
    assert!(opened.says("Da ana"));
    assert!(opened.says("Pela rede"));
    opened.press("Sair");
    assert!(opened.says("Entrar"), "back at the door");
    assert!(!opened.says("Da bia"));
}

#[test]
fn someone_who_has_to_change_their_password_does_that_first() {
    let (client, _) = desktop();
    let mut window = window(client);
    window.type_into("Nome", "dani");
    window.type_into("Senha", PASSWORD);
    window.press("Entrar");
    assert!(!window.says("Da bia"));
    window.type_into("Senha atual", PASSWORD);
    window.type_into("Nova senha", "uma-senha-bem-nova");
    window.press("Trocar senha");
    assert!(window.says("Da bia"));
}
