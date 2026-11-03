//! Signing in and the people who can: users, passwords and being locked out.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use ravenrs_criteria::{DateTime, Value};

use super::records::{
    ADMINISTRATORS_ROLE, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, is_security_entity,
    role_from_records,
};
use super::{AuditTrail, Identity, Permissions, Role, RoleMerge, hash_password, verify_password};
use crate::{EntityInfo, Object, ObjectSpace, Query, SpaceError};

/// The shortest password taken, in characters.
pub const MIN_PASSWORD_CHARS: usize = 8;
/// Wrong passwords in a row before an account waits.
const MOST_FAILED_ATTEMPTS: f64 = 5.0;
/// How long it waits, in minutes.
const LOCKED_MINUTES: i64 = 15;

/// What someone gave to sign in.
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct Credentials {
    /// Their name.
    pub name: String,
    /// Their password.
    pub password: String,
    /// Anything else another way of signing in asks for.
    pub extra: BTreeMap<String, String>,
}

impl Credentials {
    /// A name and a password.
    pub fn new(name: impl Into<String>, password: impl Into<String>) -> Self {
        Self { name: name.into(), password: password.into(), extra: BTreeMap::new() }
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials").field("name", &self.name).finish_non_exhaustive()
    }
}

/// Why someone could not sign in.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthError {
    /// The name, the password or the state of the account did not fit. It never says which, so
    /// this cannot be used to find out who has an account.
    Rejected,
    /// Storage failed.
    Storage(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected => f.write_str("the name or the password is not right"),
            Self::Storage(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for AuthError {}

/// A way of signing in. Passwords are [`UserDirectory`]; a way that talks to an outside provider,
/// as OAuth 2 does, implements this and asks [`UserDirectory::identity_of`] for the roles.
pub trait Authenticator {
    /// Who this is, or why not.
    fn authenticate(&self, credentials: &Credentials) -> Result<Identity, AuthError>;
}

/// Why a password could not be set.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PasswordError {
    /// It is shorter than [`MIN_PASSWORD_CHARS`].
    TooShort,
    /// The current password given is not right.
    WrongCurrent,
    /// This person cannot change that password.
    NotAllowed,
    /// There is no such user.
    NoSuchUser,
    /// Something else went wrong.
    Failed(String),
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "a password has at least {MIN_PASSWORD_CHARS} characters"),
            Self::WrongCurrent => f.write_str("the current password is not right"),
            Self::NotAllowed => f.write_str("this password cannot be changed by this person"),
            Self::NoSuchUser => f.write_str("there is no such user"),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Makes a unit of work with no permissions around it. It only ever goes to what needs to read
/// before anyone has signed in, or to what the security system does itself.
pub type SpaceMaker = Rc<dyn Fn() -> Box<dyn ObjectSpace>>;

thread_local! {
    /// What a password is compared with when there is no such user, so that takes as long.
    static NOBODY: String = hash_password("nobody has this password").unwrap_or_default();
}

fn failed(error: SpaceError) -> AuthError {
    AuthError::Storage(error.to_string())
}

/// The people who can sign in, kept as records.
#[derive(Clone)]
pub struct UserDirectory {
    space: SpaceMaker,
    roles: Rc<Vec<Role>>,
}

impl fmt::Debug for UserDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDirectory").field("roles", &self.roles).finish_non_exhaustive()
    }
}

impl UserDirectory {
    /// Over what this makes. `roles` are what the application declares in code: a role record of
    /// the same name gets what they say, and one is created if missing.
    pub fn new(space: SpaceMaker, roles: Vec<Role>) -> Result<Self, SpaceError> {
        let directory = Self { space, roles: Rc::new(roles) };
        directory.ensure_roles()?;
        Ok(directory)
    }

    fn ensure_roles(&self) -> Result<(), SpaceError> {
        let mut space = (self.space)();
        let existing = space.query(SECURITY_ROLE, &Query::all())?;
        for role in self.roles.iter() {
            if !existing.iter().any(|row| row.get("name") == Value::Text(role.name.clone())) {
                let mut record = space.new_object(SECURITY_ROLE)?;
                record.set("name", role.name.clone());
                record.set("administrative", role.administrative);
                space.save(record)?;
            }
        }
        space.commit()
    }

    /// Whether nobody can sign in yet, so the first person has to be created.
    pub fn is_empty(&self) -> Result<bool, SpaceError> {
        Ok((self.space)().query(SECURITY_USER, &Query::all())?.is_empty())
    }

    /// Creates the first person, as an administrator. It only works while there is nobody, so it
    /// cannot be used to get in later. Whoever opens a new installation first gets this, so it
    /// is for a program on someone's machine: a server creates its first user in its setup.
    pub fn create_first_administrator(&self, name: &str, password: &str) -> Result<Identity, PasswordError> {
        let wrong = |error: SpaceError| PasswordError::Failed(error.to_string());
        if !self.is_empty().map_err(wrong)? {
            return Err(PasswordError::NotAllowed);
        }
        if name.trim().is_empty() {
            return Err(PasswordError::Failed("a user has a name".to_owned()));
        }
        let hash = checked_hash(password)?;
        let mut space = (self.space)();
        let roles = space.query(SECURITY_ROLE, &Query::all()).map_err(wrong)?;
        let role = match roles.iter().find(|row| row.get("administrative") == Value::Bool(true)) {
            Some(role) => role.get("id"),
            None => {
                let mut record = space.new_object(SECURITY_ROLE).map_err(wrong)?;
                record.set("name", ADMINISTRATORS_ROLE);
                record.set("administrative", true);
                let key = record.get("id");
                space.save(record).map_err(wrong)?;
                key
            }
        };
        let mut user = space.new_object(SECURITY_USER).map_err(wrong)?;
        user.set("name", name.trim());
        user.set("password_hash", hash);
        user.set("must_change_password", false);
        let key = user.get("id");
        space.save(user).map_err(wrong)?;
        let mut link = space.new_object(SECURITY_USER_ROLE).map_err(wrong)?;
        link.set("user_id", key.clone());
        link.set("role_id", role);
        space.save(link).map_err(wrong)?;
        space.commit().map_err(wrong)?;
        self.identity_by_key(&key).map_err(wrong)?.ok_or(PasswordError::NoSuchUser)
    }

    fn user_named(&self, space: &dyn ObjectSpace, name: &str) -> Result<Option<Object>, SpaceError> {
        // Compared here, so a name never becomes part of an expression.
        Ok(space
            .query(SECURITY_USER, &Query::all())?
            .into_iter()
            .find(|user| user.get("name") == Value::Text(name.to_owned())))
    }

    fn roles_of(&self, space: &dyn ObjectSpace, user: &Value) -> Result<Vec<Role>, SpaceError> {
        let links = space.query(SECURITY_USER_ROLE, &Query::all())?;
        let records = space.query(SECURITY_ROLE, &Query::all())?;
        let mut roles = Vec::new();
        for link in links.iter().filter(|link| link.get("user_id") == *user) {
            let wanted = link.get("role_id");
            if let Some(record) = records.iter().find(|record| record.get("id") == wanted) {
                let role = role_from_records(space, record)?;
                let declared = self.roles.iter().find(|code| code.name == role.name);
                roles.push(match declared {
                    Some(code) => role.merged_with(code),
                    None => role,
                });
            }
        }
        Ok(roles)
    }

    fn identity_from(&self, space: &dyn ObjectSpace, user: &Object) -> Result<Identity, SpaceError> {
        let key = user.get("id");
        let name = match user.get("name") {
            Value::Text(name) => name,
            _ => String::new(),
        };
        let mut identity = Identity::new(key.clone(), name, self.roles_of(space, &key)?);
        identity.must_change_password = user.get("must_change_password") == Value::Bool(true);
        Ok(identity)
    }

    /// Who this name is, if they can sign in, for a way of signing in that checked them itself.
    pub fn identity_of(&self, name: &str) -> Result<Option<Identity>, SpaceError> {
        let space = (self.space)();
        match self.user_named(&*space, name)? {
            Some(user) if user.get("active") == Value::Bool(true) => {
                self.identity_from(&*space, &user).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Who this key is, as they stand now; nobody once they cannot sign in.
    pub fn identity_by_key(&self, key: &Value) -> Result<Option<Identity>, SpaceError> {
        let space = (self.space)();
        match space.find(SECURITY_USER, key)? {
            Some(user) if user.get("active") == Value::Bool(true) => {
                self.identity_from(&*space, &user).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Whether someone active is left who can do everything, in what this space holds.
    pub(crate) fn has_active_administrator(&self, space: &dyn ObjectSpace) -> Result<bool, SpaceError> {
        for user in space.query(SECURITY_USER, &Query::all())? {
            if user.get("active") == Value::Bool(true)
                && self.roles_of(space, &user.get("id"))?.iter().any(Role::is_administrative)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Sets the password of a user, with no questions: who may do this is up to the caller.
    fn write_password(&self, key: &Value, password: &str, temporary: bool) -> Result<(), PasswordError> {
        let hash = checked_hash(password)?;
        let wrong = |error: SpaceError| PasswordError::Failed(error.to_string());
        let mut space = (self.space)();
        let mut user = space.find(SECURITY_USER, key).map_err(wrong)?.ok_or(PasswordError::NoSuchUser)?;
        user.set("password_hash", hash);
        user.set("must_change_password", temporary);
        user.set("failed_attempts", 0.0);
        user.set("locked_until", Value::Null);
        space.save(user).map_err(wrong)?;
        space.commit().map_err(wrong)
    }

    fn check_password(&self, key: &Value, password: &str) -> Result<bool, PasswordError> {
        let user = (self.space)()
            .find(SECURITY_USER, key)
            .map_err(|e| PasswordError::Failed(e.to_string()))?
            .ok_or(PasswordError::NoSuchUser)?;
        Ok(matches!(user.get("password_hash"), Value::Text(hash) if verify_password(password, &hash)))
    }
}

fn checked_hash(password: &str) -> Result<String, PasswordError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(PasswordError::TooShort);
    }
    hash_password(password).map_err(PasswordError::Failed)
}

impl Authenticator for UserDirectory {
    fn authenticate(&self, credentials: &Credentials) -> Result<Identity, AuthError> {
        let mut space = (self.space)();
        let Some(mut user) = self.user_named(&*space, &credentials.name).map_err(failed)? else {
            NOBODY.with(|hash| verify_password(&credentials.password, hash));
            return Err(AuthError::Rejected);
        };
        let now = DateTime::now();
        // While it waits the password is not even looked at, so guessing goes nowhere.
        if let Value::Date(until) = user.get("locked_until")
            && (until.days, until.seconds) > (now.days, now.seconds)
        {
            NOBODY.with(|hash| verify_password(&credentials.password, hash));
            return Err(AuthError::Rejected);
        }
        let right = matches!(
            user.get("password_hash"),
            Value::Text(hash) if verify_password(&credentials.password, &hash)
        );
        if !right {
            if !matches!(user.get("password_hash"), Value::Text(_)) {
                NOBODY.with(|hash| verify_password(&credentials.password, hash));
            }
            let attempts = match user.get("failed_attempts") {
                Value::Number(count) => count + 1.0,
                _ => 1.0,
            };
            if attempts >= MOST_FAILED_ATTEMPTS {
                user.set("failed_attempts", 0.0);
                let seconds = i64::from(now.seconds) + LOCKED_MINUTES * 60;
                user.set(
                    "locked_until",
                    Value::Date(DateTime {
                        days: now.days + seconds.div_euclid(86_400),
                        seconds: seconds.rem_euclid(86_400) as u32,
                    }),
                );
            } else {
                user.set("failed_attempts", attempts);
            }
            space.save(user).map_err(failed)?;
            space.commit().map_err(failed)?;
            return Err(AuthError::Rejected);
        }
        if user.get("active") != Value::Bool(true) {
            return Err(AuthError::Rejected);
        }
        if user.get("failed_attempts") != Value::Number(0.0) || user.get("locked_until") != Value::Null {
            user.set("failed_attempts", 0.0);
            user.set("locked_until", Value::Null);
            space.save(user.clone()).map_err(failed)?;
            space.commit().map_err(failed)?;
        }
        self.identity_from(&*space, &user).map_err(failed)
    }
}

/// Who is signed in and what the security system needs to answer for them. One per signed-in
/// person, shared by their units of work.
pub struct SecurityContext {
    permissions: RefCell<Permissions>,
    directory: UserDirectory,
    entities: Vec<EntityInfo>,
    merge: RoleMerge,
    trail: Option<Rc<AuditTrail>>,
    denied: String,
}

impl fmt::Debug for SecurityContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityContext").field("permissions", &self.permissions).finish_non_exhaustive()
    }
}

impl SecurityContext {
    /// For this person, over these entities.
    pub fn new(
        identity: Identity,
        directory: UserDirectory,
        entities: Vec<EntityInfo>,
        merge: RoleMerge,
    ) -> Self {
        let permissions = Permissions::new(identity, entities.clone(), merge);
        Self {
            permissions: RefCell::new(permissions),
            directory,
            entities,
            merge,
            trail: None,
            denied: "You are not allowed to do this.".to_owned(),
        }
    }

    /// Writes what the security system does itself there.
    pub fn with_audit_trail(mut self, trail: Rc<AuditTrail>) -> Self {
        self.trail = Some(trail);
        self
    }

    /// What people are told when something is refused.
    pub fn with_denied_message(mut self, message: impl Into<String>) -> Self {
        self.denied = message.into();
        self
    }

    /// Asks what this person can do.
    pub fn with<T>(&self, ask: impl FnOnce(&Permissions) -> T) -> T {
        ask(&self.permissions.borrow())
    }

    /// Who is signed in.
    pub fn identity(&self) -> Identity {
        self.permissions.borrow().identity().clone()
    }

    pub(crate) fn denied(&self) -> SpaceError {
        SpaceError::Denied(self.denied.clone())
    }

    pub(crate) fn directory(&self) -> &UserDirectory {
        &self.directory
    }

    /// Reads who they are again, after their records may have changed: losing a role, or being
    /// turned off, takes effect now. Someone no longer active keeps no permission at all.
    pub fn reload(&self) -> Result<(), SpaceError> {
        let current = self.identity();
        let identity = self
            .directory
            .identity_by_key(&current.key)?
            .unwrap_or(Identity { roles: Vec::new(), ..current });
        *self.permissions.borrow_mut() = Permissions::new(identity, self.entities.clone(), self.merge);
        Ok(())
    }

    /// Changes this person's own password, which takes the current one.
    pub fn change_own_password(&self, current: &str, new: &str) -> Result<(), PasswordError> {
        let key = self.identity().key;
        let active =
            self.directory.identity_by_key(&key).map_err(|e| PasswordError::Failed(e.to_string()))?;
        if active.is_none() {
            return Err(PasswordError::NotAllowed);
        }
        if !self.directory.check_password(&key, current)? {
            return Err(PasswordError::WrongCurrent);
        }
        self.directory.write_password(&key, new, false)?;
        self.note_password_change(&key);
        self.reload().map_err(|e| PasswordError::Failed(e.to_string()))
    }

    /// An administrator gives someone a password, which they have to change when they sign in.
    pub fn set_password_of(&self, user: &Value, password: &str) -> Result<(), PasswordError> {
        if !self.with(Permissions::is_administrator) {
            return Err(PasswordError::NotAllowed);
        }
        self.directory.write_password(user, password, true)?;
        self.note_password_change(user);
        Ok(())
    }

    /// The hash itself never goes in the trail, only that it happened.
    fn note_password_change(&self, user: &Value) {
        if let Some(trail) = &self.trail {
            trail.custom(SECURITY_USER, user, "password_hash", "Password changed");
        }
    }

    /// Whether a change touches what decides who can do what.
    pub(crate) fn touches_security(&self, entity: &str) -> bool {
        is_security_entity(entity)
    }
}
