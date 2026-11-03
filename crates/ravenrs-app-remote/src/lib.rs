//! An application over the web API of `ravenrs-app-server`, on the desktop and in the browser.
//!
//! The screens read and write through an [`ObjectSpace`], which answers at once, and a browser
//! cannot wait for the network. So [`RemoteDatabase`] keeps a copy in memory of what the person
//! can see, screens work over that copy, and a commit goes to the server as one batch.
//!
//! A transport that answers before returning, as the one on the desktop does, makes a commit wait
//! for the server, and what it refuses is refused there and then. One that answers later, as in a
//! browser, keeps what was done in the copy at once and sets it right when the answer comes: what
//! the server refused goes away, and whoever watches is told with
//! [`on_problem`](RemoteDatabase::on_problem).
//!
//! The server decides everything. The copy only ever holds what the person can see, so nothing
//! here is a permission, and being taken apart gives nobody anything more.

mod gate;
mod transport;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

pub use gate::RemoteApp;
use ravenrs_app::{
    Change, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, MemoryObjectSpace, ModelError, Object,
    ObjectSpace, Query, SpaceError, SpaceWatcher, Value, object_from_json, value_to_json,
};
use serde_json::{Map, Value as Json, json};
pub use transport::{HttpTransport, RemoteRequest, RemoteResponse, Transport};

/// How many records to ask for at a time.
const PAGE: usize = 1000;

/// What runs when the copy changes.
type Listeners<T> = RefCell<Vec<Rc<T>>>;

/// What came back: the status and the body, or that nothing did.
type Answered = Result<(u16, Json), String>;

struct ClientInner {
    base: String,
    transport: Rc<dyn Transport>,
    token: RefCell<Option<String>>,
}

/// Talks to a server, holding the token of whoever signed in.
#[derive(Clone)]
pub struct RemoteClient(Rc<ClientInner>);

impl std::fmt::Debug for RemoteClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteClient").field("base", &self.0.base).finish_non_exhaustive()
    }
}

/// Why signing in did not work.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignInError {
    /// The name or the password is not right.
    Rejected,
    /// Too many tries, or the server could not be reached, in its own words.
    Other(String),
}

/// How signing in went.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SignedIn {
    /// They have to choose a new password before anything else.
    pub must_change_password: bool,
}

fn message_of(body: &Json) -> String {
    let violations: Vec<&str> = body
        .get("violations")
        .and_then(Json::as_array)
        .map(|found| found.iter().filter_map(|one| one.get("message")?.as_str()).collect())
        .unwrap_or_default();
    if !violations.is_empty() {
        return violations.join("\n");
    }
    body.get("message").and_then(Json::as_str).unwrap_or("The server refused this.").to_owned()
}

fn space_error(status: u16, body: &Json, denied: &str) -> SpaceError {
    let message = message_of(body);
    match status {
        400 => SpaceError::BadQuery(message),
        401 | 403 => SpaceError::Denied(denied.to_owned()),
        404 | 409 => SpaceError::Conflict(message),
        422 => SpaceError::Refused(message),
        _ => SpaceError::Storage(message),
    }
}

impl RemoteClient {
    /// A client of the server at this address, over this.
    pub fn new(base: impl Into<String>, transport: impl Transport + 'static) -> Self {
        let base = base.into().trim_end_matches('/').to_owned();
        Self(Rc::new(ClientInner { base, transport: Rc::new(transport), token: RefCell::new(None) }))
    }

    /// A client of the server at this address, over HTTP.
    pub fn http(base: impl Into<String>) -> Self {
        Self::new(base, HttpTransport)
    }

    fn ask(&self, method: &str, path: &str, body: Option<&Json>, done: impl FnOnce(Answered) + 'static) {
        let request = RemoteRequest {
            method: method.to_owned(),
            url: format!("{}{path}", self.0.base),
            bearer: self.0.token.borrow().clone(),
            body: body.map(Json::to_string),
        };
        self.0.transport.send(
            request,
            Box::new(move |answer| {
                done(answer.map(|response| {
                    (response.status, serde_json::from_str(&response.body).unwrap_or(Json::Null))
                }));
            }),
        );
    }

    /// Signs in, keeping the token for what comes next.
    pub fn sign_in(
        &self,
        name: &str,
        password: &str,
        done: impl FnOnce(Result<SignedIn, SignInError>) + 'static,
    ) {
        let client = self.clone();
        let body = json!({ "name": name, "password": password });
        self.ask("POST", "/api/auth/token", Some(&body), move |answer| match answer {
            Ok((200, body)) => {
                *client.0.token.borrow_mut() = body.get("token").and_then(Json::as_str).map(str::to_owned);
                let must_change_password =
                    body.get("must_change_password").and_then(Json::as_bool).unwrap_or(false);
                done(Ok(SignedIn { must_change_password }));
            }
            Ok((401, _)) => done(Err(SignInError::Rejected)),
            Ok((_, body)) => done(Err(SignInError::Other(message_of(&body)))),
            Err(why) => done(Err(SignInError::Other(why))),
        });
    }

    /// Changes the password of whoever signed in.
    pub fn change_password(&self, current: &str, new: &str, done: impl FnOnce(Result<(), String>) + 'static) {
        let body = json!({ "current": current, "new": new });
        self.ask("POST", "/api/auth/password", Some(&body), move |answer| match answer {
            Ok((200, _)) => done(Ok(())),
            Ok((_, body)) => done(Err(message_of(&body))),
            Err(why) => done(Err(why)),
        });
    }

    /// Ends the token, here and on the server.
    pub fn sign_out(&self) {
        self.ask("POST", "/api/auth/revoke", None, |_| {});
        *self.0.token.borrow_mut() = None;
    }
}

struct DatabaseInner {
    client: RemoteClient,
    copy: MemoryDatabase,
    entities: Vec<EntityInfo>,
    on_changed: Listeners<dyn Fn()>,
    on_problem: Listeners<dyn Fn(&str)>,
    denied: RefCell<String>,
}

/// A copy in memory of what the person who signed in can see, kept right with the server.
#[derive(Clone)]
pub struct RemoteDatabase(Rc<DatabaseInner>);

impl std::fmt::Debug for RemoteDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteDatabase").finish_non_exhaustive()
    }
}

impl RemoteDatabase {
    /// Over this client, for these entities. Nothing is read until [`load`](Self::load).
    pub fn new(client: RemoteClient, entities: Vec<EntityInfo>) -> Result<Self, ModelError> {
        let copy = MemoryDatabase::new(entities.clone())?;
        Ok(Self(Rc::new(DatabaseInner {
            client,
            copy,
            entities,
            on_changed: RefCell::default(),
            on_problem: RefCell::default(),
            denied: RefCell::new("You are not allowed to do this.".to_owned()),
        })))
    }

    /// What people are told when the server does not let them do something.
    pub fn denied_message(&self, message: impl Into<String>) {
        *self.0.denied.borrow_mut() = message.into();
    }

    /// Runs this whenever the copy changed from the server's side.
    pub fn on_changed(&self, run: impl Fn() + 'static) {
        self.0.on_changed.borrow_mut().push(Rc::new(run));
    }

    /// Runs this when the server refused something after the screen went on, with what it said.
    pub fn on_problem(&self, run: impl Fn(&str) + 'static) {
        self.0.on_problem.borrow_mut().push(Rc::new(run));
    }

    /// Reads all it can see. What the server does not offer to this person stays empty, as it is
    /// for them.
    pub fn load(&self, done: impl FnOnce(Result<(), String>) + 'static) {
        let names: Vec<String> = self
            .0
            .entities
            .iter()
            .filter(|entity| !entity.transient)
            .map(|entity| entity.name.clone())
            .collect();
        self.load_these(names, Box::new(done));
    }

    fn load_these(&self, mut names: Vec<String>, done: Box<dyn FnOnce(Result<(), String>)>) {
        let Some(name) = names.pop() else {
            done(Ok(()));
            return;
        };
        let database = self.clone();
        self.load_pages(
            name,
            0,
            Vec::new(),
            Box::new(move |loaded| match loaded {
                Ok(()) => database.load_these(names, done),
                Err(why) => done(Err(why)),
            }),
        );
    }

    fn load_pages(
        &self,
        name: String,
        skip: usize,
        mut gathered: Vec<Object>,
        done: Box<dyn FnOnce(Result<(), String>)>,
    ) {
        let database = self.clone();
        let path = format!("/api/data/{name}?top={PAGE}&skip={skip}");
        self.0.client.ask("GET", &path, None, move |answer| {
            let body = match answer {
                Ok((200, body)) => body,
                // Not for this person.
                Ok((403 | 404, _)) => Json::Null,
                Ok((_, body)) => return done(Err(message_of(&body))),
                Err(why) => return done(Err(why)),
            };
            let items = body.get("items").and_then(Json::as_array).cloned().unwrap_or_default();
            let count = items.len();
            if let Some(entity) = database.0.entities.iter().find(|entity| entity.name == name) {
                gathered.extend(
                    items.iter().filter_map(Json::as_object).map(|item| object_from_json(entity, item)),
                );
            }
            if count == PAGE {
                return database.load_pages(name, skip + PAGE, gathered, done);
            }
            database.0.copy.replace_records(&name, gathered);
            done(Ok(()));
        });
    }

    fn tell_changed(&self) {
        let watchers: Vec<_> = self.0.on_changed.borrow().clone();
        for watcher in watchers {
            watcher();
        }
    }

    fn tell_problem(&self, message: &str) {
        let watchers: Vec<_> = self.0.on_problem.borrow().clone();
        for watcher in watchers {
            watcher(message);
        }
    }

    /// A new unit of work.
    pub fn space(&self) -> RemoteObjectSpace {
        RemoteObjectSpace { database: self.clone(), inner: self.0.copy.space() }
    }

    fn entity(&self, name: &str) -> Result<&EntityInfo, SpaceError> {
        self.0
            .entities
            .iter()
            .find(|entity| entity.name == name)
            .ok_or_else(|| SpaceError::UnknownEntity(name.to_owned()))
    }
}

/// What the server takes for a field. The rest it works out, or does not want from here.
fn writable(entity: &EntityInfo, field: &FieldInfo) -> bool {
    let is_state = entity.state_machines.iter().any(|machine| machine.field == field.name);
    !field.secret
        && field.calculated.is_none()
        && !matches!(field.kind, FieldKind::Collection(_))
        && (!field.read_only || is_state)
}

/// A new record for the server, which gives numbered keys itself.
fn new_body(entity: &EntityInfo, object: &Object) -> Json {
    let mut body = Map::new();
    for field in entity.fields.iter().filter(|field| writable(entity, field)) {
        if field.is_key && matches!(field.kind, FieldKind::Integer) {
            continue;
        }
        let value = object.get(&field.name);
        if value != Value::Null {
            body.insert(field.name.clone(), value_to_json(&value));
        }
    }
    Json::Object(body)
}

/// Only what changed: what this person cannot see comes empty in the copy, and sending it back
/// would be asking to write it.
fn changed_body(entity: &EntityInfo, object: &Object, before: Option<&Object>) -> Map<String, Json> {
    let mut body = Map::new();
    for field in entity.fields.iter().filter(|field| writable(entity, field) && !field.is_key) {
        let value = object.get(&field.name);
        if before.is_none_or(|before| before.get(&field.name) != value) {
            body.insert(field.name.clone(), value_to_json(&value));
        }
    }
    body
}

/// A unit of work over a [`RemoteDatabase`]. A commit is one batch on the server.
pub struct RemoteObjectSpace {
    database: RemoteDatabase,
    inner: MemoryObjectSpace,
}

impl std::fmt::Debug for RemoteObjectSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteObjectSpace").finish_non_exhaustive()
    }
}

impl RemoteObjectSpace {
    /// What the changes are for the server, and which entities they touch.
    fn operations(&self) -> Result<(Vec<Json>, BTreeSet<String>), SpaceError> {
        let stored = self.database.0.copy.space();
        let mut operations = Vec::new();
        let mut touched = BTreeSet::new();
        for change in self.inner.changes() {
            match &change {
                Change::New(object) => {
                    let entity = self.database.entity(object.entity())?;
                    operations.push(json!({
                        "method": "POST",
                        "entity": entity.name,
                        "body": new_body(entity, object),
                    }));
                    touched.insert(entity.name.clone());
                }
                Change::Updated(object) => {
                    let entity = self.database.entity(object.entity())?;
                    let field =
                        entity.key_field().ok_or_else(|| SpaceError::MissingKey(entity.name.clone()))?;
                    let key = object.get(&field.name);
                    let before = stored.find(&entity.name, &key)?;
                    let body = changed_body(entity, object, before.as_ref());
                    if !body.is_empty() {
                        operations.push(json!({
                            "method": "PATCH",
                            "entity": entity.name,
                            "key": value_to_json(&key),
                            "body": body,
                        }));
                        touched.insert(entity.name.clone());
                    }
                }
                Change::Deleted { entity, key } => {
                    operations.push(json!({
                        "method": "DELETE",
                        "entity": entity,
                        "key": value_to_json(key),
                    }));
                    touched.insert(entity.clone());
                }
                _ => {}
            }
        }
        Ok((operations, touched))
    }
}

impl ObjectSpace for RemoteObjectSpace {
    fn entities(&self) -> Vec<EntityInfo> {
        self.inner.entities()
    }

    fn query(&self, entity: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        self.inner.query(entity, query)
    }

    fn find(&self, entity: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        self.inner.find(entity, key)
    }

    fn new_object(&mut self, entity: &str) -> Result<Object, SpaceError> {
        self.inner.new_object(entity)
    }

    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        self.inner.save(object)
    }

    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        self.inner.insert(object)
    }

    fn delete(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError> {
        self.inner.delete(entity, key)
    }

    fn changes(&self) -> Vec<Change> {
        self.inner.changes()
    }

    fn commit(&mut self) -> Result<(), SpaceError> {
        let (operations, touched) = self.operations()?;
        if operations.is_empty() {
            self.inner.rollback();
            return Ok(());
        }
        // Whether the answer came before this returned says which kind of transport this is.
        let at_once: Rc<Cell<Option<Result<(), SpaceError>>>> = Rc::new(Cell::new(None));
        let returned = Rc::new(Cell::new(false));
        let (slot, gone_on, database) = (at_once.clone(), returned.clone(), self.database.clone());
        let names = touched.clone();
        let body = json!({ "operations": operations });
        self.database.0.client.ask("POST", "/api/batch", Some(&body), move |answer| {
            let result = match answer {
                Ok((200, _)) => Ok(()),
                Ok((status, body)) => Err(space_error(status, &body, &database.0.denied.borrow())),
                Err(why) => Err(SpaceError::Storage(why)),
            };
            if !gone_on.get() {
                slot.set(Some(result));
                return;
            }
            if let Err(error) = &result {
                database.tell_problem(&error.to_string());
            }
            database.settle(names);
        });
        returned.set(true);
        match at_once.take() {
            Some(Err(error)) => Err(error),
            Some(Ok(())) => {
                // What the server keeps is what counts, with the keys it gave.
                if self.inner.commit().is_err() {
                    self.inner.rollback();
                }
                self.database.settle(touched);
                Ok(())
            }
            None => {
                // The screen goes on, and the answer sets it right.
                if self.inner.commit().is_err() {
                    self.inner.rollback();
                }
                Ok(())
            }
        }
    }

    fn rollback(&mut self) {
        self.inner.rollback();
    }

    fn refresh(&mut self) {
        self.inner.refresh();
    }

    fn on_event(&mut self, watcher: SpaceWatcher) {
        self.inner.on_event(watcher);
    }
}

impl RemoteDatabase {
    /// Reads again what a write touched, as the server left it, and says it changed.
    fn settle(&self, touched: BTreeSet<String>) {
        let database = self.clone();
        self.load_these(
            touched.into_iter().collect(),
            Box::new(move |loaded| {
                if let Err(why) = loaded {
                    database.tell_problem(&why);
                }
                database.tell_changed();
            }),
        );
    }
}
