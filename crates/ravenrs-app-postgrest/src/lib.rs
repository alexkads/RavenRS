//! Records in PostgreSQL through PostgREST, as an [`ObjectSpace`]: the server of your own, the one
//! in Supabase, or Neon's Data API. The station holds the token of the person signed in, never the
//! database's password, and the database's grants and row level security decide what it sees.
//!
//! - Reads go as `GET /{resource}` with the filter in PostgREST's words (see the `filter` module),
//!   page after page until an empty one, so the server's `max-rows` cuts nothing. What its words cannot say is read and
//!   filtered in memory, as the SQL spaces do.
//! - A commit is one call, `POST /rpc/ravenrs_commit`, of a function that applies every change in
//!   one transaction as whoever called it. Install it with [`SERVER_SQL`].
//! - What waits for a commit, the rules, state machines and calculated fields work as in the SQL
//!   spaces, through the same pieces of `ravenrs-app`.
//!
//! Only for the desktop for now: a space answers at once, and a browser cannot wait.

mod filter;
mod http;
mod server;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::rc::Rc;

use ravenrs_app::{
    Change, ChangeSet, EntityInfo, FieldInfo, FieldKind, Object, ObjectSpace, Query, SpaceError, SpaceEvent,
    SpaceWatcher, Value, check_query, check_transitions, key_of, key_text, run_query, value_to_json,
};
pub use ravenrs_app_sql::SqlWords;
use ravenrs_criteria::{DateTime, Environment};
use serde_json::{Map, Value as Json, json};

pub use crate::http::{Http, HttpError, HttpRequest, HttpResponse, UreqHttp};
pub use crate::server::SERVER_SQL;

/// The function a commit calls; see [`SERVER_SQL`].
pub const COMMIT_FUNCTION: &str = "ravenrs_commit";

/// How an entity lands on a resource: a table or a view PostgREST serves.
#[derive(Clone, Debug)]
pub struct ResourceMap {
    resource: String,
    columns: BTreeMap<String, String>,
}

impl ResourceMap {
    /// On this resource, with each field in a column of its name.
    pub fn new(resource: impl Into<String>) -> Self {
        Self { resource: resource.into(), columns: BTreeMap::new() }
    }

    /// This field is in a column by another name.
    pub fn column(mut self, field: impl Into<String>, column: impl Into<String>) -> Self {
        self.columns.insert(field.into(), column.into());
        self
    }

    fn column_of<'a>(&'a self, field: &'a str) -> &'a str {
        self.columns.get(field).map_or(field, String::as_str)
    }
}

struct Entity {
    info: EntityInfo,
    map: ResourceMap,
}

impl Entity {
    /// The fields that have a column.
    fn stored(&self) -> impl Iterator<Item = &FieldInfo> {
        self.info
            .fields
            .iter()
            .filter(|f| f.calculated.is_none() && !matches!(f.kind, FieldKind::Collection(_)))
    }

    fn columns(&self) -> filter::Columns {
        self.stored().map(|f| (f.name.clone(), self.map.column_of(&f.name).to_owned())).collect()
    }

    /// `select=` with each field under its own name: `name,active:is_active`.
    fn select(&self, only: Option<&[String]>) -> String {
        let key = self.info.key_field().map(|f| f.name.as_str());
        self.stored()
            .filter(|f| only.is_none_or(|only| only.contains(&f.name) || Some(f.name.as_str()) == key))
            .map(|f| {
                let column = self.map.column_of(&f.name);
                if column == f.name { column.to_owned() } else { format!("{}:{column}", f.name) }
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn read(&self, row: &Map<String, Json>) -> Object {
        let mut values = BTreeMap::new();
        for (name, sent) in row {
            let Some(field) = self.info.field_named(name) else { continue };
            let value = match (sent, &field.kind) {
                (Json::Null, _) => Value::Null,
                (Json::Bool(on), _) => Value::Bool(*on),
                (Json::Number(number), FieldKind::Boolean) => {
                    Value::Bool(number.as_f64().is_some_and(|n| n != 0.0))
                }
                (Json::Number(number), _) => number.as_f64().map_or(Value::Null, Value::Number),
                (Json::String(text), FieldKind::Date | FieldKind::DateTime) => moment(text),
                (Json::String(text), _) => Value::Text(text.clone()),
                (other, _) => Value::Text(other.to_string()),
            };
            values.insert(name.clone(), value);
        }
        Object::from_values(&self.info.name, values)
    }

    /// The columns of a change, by column name, as the function reads them.
    fn values(&self, object: &Object, with_key: bool) -> Map<String, Json> {
        self.stored()
            .filter(|f| with_key || !f.is_key)
            .map(|f| (self.map.column_of(&f.name).to_owned(), value_to_json(&object.get(&f.name))))
            .collect()
    }
}

/// A date as PostgREST writes it. `timestamptz` comes with its offset, in the session's time
/// zone (UTC unless the server says otherwise); `DateTime` has none, so the offset is left out.
fn moment(text: &str) -> Value {
    DateTime::parse(text)
        .or_else(|| text.get(..19).and_then(DateTime::parse))
        .or_else(|| text.get(..10).and_then(DateTime::parse))
        .map_or_else(|| Value::Text(text.to_owned()), Value::Date)
}

/// Gives the token of the person signed in for each request, so one that was renewed is used at
/// once. `None` goes with no token, as the anonymous role.
pub type TokenSource = Rc<dyn Fn() -> Option<String>>;

struct Shared {
    url: String,
    schema: Option<String>,
    api_key: Option<String>,
    token: Option<TokenSource>,
    http: Box<dyn Http>,
    entities: Vec<Entity>,
    words: SqlWords,
    page_size: usize,
    next_keys: RefCell<HashMap<String, i64>>,
}

/// Records through PostgREST. Any number of spaces share it.
#[derive(Clone)]
pub struct PostgrestDatabase(Rc<Shared>);

impl fmt::Debug for PostgrestDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The key and the token stay out.
        f.debug_struct("PostgrestDatabase")
            .field("url", &self.0.url)
            .field("schema", &self.0.schema)
            .field("entities", &self.0.entities.len())
            .finish_non_exhaustive()
    }
}

/// Puts a [`PostgrestDatabase`] together.
pub struct PostgrestBuilder {
    url: String,
    schema: Option<String>,
    api_key: Option<String>,
    token: Option<TokenSource>,
    http: Box<dyn Http>,
    entities: Vec<Entity>,
    words: SqlWords,
    page_size: usize,
}

impl fmt::Debug for PostgrestBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgrestBuilder").field("url", &self.url).finish_non_exhaustive()
    }
}

impl PostgrestBuilder {
    /// The schema to use when PostgREST serves more than one (`Accept-Profile`).
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// The project's public key, which Supabase asks for in `apikey` on every request.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Always this token.
    pub fn bearer(self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.token_source(Rc::new(move || Some(token.clone())))
    }

    /// The token as it is at each request, for one that is renewed.
    pub fn token_source(mut self, source: TokenSource) -> Self {
        self.token = Some(source);
        self
    }

    /// Carries requests another way: a proxy, a test.
    pub fn http(mut self, http: impl Http + 'static) -> Self {
        self.http = Box::new(http);
        self
    }

    /// What to say when the database refuses something.
    pub fn words(mut self, words: SqlWords) -> Self {
        self.words = words;
        self
    }

    /// How many records to ask for at a time. Keep it at or under the server's `max-rows`.
    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size = size.max(1);
        self
    }

    /// An entity, on a resource.
    ///
    /// # Errors
    ///
    /// When the entity has no key: records are found and changed by it.
    pub fn resource(mut self, info: EntityInfo, map: ResourceMap) -> Result<Self, SpaceError> {
        if info.key_field().is_none() {
            return Err(SpaceError::MissingKey(info.name));
        }
        self.entities.push(Entity { info, map });
        Ok(self)
    }

    /// The database.
    pub fn build(self) -> PostgrestDatabase {
        PostgrestDatabase(Rc::new(Shared {
            url: self.url,
            schema: self.schema,
            api_key: self.api_key,
            token: self.token,
            http: self.http,
            entities: self.entities,
            words: self.words,
            page_size: self.page_size,
            next_keys: RefCell::default(),
        }))
    }
}

impl PostgrestDatabase {
    /// Starts one on the address PostgREST answers on, such as `https://<project>.supabase.co/rest/v1`.
    pub fn builder(url: impl Into<String>) -> PostgrestBuilder {
        PostgrestBuilder {
            url: url.into().trim_end_matches('/').to_owned(),
            schema: None,
            api_key: None,
            token: None,
            http: Box::new(UreqHttp::default()),
            entities: Vec::new(),
            words: SqlWords::default(),
            page_size: 1000,
        }
    }

    /// A new unit of work.
    pub fn space(&self) -> PostgrestObjectSpace {
        PostgrestObjectSpace {
            shared: self.0.clone(),
            environment: ravenrs_app::environment(),
            changes: ChangeSet::default(),
            watchers: Vec::new(),
        }
    }

    /// The entities it knows.
    pub fn entities(&self) -> Vec<EntityInfo> {
        self.0.entities.iter().map(|entity| entity.info.clone()).collect()
    }

    /// Forgets the next keys it handed out, so they are counted again from what is stored.
    pub fn forget_next_keys(&self) {
        self.0.next_keys.borrow_mut().clear();
    }
}

/// Encodes a query value: what is not a letter, a digit or one of `-_.~,()*:` goes as `%XX`,
/// quotes among it.
fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~,()*:".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// What PostgREST says when it refuses.
#[derive(Debug, Default)]
struct Refusal {
    status: u16,
    code: String,
    message: String,
    details: String,
}

fn refusal(response: &HttpResponse) -> Refusal {
    let body: Json = serde_json::from_str(&response.body).unwrap_or(Json::Null);
    let text = |name: &str| body.get(name).and_then(Json::as_str).unwrap_or_default().to_owned();
    Refusal {
        status: response.status,
        code: text("code"),
        message: text("message"),
        details: text("details"),
    }
}

/// A unit of work through PostgREST. A commit is one transaction on the server.
pub struct PostgrestObjectSpace {
    shared: Rc<Shared>,
    environment: Environment,
    changes: ChangeSet,
    watchers: Vec<SpaceWatcher>,
}

impl fmt::Debug for PostgrestObjectSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgrestObjectSpace").finish_non_exhaustive()
    }
}

/// Whether the request reads or writes, for what a lost answer means.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Doing {
    Reading,
    Writing,
}

impl PostgrestObjectSpace {
    fn entity(&self, name: &str) -> Result<&Entity, SpaceError> {
        self.shared
            .entities
            .iter()
            .find(|entity| entity.info.name == name)
            .ok_or_else(|| SpaceError::UnknownEntity(name.to_owned()))
    }

    fn headers(&self, doing: Doing) -> Vec<(String, String)> {
        let shared = &self.shared;
        let mut headers = vec![("Accept".to_owned(), "application/json".to_owned())];
        if doing == Doing::Writing {
            headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
        }
        if let Some(key) = &shared.api_key {
            headers.push(("apikey".to_owned(), key.clone()));
        }
        if let Some(token) = shared.token.as_ref().and_then(|source| source()) {
            headers.push(("Authorization".to_owned(), format!("Bearer {token}")));
        }
        if let Some(schema) = &shared.schema {
            let profile = if doing == Doing::Writing { "Content-Profile" } else { "Accept-Profile" };
            headers.push((profile.to_owned(), schema.clone()));
        }
        headers
    }

    /// Says what the server refused, in words for people.
    fn refused(&self, entity: Option<&Entity>, refusal: &Refusal) -> SpaceError {
        let words = &self.shared.words;
        let code = refusal.code.as_str();
        let said = if refusal.message.is_empty() {
            format!("HTTP {}", refusal.status)
        } else {
            refusal.message.clone()
        };
        match code {
            "23505" => {
                // "Key (name)=(x) already exists." The value stays out of it.
                let column = refusal
                    .details
                    .strip_prefix("Key (")
                    .and_then(|rest| rest.split(')').next())
                    .and_then(|columns| columns.split(", ").next())
                    .map(|column| column.trim_matches('"'));
                let caption = entity
                    .zip(column)
                    .and_then(|(entity, column)| {
                        entity.info.fields.iter().find(|f| entity.map.column_of(&f.name) == column)
                    })
                    .map_or_else(String::new, |field| field.caption.clone());
                SpaceError::Refused(words.unique.replace("{0}", &caption))
            }
            "23503" => SpaceError::Refused(words.in_use.clone()),
            _ if code.starts_with("23") => SpaceError::Refused(words.rule.clone()),
            "PT409" => SpaceError::Conflict(said),
            "42501" => SpaceError::Denied(said),
            "PGRST202" => SpaceError::Unavailable(format!(
                "the database has no {COMMIT_FUNCTION} function: install SERVER_SQL in the schema PostgREST serves ({said})"
            )),
            _ if matches!(refusal.status, 401 | 403) => SpaceError::Denied(said),
            _ if matches!(refusal.status, 502..=504) || code == "PGRST001" => {
                SpaceError::Temporary(words.failed.replace("{0}", &said))
            }
            "PGRST205" | "42P01" => SpaceError::Unavailable(said),
            _ => SpaceError::Storage(words.failed.replace("{0}", &said)),
        }
    }

    fn send(
        &self,
        entity: Option<&Entity>,
        request: &HttpRequest,
        doing: Doing,
    ) -> Result<String, SpaceError> {
        let failed = |why: &str| self.shared.words.failed.replace("{0}", why);
        match self.shared.http.send(request) {
            Ok(response) if (200..300).contains(&response.status) => Ok(response.body),
            Ok(response) => Err(self.refused(entity, &refusal(&response))),
            Err(HttpError::NotSent(why)) => Err(SpaceError::Temporary(failed(&why))),
            Err(HttpError::NoAnswer(why)) if doing == Doing::Writing => {
                Err(SpaceError::Unknown(failed(&why)))
            }
            Err(HttpError::NoAnswer(why)) => Err(SpaceError::Temporary(failed(&why))),
        }
    }

    /// Reads rows of a resource, page by page, until `limit` or the end.
    fn fetch(
        &self,
        entity: &Entity,
        select: &str,
        filters: &[(String, String)],
        order: &str,
        offset: usize,
        limit: Option<usize>,
    ) -> Result<Vec<Object>, SpaceError> {
        let page = self.shared.page_size;
        let mut found = Vec::new();
        loop {
            let wanted = limit.map_or(page, |limit| page.min(limit - found.len()));
            if wanted == 0 {
                break;
            }
            let mut query = vec![("select".to_owned(), select.to_owned())];
            query.extend(filters.iter().cloned());
            query.push(("order".to_owned(), order.to_owned()));
            query.push(("limit".to_owned(), wanted.to_string()));
            query.push(("offset".to_owned(), (offset + found.len()).to_string()));
            let url = format!(
                "{}/{}?{}",
                self.shared.url,
                encoded(&entity.map.resource),
                query
                    .iter()
                    .map(|(name, value)| format!("{name}={}", encoded(value)))
                    .collect::<Vec<_>>()
                    .join("&")
            );
            let request =
                HttpRequest { method: "GET", url, headers: self.headers(Doing::Reading), body: None };
            let body = self.send(Some(entity), &request, Doing::Reading)?;
            let rows: Vec<Map<String, Json>> = serde_json::from_str(&body).map_err(|error| {
                SpaceError::Storage(self.shared.words.failed.replace("{0}", &error.to_string()))
            })?;
            let count = rows.len();
            for row in &rows {
                let mut object = entity.read(row);
                entity.info.compute(&mut object, &self.environment);
                found.push(object);
            }
            // A short page is not the end: the server's `max-rows` may have cut it. Only an empty
            // one is.
            if count == 0 {
                break;
            }
        }
        Ok(found)
    }

    fn key_column<'a>(&self, entity: &'a Entity) -> Result<&'a str, SpaceError> {
        let field =
            entity.info.key_field().ok_or_else(|| SpaceError::MissingKey(entity.info.name.clone()))?;
        Ok(entity.map.column_of(&field.name))
    }

    /// Every record, in key order, for what is filtered in memory.
    fn everything(&self, entity: &Entity) -> Result<Vec<Object>, SpaceError> {
        let order = format!("{}.asc", self.key_column(entity)?);
        self.fetch(entity, &entity.select(None), &[], &order, 0, None)
    }

    fn by_key(&self, entity: &Entity, key: &Value) -> Result<Option<Object>, SpaceError> {
        let column = self.key_column(entity)?;
        let Some(text) = key_text(key) else { return Ok(None) };
        let condition = match key {
            Value::Number(_) => text,
            _ => format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")),
        };
        let filters = [(column.to_owned(), format!("eq.{condition}"))];
        Ok(self
            .fetch(entity, &entity.select(None), &filters, &format!("{column}.asc"), 0, Some(1))?
            .into_iter()
            .next())
    }

    fn exists(&self, entity: &Entity, key: &Value) -> bool {
        self.by_key(entity, key).is_ok_and(|found| found.is_some())
    }

    fn next_key(&self, entity: &Entity, field: &FieldInfo) -> Result<i64, SpaceError> {
        let column = entity.map.column_of(&field.name);
        let select =
            if column == field.name { column.to_owned() } else { format!("{}:{column}", field.name) };
        let last = self.fetch(entity, &select, &[], &format!("{column}.desc.nullslast"), 0, Some(1))?;
        let stored = match last.first().map(|object| object.get(&field.name)) {
            Some(Value::Number(highest)) => highest as i64 + 1,
            _ => 1,
        };
        let mut keys = self.shared.next_keys.borrow_mut();
        let next = keys.get(&entity.info.name).copied().unwrap_or(0).max(stored);
        keys.insert(entity.info.name.clone(), next + 1);
        Ok(next)
    }

    /// One change, as the function reads it.
    fn described(&self, change: &Change) -> Result<Option<Json>, SpaceError> {
        let (name, op) = match change {
            Change::New(object) => (object.entity(), "insert"),
            Change::Updated(object) => (object.entity(), "update"),
            Change::Deleted { entity, .. } => (entity.as_str(), "delete"),
            _ => return Ok(None),
        };
        let entity = self.entity(name)?;
        let key_field = entity.info.key_field().ok_or_else(|| SpaceError::MissingKey(name.to_owned()))?;
        let key_column = entity.map.column_of(&key_field.name);
        let (key, values) = match change {
            Change::New(object) => (object.get(&key_field.name), Json::Object(entity.values(object, true))),
            Change::Updated(object) => {
                (object.get(&key_field.name), Json::Object(entity.values(object, false)))
            }
            Change::Deleted { key, .. } => (key.clone(), Json::Object(Map::new())),
            _ => return Ok(None),
        };
        if let Change::New(new) | Change::Updated(new) = change
            && !entity.info.state_machines.is_empty()
        {
            let stored = self.by_key(entity, &key)?;
            check_transitions(&entity.info, stored.as_ref(), new)?;
        }
        Ok(Some(json!({
            "op": op,
            "resource": entity.map.resource,
            "key_column": key_column,
            "key": value_to_json(&key),
            "values": values,
        })))
    }

    fn tell(&mut self, event: SpaceEvent<'_>) -> Result<(), SpaceError> {
        for watcher in &mut self.watchers {
            watcher(event).map_err(SpaceError::Refused)?;
        }
        Ok(())
    }

    /// The entity a refusal speaks of, when the commit touched only one.
    fn only_entity(&self, changes: &[Change]) -> Option<&Entity> {
        let mut names = changes.iter().filter_map(|change| match change {
            Change::New(object) | Change::Updated(object) => Some(object.entity()),
            Change::Deleted { entity, .. } => Some(entity.as_str()),
            _ => None,
        });
        let first = names.next()?;
        if names.all(|name| name == first) { self.entity(first).ok() } else { None }
    }
}

impl ObjectSpace for PostgrestObjectSpace {
    fn entities(&self) -> Vec<EntityInfo> {
        self.shared.entities.iter().map(|entity| entity.info.clone()).collect()
    }

    fn query(&self, name: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        let entity = self.entity(name)?;
        let columns = entity.columns();
        let translated = query.filter.as_ref().map(|criteria| filter::translate(criteria, &columns));
        let sortable = query.sort.iter().all(|(field, _)| columns.contains_key(field));
        let in_database = !self.changes.touches(name) && sortable && !matches!(translated, Some(Err(_)));
        if !in_database {
            let mut by_key: BTreeMap<String, Object> = self
                .everything(entity)?
                .into_iter()
                .filter_map(|object| key_of(&entity.info, &object).map(|key| (key, object)))
                .collect();
            self.changes.overlay(&entity.info, &mut by_key);
            let mut objects: Vec<Object> = by_key.into_values().collect();
            for object in &mut objects {
                entity.info.compute(object, &self.environment);
            }
            return run_query(&entity.info, objects, query, &self.environment);
        }
        check_query(&entity.info, query)?;
        let mut filters = Vec::new();
        if let Some(Ok(said)) = translated {
            filters.push(("and".to_owned(), said));
        }
        let key = self.key_column(entity)?;
        let mut order: Vec<String> = query
            .sort
            .iter()
            .map(|(field, ascending)| {
                format!("{}.{}", entity.map.column_of(field), if *ascending { "asc" } else { "desc" })
            })
            .collect();
        order.push(format!("{key}.asc"));
        let select = entity.select(query.only.as_deref());
        self.fetch(entity, &select, &filters, &order.join(","), query.offset, query.limit)
    }

    fn find(&self, name: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        let entity = self.entity(name)?;
        let Some(text) = key_text(key) else { return Ok(None) };
        let stored = self.by_key(entity, key)?;
        if !self.changes.touches(name) {
            return Ok(stored);
        }
        let mut by_key = BTreeMap::new();
        if let Some(object) = stored {
            by_key.insert(text.clone(), object);
        }
        self.changes.overlay(&entity.info, &mut by_key);
        Ok(by_key.remove(&text).map(|mut object| {
            entity.info.compute(&mut object, &self.environment);
            object
        }))
    }

    fn new_object(&mut self, name: &str) -> Result<Object, SpaceError> {
        let entity = self.entity(name)?;
        let mut object = entity.info.new_object();
        if let Some(field) = entity.info.key_field()
            && matches!(field.kind, FieldKind::Integer)
            && matches!(object.get(&field.name), Value::Null)
        {
            let next = self.next_key(entity, field)?;
            object.set(&field.name, Value::Number(next as f64));
        }
        Ok(object)
    }

    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        let entity = self.entity(object.entity())?;
        let info = entity.info.clone();
        let key = info.key_field().map(|field| object.get(&field.name));
        let exists = key.is_some_and(|key| self.exists(entity, &key));
        self.changes.save(&info, object, exists)
    }

    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        let entity = self.entity(object.entity())?;
        let info = entity.info.clone();
        let key = info.key_field().map(|field| object.get(&field.name));
        let exists = key.is_some_and(|key| self.exists(entity, &key));
        self.changes.insert(&info, object, exists)
    }

    fn delete(&mut self, name: &str, key: &Value) -> Result<(), SpaceError> {
        let entity = self.entity(name)?;
        let info = entity.info.clone();
        let exists = self.exists(entity, key);
        self.changes.delete(&info, key, exists);
        Ok(())
    }

    fn changes(&self) -> Vec<Change> {
        self.changes.list().to_vec()
    }

    fn commit(&mut self) -> Result<(), SpaceError> {
        let changes = self.changes.list().to_vec();
        self.tell(SpaceEvent::Committing(&changes))?;
        if !changes.is_empty() {
            let mut described = Vec::with_capacity(changes.len());
            for change in &changes {
                if let Some(said) = self.described(change)? {
                    described.push(said);
                }
            }
            let request = HttpRequest {
                method: "POST",
                url: format!("{}/rpc/{COMMIT_FUNCTION}", self.shared.url),
                headers: self.headers(Doing::Writing),
                body: Some(json!({ "changes": described }).to_string()),
            };
            self.send(self.only_entity(&changes), &request, Doing::Writing)?;
        }
        self.changes.clear();
        let _ = self.tell(SpaceEvent::Committed(&changes));
        Ok(())
    }

    fn rollback(&mut self) {
        self.changes.clear();
        let _ = self.tell(SpaceEvent::RolledBack);
    }

    fn on_event(&mut self, watcher: SpaceWatcher) {
        self.watchers.push(watcher);
    }
}

#[cfg(test)]
mod tests;
