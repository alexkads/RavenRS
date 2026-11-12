//! A web API over an application's model: the same entities, rules, permissions, states and audit
//! trail as its screens, for clients that do not use RavenRS.
//!
//! [`ApiService`] answers requests with no HTTP around it, and [`serve`] and [`router`] put HTTP
//! around one. Every write goes through a [`SecuredObjectSpace`], so what a person cannot do on the
//! screens they cannot do here, and every read leaves as that person can see it.
//!
//! Tokens live in memory: restarting the service asks everyone to sign in again. It speaks plain
//! HTTP, so anything beyond the machine it runs on belongs behind something that does TLS.

mod http;
mod json;
mod keys;
mod openapi;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub use http::{ApiOptions, router, router_with, serve, serve_with};
use ravenrs_app::{
    APP_SETTING, AuditSettings, AuditTrail, AuditedObjectSpace, AuthError, Authenticator, CONTEXT_DELETE,
    CONTEXT_SAVE, Credentials, Criteria, EntityInfo, FieldKind, Object, ObjectSpace, Operation,
    PasswordError, Query, RecordSettings, Role, RoleMerge, RuleWords, SecuredObjectSpace, SecurityContext,
    SettingScope, SettingsStore, SpaceError, SpaceMaker, UserDirectory, Value, audit_entity,
    security_entities, validate,
};
use serde_json::{Map, Value as Json, json};

use crate::json::{apply, exposed, from_json, key_from_path, object_to_json};

const DEFAULT_TOP: usize = 100;
const MOST_TOP: usize = 1000;
const MOST_FILTER: usize = 4096;
const MOST_OPERATIONS: usize = 200;
/// How deep a record can carry its collections, and those theirs.
const MOST_NESTING: usize = 4;
/// How many collections a request can ask to bring along, and how many records it asks for that way.
const MOST_EXPANDED: usize = 8;
const MOST_TOP_EXPANDED: usize = 100;
/// How many records and operations a body can hold. Each write checks rules that may read others.
const MOST_RECORDS: usize = 2000;
const SESSION_HOURS: u64 = 8;
/// How many tries to sign in a minute from one address. Each costs the hash of a password, and the
/// thread that does it is the one that answers everybody.
const SIGN_INS_A_MINUTE: u32 = 20;
/// How many times that, from everyone together.
const ALL_ADDRESSES: u32 = 10;
/// The longest password anyone is asked to compare.
const MOST_PASSWORD: usize = 1024;
const MINUTE: Duration = Duration::from_secs(60);
/// The most a setting holds.
const MOST_SETTING: usize = 64 * 1024;

/// What came in, with no HTTP left in it.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct ApiRequest {
    /// `GET`, `POST` and so on.
    pub method: String,
    /// The parts of the path.
    pub path: Vec<String>,
    /// The query string, decoded.
    pub query: BTreeMap<String, String>,
    /// The token presented.
    pub bearer: Option<String>,
    /// The address it came from, as far as anyone can tell.
    pub client: Option<String>,
    /// The body.
    pub body: Vec<u8>,
}

impl ApiRequest {
    /// A request with this method and path, such as `/api/data/Order`.
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            method: method.to_owned(),
            path: path.split('/').filter(|part| !part.is_empty()).map(str::to_owned).collect(),
            ..Self::default()
        }
    }

    /// With this token.
    pub fn bearer(mut self, token: impl Into<String>) -> Self {
        self.bearer = Some(token.into());
        self
    }

    /// From this address.
    pub fn client(mut self, client: impl Into<String>) -> Self {
        self.client = Some(client.into());
        self
    }

    /// With this body.
    pub fn json(mut self, body: &Json) -> Self {
        self.body = body.to_string().into_bytes();
        self
    }

    /// With this in the query string.
    pub fn query(mut self, name: &str, value: &str) -> Self {
        self.query.insert(name.to_owned(), value.to_owned());
        self
    }
}

/// What goes back.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ApiResponse {
    /// The HTTP status.
    pub status: u16,
    /// The body.
    pub body: Json,
}

/// What a request's answer is when it does not work out.
#[derive(Debug)]
pub struct Failure {
    status: u16,
    code: &'static str,
    message: String,
    violations: Option<Json>,
    /// Which operation of a batch it was.
    operation: Option<usize>,
}

impl Failure {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into(), violations: None, operation: None }
    }

    /// What was asked does not fit.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, "bad_request", message)
    }

    /// There is nothing there. It is also what someone who cannot see a record is told.
    pub fn not_found() -> Self {
        Self::new(404, "not_found", "There is no such record.")
    }

    /// This person cannot do this.
    pub fn denied() -> Self {
        Self::new(403, "denied", "You are not allowed to do this.")
    }

    fn unauthorized() -> Self {
        Self::new(401, "unauthorized", "A valid token is required.")
    }

    fn into_response(self) -> ApiResponse {
        let mut body = json!({ "error": self.code, "message": self.message });
        if let (Some(violations), Some(object)) = (self.violations, body.as_object_mut()) {
            object.insert("violations".to_owned(), violations);
        }
        if let (Some(operation), Some(object)) = (self.operation, body.as_object_mut()) {
            object.insert("operation".to_owned(), json!(operation));
        }
        ApiResponse { status: self.status, body }
    }
}

impl From<SpaceError> for Failure {
    fn from(error: SpaceError) -> Self {
        match error {
            SpaceError::UnknownEntity(_) => Self::not_found(),
            SpaceError::BadQuery(why) => Self::bad_request(why),
            SpaceError::MissingKey(why) => Self::bad_request(why),
            SpaceError::Denied(_) => Self::denied(),
            SpaceError::Refused(why) => Self::new(422, "refused", why),
            error @ SpaceError::DuplicateKey { .. } => Self::new(409, "conflict", error.to_string()),
            SpaceError::Conflict(why) => Self::new(409, "conflict", why),
            // What went wrong in storage is for the log, and not for whoever is asking.
            other => {
                tracing::error!(error = %other, "storage failed");
                Self::new(500, "internal", "Something went wrong on the server.")
            }
        }
    }
}

/// Something of the application's own to run, by name: `POST /api/run/{name}`.
pub type Endpoint = Rc<dyn Fn(&EndpointContext<'_>, &Json) -> Result<Json, Failure>>;

/// What an endpoint works with: a unit of work with the permissions of whoever called.
#[non_exhaustive]
pub struct EndpointContext<'a> {
    /// Who called.
    pub name: &'a str,
    /// Their unit of work. Nothing stays until it is committed.
    pub space: &'a std::cell::RefCell<Box<dyn ObjectSpace>>,
}

impl std::fmt::Debug for EndpointContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EndpointContext").field("name", &self.name).finish_non_exhaustive()
    }
}

/// Who gets to a setting, over the API. A setting nobody declared cannot be reached at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SettingAccess {
    /// Each person has their own, and only they reach it.
    PerPerson,
    /// One for everyone, who can read it, and only administrators change it.
    Shared,
}

struct Session {
    context: Rc<SecurityContext>,
    trail: Option<Rc<AuditTrail>>,
    expires: Instant,
}

/// An application's model and permissions, answering requests.
pub struct ApiService {
    title: String,
    space: SpaceMaker,
    entities: Vec<EntityInfo>,
    directory: UserDirectory,
    authenticator: Rc<dyn Authenticator>,
    merge: RoleMerge,
    audit: Option<AuditSettings>,
    hidden: BTreeSet<String>,
    endpoints: BTreeMap<String, Endpoint>,
    words: RuleWords,
    sessions: HashMap<String, Session>,
    lifetime: Duration,
    tries: HashMap<String, u32>,
    tries_since: Instant,
    sign_ins_a_minute: u32,
    swept: Instant,
    settings: Option<Rc<dyn SettingsStore>>,
    declared: BTreeMap<String, SettingAccess>,
}

impl std::fmt::Debug for ApiService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiService").field("title", &self.title).finish_non_exhaustive()
    }
}

fn token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn records_in(value: &Json) -> usize {
    match value {
        Json::Object(object) => 1 + object.values().map(records_in).sum::<usize>(),
        Json::Array(items) => items.iter().map(records_in).sum(),
        _ => 0,
    }
}

fn body_of(request: &ApiRequest) -> Result<Map<String, Json>, Failure> {
    let value: Json = serde_json::from_slice(&request.body)
        .map_err(|_| Failure::bad_request("The body has to be a JSON object."))?;
    if records_in(&value) > MOST_RECORDS {
        return Err(Failure::bad_request("The body holds too many records."));
    }
    match value {
        Json::Object(object) => Ok(object),
        _ => Err(Failure::bad_request("The body has to be a JSON object.")),
    }
}

fn number_in(request: &ApiRequest, name: &str) -> Result<Option<usize>, Failure> {
    request
        .query
        .get(name)
        .map(|text| text.parse().map_err(|_| Failure::bad_request(format!("{name} has to be a number"))))
        .transpose()
}

impl ApiService {
    /// A service over what `space` makes, with these roles. What keeps users and roles has to be
    /// among what storage knows, as for a window with a sign-in.
    pub fn new(space: SpaceMaker, roles: Vec<Role>) -> Result<Self, SpaceError> {
        let entities = space().entities();
        let settings: Option<Rc<dyn SettingsStore>> = entities
            .iter()
            .any(|entity| entity.name == APP_SETTING)
            .then(|| Rc::new(RecordSettings::new(space.clone())) as Rc<dyn SettingsStore>);
        let directory = UserDirectory::new(space.clone(), roles)?;
        // Who can sign in and what they did are not records for a client to go through.
        let hidden = security_entities()
            .into_iter()
            .chain([audit_entity(), ravenrs_app::settings_entity()])
            .map(|entity| entity.name)
            .collect();
        Ok(Self {
            title: "RavenRS".to_owned(),
            authenticator: Rc::new(directory.clone()),
            space,
            entities,
            directory,
            merge: RoleMerge::default(),
            audit: None,
            hidden,
            endpoints: BTreeMap::new(),
            words: RuleWords::default(),
            sessions: HashMap::new(),
            lifetime: Duration::from_secs(SESSION_HOURS * 3600),
            tries: HashMap::new(),
            tries_since: Instant::now(),
            sign_ins_a_minute: SIGN_INS_A_MINUTE,
            swept: Instant::now(),
            settings,
            declared: BTreeMap::new(),
        })
    }

    /// The name in its description.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Writes what is done here to an audit trail, under the name of who did it.
    pub fn audit(mut self, settings: AuditSettings) -> Self {
        self.audit = Some(settings);
        self
    }

    /// How the permissions of several roles add up.
    pub fn role_merge(mut self, merge: RoleMerge) -> Self {
        self.merge = merge;
        self
    }

    /// Another way of signing in than the passwords of the users.
    pub fn authenticator(mut self, authenticator: Rc<dyn Authenticator>) -> Self {
        self.authenticator = authenticator;
        self
    }

    /// The words of what the rules say.
    pub fn rule_words(mut self, words: RuleWords) -> Self {
        self.words = words;
        self
    }

    /// Keeps an entity away from the API, whatever the permissions say.
    pub fn hide(mut self, entity: impl Into<String>) -> Self {
        self.hidden.insert(entity.into());
        self
    }

    /// How long a token lasts. Eight hours by default.
    pub fn session_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }

    /// How many tries to sign in or to change a password it takes a minute from one address, and
    /// ten times that from everyone. Twenty by default. Behind a proxy everyone is one address.
    pub fn sign_ins_a_minute(mut self, most: u32) -> Self {
        self.sign_ins_a_minute = most;
        self
    }

    /// Lets this setting be reached at `/api/settings/{name}`. Storage has to know
    /// [`settings_entity`](ravenrs_app::settings_entity).
    pub fn setting(mut self, name: impl Into<String>, access: SettingAccess) -> Self {
        self.declared.insert(name.into(), access);
        self
    }

    /// Adds something of the application's own, run with the permissions of whoever calls it.
    pub fn endpoint(
        mut self,
        name: impl Into<String>,
        run: impl Fn(&EndpointContext<'_>, &Json) -> Result<Json, Failure> + 'static,
    ) -> Self {
        self.endpoints.insert(name.into(), Rc::new(run));
        self
    }

    /// Answers a request.
    pub fn handle(&mut self, request: ApiRequest) -> ApiResponse {
        match self.route(&request) {
            Ok((status, body)) => ApiResponse { status, body },
            Err(failure) => failure.into_response(),
        }
    }

    fn route(&mut self, request: &ApiRequest) -> Result<(u16, Json), Failure> {
        let path: Vec<&str> = request.path.iter().map(String::as_str).collect();
        let method = request.method.as_str();
        match (method, path.as_slice()) {
            ("POST", ["api", "auth", "token"]) => return self.sign_in(request),
            ("POST", ["api", "auth", "revoke"]) => {
                if let Some(token) = &request.bearer {
                    self.sessions.remove(token);
                }
                return Ok((200, json!({})));
            }
            _ => {}
        }
        let session = self.session_of(request)?;
        let by_key = request.bearer.as_deref().is_some_and(keys::is_key);
        if by_key && path.get(1) == Some(&"auth") {
            // A key does not get to change passwords, or to make itself more keys.
            return Err(Failure::denied());
        }
        if path.as_slice() == ["api", "auth", "password"] && method == "POST" {
            return self.change_password(request, &session);
        }
        if session.0.identity().must_change_password {
            return Err(Failure::new(
                403,
                "password_change_required",
                "This person has to choose a new password first, at /api/auth/password.",
            ));
        }
        let mut space = self.space_for(&session);
        match (method, path.as_slice()) {
            ("GET", ["api", "openapi.json"]) => Ok((200, self.description(&session))),
            ("GET", ["api", "me"]) => {
                let identity = session.0.identity();
                let roles: Vec<&str> = identity.roles.iter().map(Role::name).collect();
                Ok((200, json!({ "name": identity.name, "roles": roles })))
            }
            ("POST", ["api", "auth", "keys"]) => self.make_key(request, &session),
            ("GET", ["api", "auth", "keys"]) => self.list_keys(&session),
            ("POST", ["api", "auth", "keys", id, "rotate"]) => self.rotate_key(id, &session),
            ("DELETE", ["api", "auth", "keys", id]) => self.revoke_key(id, &session),
            ("GET" | "PUT" | "DELETE", ["api", "settings", name]) => {
                self.reach_setting(method, name, request, &session)
            }
            ("GET", ["api", "data", entity]) => self.list(&*space, entity, request),
            ("GET", ["api", "data", entity, key]) => {
                let info = self.entity(entity)?;
                let key = key_from_path(&info, key)?;
                let found = space.find(entity, &key)?.ok_or_else(Failure::not_found)?;
                let mut record = object_to_json(&info, &found);
                self.expand(&*space, &info, &key, &mut record, request)?;
                Ok((200, Json::Object(record)))
            }
            ("POST", ["api", "data", entity]) => {
                let answer = self.create(&mut *space, entity, &body_of(request)?, 0)?;
                space.commit()?;
                Ok((201, answer))
            }
            ("PATCH", ["api", "data", entity, key]) => {
                let info = self.entity(entity)?;
                let key = key_from_path(&info, key)?;
                let answer = self.change(&mut *space, &info, &key, &body_of(request)?, 0)?;
                space.commit()?;
                Ok((200, answer))
            }
            ("DELETE", ["api", "data", entity, key]) => {
                let info = self.entity(entity)?;
                let key = key_from_path(&info, key)?;
                self.remove(&mut *space, &info, &key)?;
                space.commit()?;
                Ok((200, json!({})))
            }
            ("POST", ["api", "validate", entity]) => self.check(entity, &body_of(request)?),
            ("POST", ["api", "batch"]) => self.batch(&mut *space, &body_of(request)?),
            ("POST", ["api", "run", name]) => {
                let endpoint = self.endpoints.get(*name).cloned().ok_or_else(Failure::not_found)?;
                let input: Json = if request.body.is_empty() {
                    Json::Null
                } else {
                    serde_json::from_slice(&request.body)
                        .map_err(|_| Failure::bad_request("The body has to be JSON."))?
                };
                let held = std::cell::RefCell::new(space);
                let identity = session.0.identity();
                let context = EndpointContext { name: &identity.name, space: &held };
                let answer = endpoint(&context, &input)?;
                Ok((200, answer))
            }
            _ => Err(Failure::new(404, "not_found", "There is no such route.")),
        }
    }

    /// Counts a try that costs the hash of a password, or refuses it. Only what gets this far counts,
    /// so a body that means nothing does not use anyone's turn.
    fn count_try(&mut self, request: &ApiRequest) -> Result<(), Failure> {
        let now = Instant::now();
        if now.duration_since(self.tries_since) >= MINUTE {
            self.tries.clear();
            self.tries_since = now;
        }
        let all: u32 = self.tries.values().sum();
        let client = request.client.clone().unwrap_or_default();
        let mine = self.tries.entry(client).or_insert(0);
        if *mine >= self.sign_ins_a_minute || all >= self.sign_ins_a_minute.saturating_mul(ALL_ADDRESSES) {
            return Err(Failure::new(429, "too_many", "Too many tries. Try again soon."));
        }
        *mine += 1;
        Ok(())
    }

    fn sign_in(&mut self, request: &ApiRequest) -> Result<(u16, Json), Failure> {
        let now = Instant::now();
        let body = body_of(request)?;
        let text = |name: &str| body.get(name).and_then(Json::as_str).unwrap_or("").to_owned();
        let rejected = || Failure::new(401, "unauthorized", "The name or the password is not right.");
        if text("password").len() > MOST_PASSWORD {
            return Err(rejected());
        }
        self.count_try(request)?;
        self.sessions.retain(|_, session| session.expires > now);
        let identity = self
            .authenticator
            .authenticate(&Credentials::new(text("name"), text("password")))
            .map_err(|error| match error {
                AuthError::Rejected => rejected(),
                other => {
                    tracing::error!(error = %other, "signing in failed");
                    Failure::new(500, "internal", "Something went wrong on the server.")
                }
            })?;
        let must_change_password = identity.must_change_password;
        let author = identity.name.clone();
        let mut context =
            SecurityContext::new(identity, self.directory.clone(), self.entities.clone(), self.merge);
        let trail = self.audit.clone().map(|settings| {
            let trail = Rc::new(AuditTrail::new(settings, move || author.clone()));
            trail.write_with(self.space.clone());
            trail
        });
        if let Some(trail) = &trail {
            context = context.with_audit_trail(trail.clone());
        }
        let token = token();
        self.sessions.insert(
            token.clone(),
            Session { context: Rc::new(context), trail, expires: now + self.lifetime },
        );
        Ok((
            200,
            json!({
                "token": token,
                "expires_in": self.lifetime.as_secs(),
                "must_change_password": must_change_password,
            }),
        ))
    }

    fn session_of(
        &mut self,
        request: &ApiRequest,
    ) -> Result<(Rc<SecurityContext>, Option<Rc<AuditTrail>>), Failure> {
        let token = request.bearer.as_deref().ok_or_else(Failure::unauthorized)?;
        if keys::is_key(token) {
            return self.session_of_key(token);
        }
        let now = Instant::now();
        if now.duration_since(self.swept) >= MINUTE {
            self.sessions.retain(|_, session| session.expires > now);
            self.swept = now;
        }
        let session = self.sessions.get(token).ok_or_else(Failure::unauthorized)?;
        if session.expires <= now {
            self.sessions.remove(token);
            return Err(Failure::unauthorized());
        }
        // Who they are is read each time, so a role lost or a user turned off counts now. Someone
        // who cannot sign in any more has no session either, whatever a route goes on to check.
        let (context, trail) = (session.context.clone(), session.trail.clone());
        let key = context.identity().key;
        if self.directory.identity_by_key(&key).map_err(Failure::from)?.is_none() {
            self.sessions.remove(token);
            return Err(Failure::unauthorized());
        }
        context.reload().map_err(Failure::from)?;
        Ok((context, trail))
    }

    fn change_password(
        &mut self,
        request: &ApiRequest,
        session: &(Rc<SecurityContext>, Option<Rc<AuditTrail>>),
    ) -> Result<(u16, Json), Failure> {
        let body = body_of(request)?;
        let text = |name: &str| body.get(name).and_then(Json::as_str).unwrap_or("").to_owned();
        if text("current").len() > MOST_PASSWORD {
            return Err(Failure::new(403, "denied", PasswordError::WrongCurrent.to_string()));
        }
        self.count_try(request)?;
        session.0.change_own_password(&text("current"), &text("new")).map_err(|error| match error {
            PasswordError::TooShort | PasswordError::SameAsCurrent => {
                Failure::new(422, "refused", error.to_string())
            }
            PasswordError::WrongCurrent => Failure::new(403, "denied", error.to_string()),
            PasswordError::NotAllowed | PasswordError::NoSuchUser => Failure::denied(),
            other => {
                tracing::error!(error = %other, "changing a password failed");
                Failure::new(500, "internal", "Something went wrong on the server.")
            }
        })?;
        // The new password ends the other sessions the old one opened. Keys are made apart, and stay.
        let key = session.0.identity().key;
        let kept = request.bearer.clone().unwrap_or_default();
        self.sessions.retain(|token, other| *token == kept || other.context.identity().key != key);
        Ok((200, json!({})))
    }

    fn space_for(&self, session: &(Rc<SecurityContext>, Option<Rc<AuditTrail>>)) -> Box<dyn ObjectSpace> {
        let raw = (self.space)();
        let under: Box<dyn ObjectSpace> = match &session.1 {
            Some(trail) => Box::new(AuditedObjectSpace::new(raw, trail.clone())),
            None => raw,
        };
        Box::new(SecuredObjectSpace::new(under, session.0.clone()))
    }

    fn entity(&self, name: &str) -> Result<EntityInfo, Failure> {
        if self.hidden.contains(name) {
            return Err(Failure::not_found());
        }
        self.entities
            .iter()
            .find(|entity| entity.name == name && !entity.transient)
            .cloned()
            .ok_or_else(Failure::not_found)
    }

    fn description(&self, session: &(Rc<SecurityContext>, Option<Rc<AuditTrail>>)) -> Json {
        // Only what this person can reach is any of their business.
        let reachable: Vec<EntityInfo> = self
            .entities
            .iter()
            .filter(|entity| !self.hidden.contains(&entity.name) && !entity.transient)
            .filter(|entity| {
                session.0.with(|permissions| {
                    [Operation::Read, Operation::Create, Operation::Write, Operation::Delete]
                        .into_iter()
                        .any(|operation| permissions.can(operation, &entity.name, None))
                })
            })
            .cloned()
            .collect();
        openapi::document(&self.title, &reachable)
    }

    fn reach_setting(
        &self,
        method: &str,
        name: &str,
        request: &ApiRequest,
        session: &(Rc<SecurityContext>, Option<Rc<AuditTrail>>),
    ) -> Result<(u16, Json), Failure> {
        let (Some(access), Some(store)) = (self.declared.get(name), &self.settings) else {
            return Err(Failure::not_found());
        };
        // A key stands for a system, and has no preferences of a person to reach. One with no role
        // left reaches nothing.
        let by_key = request.bearer.as_deref().is_some_and(keys::is_key);
        if session.0.identity().roles.is_empty() || (by_key && *access == SettingAccess::PerPerson) {
            return Err(Failure::denied());
        }
        let scope = match access {
            SettingAccess::PerPerson => {
                let key = session.0.identity().key;
                SettingScope::User(ravenrs_app::key_text(&key).ok_or_else(Failure::denied)?)
            }
            SettingAccess::Shared => {
                if method != "GET" && !session.0.with(ravenrs_app::Permissions::is_administrator) {
                    return Err(Failure::denied());
                }
                SettingScope::Global
            }
        };
        let failed = |why: String| {
            tracing::error!(error = %why, "a setting failed");
            Failure::new(500, "internal", "Something went wrong on the server.")
        };
        match method {
            "GET" => {
                let held = store.get(&scope, name).map_err(failed)?;
                let value =
                    held.and_then(|text| serde_json::from_str::<Json>(&text).ok()).unwrap_or(Json::Null);
                Ok((200, json!({ "value": value })))
            }
            "PUT" => {
                if request.body.len() > MOST_SETTING {
                    return Err(Failure::bad_request("The setting is too large."));
                }
                let value = body_of(request)?.remove("value").unwrap_or(Json::Null);
                store.set(&scope, name, &value.to_string()).map_err(failed)?;
                Ok((200, json!({})))
            }
            _ => {
                store.remove(&scope, name).map_err(failed)?;
                Ok((200, json!({})))
            }
        }
    }

    fn query_of(&self, info: &EntityInfo, request: &ApiRequest) -> Result<Query, Failure> {
        let mut query = Query::all();
        if let Some(filter) = request.query.get("filter") {
            if filter.len() > MOST_FILTER {
                return Err(Failure::bad_request("The filter is too long."));
            }
            query.filter = Some(
                Criteria::parse(filter)
                    .map_err(|error| Failure::bad_request(format!("The filter: {error}")))?,
            );
        }
        if let Some(order) = request.query.get("order") {
            for part in order.split(',').map(str::trim).filter(|part| !part.is_empty()) {
                let (name, ascending) = part.strip_prefix('-').map_or((part, true), |name| (name, false));
                let known = info.field_named(name).is_some_and(exposed);
                if !known {
                    return Err(Failure::bad_request(format!("{} has no field {name}", info.name)));
                }
                query.sort.push((name.to_owned(), ascending));
            }
        }
        if let Some(filter) = &query.filter
            && filter.properties().iter().any(|path| {
                let head = path.split('.').next().unwrap_or(path);
                info.field_named(head).is_some_and(|field| field.secret)
            })
        {
            return Err(Failure::bad_request("The filter asks about a field that cannot be read."));
        }
        query.offset = number_in(request, "skip")?.unwrap_or(0);
        let most = if request.query.contains_key("expand") { MOST_TOP_EXPANDED } else { MOST_TOP };
        query.limit = Some(number_in(request, "top")?.unwrap_or(DEFAULT_TOP).min(most));
        Ok(query)
    }

    fn list(
        &self,
        space: &dyn ObjectSpace,
        entity: &str,
        request: &ApiRequest,
    ) -> Result<(u16, Json), Failure> {
        let info = self.entity(entity)?;
        let query = self.query_of(&info, request)?;
        let found = space.query(entity, &query)?;
        let key_name = info.key_field().map(|field| field.name.clone());
        let mut items = Vec::with_capacity(found.len());
        for object in &found {
            let mut record = object_to_json(&info, object);
            if let Some(name) = &key_name {
                self.expand(space, &info, &object.get(name), &mut record, request)?;
            }
            items.push(Json::Object(record));
        }
        let mut answer = json!({ "items": items });
        if request.query.get("count").is_some_and(|value| value == "true")
            && let Some(object) = answer.as_object_mut()
        {
            let all = Query { filter: query.filter, ..Query::all() };
            object.insert("count".to_owned(), json!(space.query(entity, &all)?.len()));
        }
        Ok((200, answer))
    }

    fn back_reference(
        &self,
        owner: &EntityInfo,
        field: &ravenrs_app::FieldInfo,
    ) -> Option<(EntityInfo, String)> {
        let FieldKind::Collection(target) = &field.kind else { return None };
        let child = self.entities.iter().find(|entity| entity.name == *target)?.clone();
        let back = field.back_reference.clone().or_else(|| {
            child
                .fields
                .iter()
                .find(|other| matches!(&other.kind, FieldKind::Reference(to) if *to == owner.name))
                .map(|other| other.name.clone())
        })?;
        Some((child, back))
    }

    fn expand(
        &self,
        space: &dyn ObjectSpace,
        info: &EntityInfo,
        key: &Value,
        record: &mut Map<String, Json>,
        request: &ApiRequest,
    ) -> Result<(), Failure> {
        let Some(asked) = request.query.get("expand") else { return Ok(()) };
        let names: BTreeSet<&str> = asked.split(',').map(str::trim).filter(|name| !name.is_empty()).collect();
        if names.len() > MOST_EXPANDED {
            return Err(Failure::bad_request("It asks to expand too many collections."));
        }
        for name in names {
            let field = info
                .field_named(name)
                .filter(|field| matches!(field.kind, FieldKind::Collection(_)))
                .ok_or_else(|| Failure::bad_request(format!("{} has no collection {name}", info.name)))?;
            let (child, back) = self
                .back_reference(info, field)
                .filter(|(child, _)| !self.hidden.contains(&child.name))
                .ok_or_else(|| Failure::bad_request(format!("{name} cannot be expanded")))?;
            let query = Query {
                filter: Some(Criteria::property(&back).equals(Criteria::value(key.clone()))),
                limit: Some(MOST_TOP),
                ..Query::all()
            };
            let items: Vec<Json> = space
                .query(&child.name, &query)?
                .iter()
                .map(|object| Json::Object(object_to_json(&child, object)))
                .collect();
            record.insert(name.to_owned(), Json::Array(items));
        }
        Ok(())
    }

    /// The rules look at the record whole, as stored with what was sent over it, and at all the
    /// others: a required field this person cannot see is still there, and only a whole record can
    /// tell. What answers has been through the permissions by the time it leaves.
    fn refuse_broken_rules(&self, info: &EntityInfo, object: &Object, context: &str) -> Result<(), Failure> {
        let raw = (self.space)();
        let broken: Vec<Json> = validate(info, object, context, &*raw, &self.words)
            .iter()
            .filter(|violation| violation.is_error())
            .map(|violation| json!({ "field": violation.field, "message": violation.message }))
            .collect();
        if broken.is_empty() {
            return Ok(());
        }
        let mut failure = Failure::new(422, "invalid", "The rules of the entity refuse this.");
        failure.violations = Some(Json::Array(broken));
        Err(failure)
    }

    /// A reference has to be to a record this person can see, of the kind of key it has.
    fn refuse_broken_references(
        &self,
        space: &dyn ObjectSpace,
        info: &EntityInfo,
        object: &Object,
        sent: &Map<String, Json>,
    ) -> Result<(), Failure> {
        for name in sent.keys() {
            let Some(field) = info.field_named(name) else { continue };
            let FieldKind::Reference(target) = &field.kind else { continue };
            let value = object.get(name);
            if value != Value::Null && space.find(target, &value)?.is_none() {
                return Err(Failure::bad_request(format!("{name} refers to a record that is not there")));
            }
        }
        Ok(())
    }

    /// What goes back after a write, as this person can see it.
    fn seen(
        &self,
        space: &dyn ObjectSpace,
        info: &EntityInfo,
        key: &Value,
        object: &Object,
    ) -> Result<Json, Failure> {
        let found = space.find(&info.name, key)?;
        let mut record = object_to_json(info, found.as_ref().unwrap_or(object));
        if found.is_none() {
            // Created where they cannot see it: they get to know its key, and nothing else.
            record.retain(|name, _| info.field_named(name).is_some_and(|field| field.is_key));
        }
        Ok(Json::Object(record))
    }

    fn create(
        &self,
        space: &mut dyn ObjectSpace,
        entity: &str,
        sent: &Map<String, Json>,
        depth: usize,
    ) -> Result<Json, Failure> {
        let info = self.entity(entity)?;
        let mut object = space.new_object(entity)?;
        apply(&info, &mut object, sent, true)?;
        info.compute(&mut object, &ravenrs_app::environment());
        self.refuse_broken_references(&*space, &info, &object, sent)?;
        self.refuse_broken_rules(&info, &object, CONTEXT_SAVE)?;
        let key = info.key_field().map(|field| object.get(&field.name)).unwrap_or_default();
        space.insert(object.clone())?;
        self.nested(space, &info, &key, sent, depth)?;
        self.seen(&*space, &info, &key, &object)
    }

    fn change(
        &self,
        space: &mut dyn ObjectSpace,
        info: &EntityInfo,
        key: &Value,
        sent: &Map<String, Json>,
        depth: usize,
    ) -> Result<Json, Failure> {
        let mut object = space.find(&info.name, key)?.ok_or_else(Failure::not_found)?;
        apply(info, &mut object, sent, false)?;
        let mut whole = (self.space)().find(&info.name, key)?.unwrap_or_else(|| object.clone());
        apply(info, &mut whole, sent, false)
            .or_else(|failure| if failure.status == 403 { Ok(()) } else { Err(failure) })?;
        info.compute(&mut whole, &ravenrs_app::environment());
        self.refuse_broken_references(&*space, info, &object, sent)?;
        self.refuse_broken_rules(info, &whole, CONTEXT_SAVE)?;
        space.save(object.clone())?;
        self.nested(space, info, key, sent, depth)?;
        self.seen(&*space, info, key, &object)
    }

    fn remove(&self, space: &mut dyn ObjectSpace, info: &EntityInfo, key: &Value) -> Result<(), Failure> {
        space.find(&info.name, key)?.ok_or_else(Failure::not_found)?;
        let whole = (self.space)().find(&info.name, key)?.ok_or_else(Failure::not_found)?;
        self.refuse_broken_rules(info, &whole, CONTEXT_DELETE)?;
        space.delete(&info.name, key)?;
        Ok(())
    }

    /// Records of an aggregated collection that came along. Each has to belong to this owner.
    fn nested(
        &self,
        space: &mut dyn ObjectSpace,
        info: &EntityInfo,
        key: &Value,
        sent: &Map<String, Json>,
        depth: usize,
    ) -> Result<(), Failure> {
        for (name, value) in sent {
            let Some(field) = info.field_named(name) else { continue };
            if !matches!(field.kind, FieldKind::Collection(_)) {
                continue;
            }
            if !field.aggregated {
                return Err(Failure::bad_request(format!("{name} is not written through its owner")));
            }
            if depth >= MOST_NESTING {
                return Err(Failure::bad_request("The records are nested too deep."));
            }
            let (child, back) = self
                .back_reference(info, field)
                .ok_or_else(|| Failure::bad_request(format!("{name} cannot be written")))?;
            self.entity(&child.name)?;
            let items =
                value.as_array().ok_or_else(|| Failure::bad_request(format!("{name} has to be a list")))?;
            let child_key = child
                .key_field()
                .ok_or_else(|| Failure::bad_request(format!("{} has no key", child.name)))?
                .clone();
            for item in items {
                let item = item
                    .as_object()
                    .ok_or_else(|| Failure::bad_request(format!("{name} has to hold records")))?;
                let mine = |space: &dyn ObjectSpace, wanted: &Value| -> Result<(), Failure> {
                    let found = space.find(&child.name, wanted)?.ok_or_else(Failure::not_found)?;
                    if found.get(&back) == *key { Ok(()) } else { Err(Failure::not_found()) }
                };
                if let Some(gone) = item.get("$delete") {
                    let gone = from_json(&child_key, gone)?;
                    mine(&*space, &gone)?;
                    self.remove(space, &child, &gone)?;
                } else if let Some(existing) = item.get(&child_key.name) {
                    let existing = from_json(&child_key, existing)?;
                    mine(&*space, &existing)?;
                    let mut rest = item.clone();
                    rest.remove(&child_key.name);
                    if rest.contains_key(&back) {
                        return Err(Failure::bad_request(format!("{back} comes from the owner")));
                    }
                    self.change(space, &child, &existing, &rest, depth + 1)?;
                } else {
                    if item.contains_key(&back) {
                        return Err(Failure::bad_request(format!("{back} comes from the owner")));
                    }
                    let mut record = item.clone();
                    record.insert(back.clone(), ravenrs_app::value_to_json(key));
                    self.create(space, &child.name, &record, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    /// What the rules would say of a record, with nothing kept.
    fn check(&self, entity: &str, sent: &Map<String, Json>) -> Result<(u16, Json), Failure> {
        let info = self.entity(entity)?;
        let mut object = info.new_object();
        apply(&info, &mut object, sent, true)?;
        info.compute(&mut object, &ravenrs_app::environment());
        let raw = (self.space)();
        let found: Vec<Json> = validate(&info, &object, CONTEXT_SAVE, &*raw, &self.words)
            .iter()
            .map(|violation| {
                json!({
                    "field": violation.field,
                    "message": violation.message,
                    "stops": violation.is_error(),
                })
            })
            .collect();
        Ok((200, json!({ "violations": found })))
    }

    fn batch(&self, space: &mut dyn ObjectSpace, sent: &Map<String, Json>) -> Result<(u16, Json), Failure> {
        let operations = sent
            .get("operations")
            .and_then(Json::as_array)
            .ok_or_else(|| Failure::bad_request("A batch has a list of operations."))?;
        if operations.len() > MOST_OPERATIONS {
            return Err(Failure::bad_request("The batch has too many operations."));
        }
        let mut results = Vec::with_capacity(operations.len());
        for (index, operation) in operations.iter().enumerate() {
            let done = self.one_of_batch(space, operation);
            match done {
                Ok(result) => results.push(result),
                Err(mut failure) => {
                    space.rollback();
                    failure.operation = Some(index);
                    return Err(failure);
                }
            }
        }
        space.commit()?;
        Ok((200, json!({ "results": results })))
    }

    fn one_of_batch(&self, space: &mut dyn ObjectSpace, operation: &Json) -> Result<Json, Failure> {
        let text = |name: &str| operation.get(name).and_then(Json::as_str);
        let entity = text("entity").ok_or_else(|| Failure::bad_request("An operation names its entity."))?;
        let empty = Map::new();
        let body = operation.get("body").and_then(Json::as_object).unwrap_or(&empty);
        let info = self.entity(entity)?;
        let key = || -> Result<Value, Failure> {
            let field = info.key_field().ok_or_else(|| Failure::bad_request("It has no key."))?;
            let sent =
                operation.get("key").ok_or_else(|| Failure::bad_request("An operation names its key."))?;
            from_json(field, sent)
        };
        match text("method") {
            Some("POST") => self.create(space, entity, body, 0),
            Some("PATCH") => self.change(space, &info, &key()?, body, 0),
            Some("DELETE") => {
                self.remove(space, &info, &key()?)?;
                Ok(json!({}))
            }
            _ => Err(Failure::bad_request("An operation is POST, PATCH or DELETE.")),
        }
    }
}
