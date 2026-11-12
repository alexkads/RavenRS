//! Keys for other systems, which present them in place of a token from signing in. A key acts as a
//! user, and never as more: with roles of its own, it gets those the user still has and no others.

use std::rc::Rc;

use ravenrs_app::{
    AuditTrail, Identity, Object, Permissions, Query, RoleMerge, SECURITY_API_KEY, SecurityContext, Value,
    local_now,
};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};

use crate::{ApiRequest, ApiService, Failure, body_of};

const PREFIX: &str = "rvk_";
const MOST_DAYS: i64 = 3650;

pub(crate) type Who = (Rc<SecurityContext>, Option<Rc<AuditTrail>>);

pub(crate) fn is_key(token: &str) -> bool {
    token.starts_with(PREFIX)
}

fn hash_of(secret: &str) -> String {
    Sha256::digest(secret.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Compares without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0_u8, |difference, (x, y)| difference | (x ^ y)) == 0
}

fn new_secret() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn public(key: &Object) -> Json {
    let text = |name: &str| match key.get(name) {
        Value::Text(text) => Json::String(text),
        Value::Date(date) => Json::String(date.to_string()),
        Value::Bool(on) => Json::Bool(on),
        _ => Json::Null,
    };
    json!({
        "id": text("id"),
        "name": text("name"),
        "user_id": ravenrs_app::value_to_json(&key.get("user_id")),
        "roles": text("roles"),
        "active": text("active"),
        "expires": text("expires"),
    })
}

fn stored(failed: ravenrs_app::SpaceError) -> Failure {
    Failure::from(failed)
}

impl ApiService {
    pub(crate) fn session_of_key(&self, token: &str) -> Result<Who, Failure> {
        let parts = token.strip_prefix(PREFIX).and_then(|rest| rest.split_once('_'));
        let (id, secret) = parts.ok_or_else(Failure::unauthorized)?;
        let space = (self.space)();
        let key = space
            .find(SECURITY_API_KEY, &Value::Text(id.to_owned()))
            .map_err(stored)?
            .ok_or_else(Failure::unauthorized)?;
        let Value::Text(hash) = key.get("secret_hash") else { return Err(Failure::unauthorized()) };
        if !same(&hash, &hash_of(secret)) || key.get("active") != Value::Bool(true) {
            return Err(Failure::unauthorized());
        }
        match key.get("expires") {
            Value::Null => {}
            Value::Date(expires) => {
                let now = local_now();
                if (expires.days, expires.seconds) <= (now.days, now.seconds) {
                    return Err(Failure::unauthorized());
                }
            }
            // A date nobody can read is not a key that lasts forever.
            _ => return Err(Failure::unauthorized()),
        }
        // Who the user is now: losing a role or being turned off counts at once for their keys.
        let owner = self
            .directory
            .identity_by_key(&key.get("user_id"))
            .map_err(stored)?
            .ok_or_else(Failure::unauthorized)?;
        let identity = narrowed(owner, &key, self.merge);
        let name = format!("{} ({})", identity.name, text_of(&key, "name"));
        let mut context =
            SecurityContext::new(identity, self.directory.clone(), self.entities.clone(), self.merge);
        let trail = self.audit.clone().map(|settings| {
            let trail = Rc::new(AuditTrail::new(settings, move || name.clone()));
            trail.write_with(self.space.clone());
            trail
        });
        if let Some(trail) = &trail {
            context = context.with_audit_trail(trail.clone());
        }
        Ok((Rc::new(context), trail))
    }

    fn only_administrators(&self, who: &Who) -> Result<(), Failure> {
        if who.0.with(Permissions::is_administrator) { Ok(()) } else { Err(Failure::denied()) }
    }

    /// `name`, and optionally `user`, `roles` and `expires_in_days`. The key shows here and never again.
    pub(crate) fn make_key(&self, request: &ApiRequest, who: &Who) -> Result<(u16, Json), Failure> {
        self.only_administrators(who)?;
        let body = body_of(request)?;
        let name = body
            .get("name")
            .and_then(Json::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| Failure::bad_request("A key has a name."))?;
        let owner = match body.get("user").and_then(Json::as_str) {
            Some(user) => self
                .directory
                .identity_of(user)
                .map_err(stored)?
                .ok_or_else(|| Failure::bad_request("There is no such active user."))?,
            None => who.0.identity(),
        };
        let roles = roles_asked(&body, &owner, self.merge)?;
        let mut space = (self.space)();
        let mut key = space.new_object(SECURITY_API_KEY).map_err(stored)?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        let secret = new_secret();
        key.set("id", id.clone());
        key.set("name", name);
        key.set("user_id", owner.key.clone());
        key.set("roles", roles);
        key.set("active", true);
        key.set("secret_hash", hash_of(&secret));
        if let Some(days) = body.get("expires_in_days") {
            let days = days
                .as_i64()
                .filter(|days| (1..=MOST_DAYS).contains(days))
                .ok_or_else(|| Failure::bad_request("expires_in_days goes from 1 to ten years."))?;
            let now = local_now();
            key.set(
                "expires",
                Value::Date(ravenrs_criteria::DateTime { days: now.days + days, seconds: now.seconds }),
            );
        }
        space.insert(key.clone()).map_err(stored)?;
        space.commit().map_err(stored)?;
        note(who, &key, "API key made");
        let mut answer = public(&key);
        if let Some(object) = answer.as_object_mut() {
            object.insert("key".to_owned(), json!(format!("{PREFIX}{id}_{secret}")));
        }
        Ok((201, answer))
    }

    pub(crate) fn list_keys(&self, who: &Who) -> Result<(u16, Json), Failure> {
        self.only_administrators(who)?;
        let found = (self.space)().query(SECURITY_API_KEY, &Query::all()).map_err(stored)?;
        Ok((200, json!({ "items": found.iter().map(public).collect::<Vec<_>>() })))
    }

    pub(crate) fn rotate_key(&self, id: &str, who: &Who) -> Result<(u16, Json), Failure> {
        self.only_administrators(who)?;
        let mut space = (self.space)();
        let mut key = space
            .find(SECURITY_API_KEY, &Value::Text(id.to_owned()))
            .map_err(stored)?
            .ok_or_else(Failure::not_found)?;
        let secret = new_secret();
        key.set("secret_hash", hash_of(&secret));
        space.save(key.clone()).map_err(stored)?;
        space.commit().map_err(stored)?;
        note(who, &key, "API key rotated");
        let mut answer = public(&key);
        if let Some(object) = answer.as_object_mut() {
            object.insert("key".to_owned(), json!(format!("{PREFIX}{id}_{secret}")));
        }
        Ok((200, answer))
    }

    pub(crate) fn revoke_key(&self, id: &str, who: &Who) -> Result<(u16, Json), Failure> {
        self.only_administrators(who)?;
        let mut space = (self.space)();
        let key = space
            .find(SECURITY_API_KEY, &Value::Text(id.to_owned()))
            .map_err(stored)?
            .ok_or_else(Failure::not_found)?;
        space.delete(SECURITY_API_KEY, &key.get("id")).map_err(stored)?;
        space.commit().map_err(stored)?;
        note(who, &key, "API key revoked");
        Ok((200, json!({})))
    }
}

fn text_of(key: &Object, field: &str) -> String {
    match key.get(field) {
        Value::Text(text) => text,
        _ => String::new(),
    }
}

/// The roles of the user that the key may use: all of them, if it names none. When every role has
/// to allow something, leaving one out would take away what it denies, so a key keeps them all.
fn narrowed(mut owner: Identity, key: &Object, merge: RoleMerge) -> Identity {
    let wanted = text_of(key, "roles");
    if wanted.is_empty() || merge != RoleMerge::default() {
        return owner;
    }
    let named: Vec<&str> = wanted.split(',').map(str::trim).filter(|name| !name.is_empty()).collect();
    owner.roles.retain(|role| named.contains(&role.name()));
    owner
}

fn roles_asked(body: &Map<String, Json>, owner: &Identity, merge: RoleMerge) -> Result<String, Failure> {
    let Some(asked) = body.get("roles") else { return Ok(String::new()) };
    if merge != RoleMerge::default() {
        return Err(Failure::bad_request(
            "Here every role has to allow something, so a key cannot leave any of them out.",
        ));
    }
    let names = asked.as_array().ok_or_else(|| Failure::bad_request("roles is a list of names."))?;
    let mut kept = Vec::with_capacity(names.len());
    for name in names {
        let name = name.as_str().ok_or_else(|| Failure::bad_request("roles is a list of names."))?;
        // They are kept separated by commas, so one with a comma would read as others.
        if name.contains(',') || !owner.roles.iter().any(|role| role.name() == name) {
            return Err(Failure::bad_request(format!("The user does not have the role {name}.")));
        }
        kept.push(name);
    }
    if kept.is_empty() {
        return Err(Failure::bad_request("A key with roles names at least one."));
    }
    Ok(kept.join(","))
}

fn note(who: &Who, key: &Object, what: &str) {
    if let Some(trail) = &who.1 {
        trail.custom(SECURITY_API_KEY, &key.get("id"), "secret_hash", what);
    }
}
