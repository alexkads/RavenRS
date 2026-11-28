//! Access for a while: a user made for someone, such as support, with a reason and a deadline,
//! who is turned off by themselves when it passes; and revoking a user in one step.
//!
//! The user, its role and the record of the access are written in one commit. Past the deadline
//! the user cannot sign in, the next reload of an open session leaves it no permission, and the
//! access is ended with the reason. Revoking turns the user off, their API keys too, and ends what
//! they had been granted.

use std::fmt;
use std::rc::Rc;
use std::time::Duration;

use ravenrs_criteria::{DateTime, Value};

use super::directory::{SecurityContext, UserDirectory};
use super::records::{
    SECURITY_API_KEY, SECURITY_ROLE, SECURITY_TEMPORARY_ACCESS, SECURITY_USER, SECURITY_USER_ROLE,
};
use super::{AuditedObjectSpace, Permissions, hash_password};
use crate::{Object, ObjectSpace, Query, SpaceError};

/// What `ended_reason` says of an access whose deadline passed.
pub const ACCESS_EXPIRED: &str = "Expired";

/// The clock a policy reads the time from.
type Clock = Rc<dyn Fn() -> DateTime>;

/// How access for a while is given: the role the user gets, the longest it lasts, the reason it
/// needs and how the user is named.
///
/// ```
/// use std::time::Duration;
/// use ravenrs_app::TemporaryAccessPolicy;
///
/// let policy = TemporaryAccessPolicy::new("Support")
///     .max_hours(72)
///     .min_reason(10)
///     .name_prefix("support-")
///     .check_every(Duration::from_secs(60));
/// assert_eq!(policy.role, "Support");
/// ```
#[derive(Clone)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TemporaryAccessPolicy {
    /// The role the user gets, which is not administrative.
    pub role: String,
    /// The longest it lasts, in hours.
    pub max_hours: u32,
    /// How many characters the reason needs at least.
    pub min_reason: usize,
    /// What the name of each user made begins with.
    pub name_prefix: String,
    /// How often an open session looks for what has expired.
    pub check_every: Duration,
    clock: Clock,
}

impl fmt::Debug for TemporaryAccessPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TemporaryAccessPolicy")
            .field("role", &self.role)
            .field("max_hours", &self.max_hours)
            .field("min_reason", &self.min_reason)
            .field("name_prefix", &self.name_prefix)
            .field("check_every", &self.check_every)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TemporaryAccessPolicy {
    /// Users made with this role, for up to 72 hours, with a reason of 10 characters or more.
    pub fn new(role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            max_hours: 72,
            min_reason: 10,
            name_prefix: "temporary-".to_owned(),
            check_every: Duration::from_secs(60),
            clock: Rc::new(crate::local_now),
        }
    }

    /// The longest an access lasts, in hours.
    pub fn max_hours(mut self, hours: u32) -> Self {
        self.max_hours = hours.max(1);
        self
    }

    /// How many characters the reason needs at least.
    pub fn min_reason(mut self, characters: usize) -> Self {
        self.min_reason = characters;
        self
    }

    /// What the name of each user made begins with.
    pub fn name_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.name_prefix = prefix.into();
        self
    }

    /// How often an open session looks for what has expired.
    pub fn check_every(mut self, every: Duration) -> Self {
        self.check_every = every;
        self
    }

    /// Reads the time from this, so a test does not have to wait.
    pub fn clock(mut self, clock: impl Fn() -> DateTime + 'static) -> Self {
        self.clock = Rc::new(clock);
        self
    }

    /// The time now, by its clock.
    pub fn now(&self) -> DateTime {
        (self.clock)()
    }
}

/// What a grant gives: who to sign in as, until when, and the password, shown once.
#[derive(Clone, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TemporaryAccess {
    /// The name to sign in with.
    pub user_name: String,
    /// The key of the user made.
    pub user_key: Value,
    /// The password, which is kept nowhere: whoever grants hands it over.
    pub password: String,
    /// When it stops.
    pub expires_at: DateTime,
}

impl fmt::Debug for TemporaryAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TemporaryAccess")
            .field("user_name", &self.user_name)
            .field("user_key", &self.user_key)
            .field("password", &"…")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Why access was not granted or revoked.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum AccessError {
    /// Only an administrator grants and revokes.
    NotAllowed,
    /// Nobody revokes themselves.
    OwnUser,
    /// The reason needs at least this many characters.
    ReasonTooShort(usize),
    /// It lasts from 1 hour up to this many.
    Hours(u32),
    /// The role of the policy is missing, or it is administrative.
    Role(String),
    /// There is no such user.
    NoSuchUser,
    /// It would leave nobody active who can do everything.
    LastAdministrator,
    /// Storage failed or refused.
    Failed(String),
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAllowed => f.write_str("only an administrator grants and revokes access"),
            Self::OwnUser => f.write_str("nobody revokes their own user"),
            Self::ReasonTooShort(at_least) => write!(f, "the reason needs at least {at_least} characters"),
            Self::Hours(most) => write!(f, "an access lasts from 1 to {most} hours"),
            Self::Role(why) => f.write_str(why),
            Self::NoSuchUser => f.write_str("there is no such user"),
            Self::LastAdministrator => f.write_str("there has to be an active administrator"),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for AccessError {}

impl From<SpaceError> for AccessError {
    fn from(error: SpaceError) -> Self {
        Self::Failed(error.to_string())
    }
}

/// This time plus some hours.
pub(crate) fn hours_after(time: DateTime, hours: u32) -> DateTime {
    let seconds = i64::from(time.seconds) + i64::from(hours) * 3600;
    DateTime { days: time.days + seconds.div_euclid(86_400), seconds: seconds.rem_euclid(86_400) as u32 }
}

/// Whether a deadline kept in a record has passed at this time. No deadline never passes.
pub(crate) fn expired(record: &Object, field: &str, now: DateTime) -> bool {
    matches!(record.get(field), Value::Date(until) if (until.days, until.seconds) <= (now.days, now.seconds))
}

/// Random text from the system, as the salt of a password: each v4 UUID has 122 random bits.
fn random_text(characters: usize) -> String {
    let mut text = String::new();
    while text.len() < characters {
        text.push_str(&uuid::Uuid::new_v4().simple().to_string());
    }
    text.truncate(characters);
    text
}

/// Ends the open accesses of a user, in this unit of work.
fn end_accesses(
    space: &mut dyn ObjectSpace,
    user: &Value,
    now: DateTime,
    reason: &str,
) -> Result<(), SpaceError> {
    for mut access in space.query(SECURITY_TEMPORARY_ACCESS, &Query::all())? {
        if access.get("user_id") == *user && access.get("ended_at") == Value::Null {
            access.set("ended_at", Value::Date(now));
            access.set("ended_reason", reason);
            space.save(access)?;
        }
    }
    Ok(())
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part2"))]
impl UserDirectory {
    /// Turns off the users whose deadline passed at `now` and ends their accesses as
    /// [`ACCESS_EXPIRED`], in one commit. Says how many users it turned off.
    pub fn end_expired(&self, now: DateTime) -> Result<usize, SpaceError> {
        let mut space = self.raw();
        let mut ended = 0;
        for mut user in space.query(SECURITY_USER, &Query::all())? {
            if !expired(&user, "expires_at", now) {
                continue;
            }
            let key = user.get("id");
            if user.get("active") == Value::Bool(true) {
                user.set("active", false);
                space.save(user)?;
                ended += 1;
            }
            end_accesses(&mut *space, &key, now, ACCESS_EXPIRED)?;
        }
        if space.changes().is_empty() {
            return Ok(0);
        }
        space.commit()?;
        Ok(ended)
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl SecurityContext {
    /// A unit of work with no permissions, written to the audit trail when there is one.
    fn system_space(&self) -> Box<dyn ObjectSpace> {
        let raw = self.directory().raw();
        match self.trail() {
            Some(trail) => Box::new(AuditedObjectSpace::new(raw, trail.clone())),
            None => raw,
        }
    }

    /// An administrator gives access for `hours`, with a reason: a new user with the role of the
    /// policy and a random password, who stops by themselves when it passes, and the record of
    /// the access with who granted it. All in one commit.
    pub fn grant_temporary_access(
        &self,
        policy: &TemporaryAccessPolicy,
        reason: &str,
        hours: u32,
    ) -> Result<TemporaryAccess, AccessError> {
        if !self.with(Permissions::is_administrator) {
            return Err(AccessError::NotAllowed);
        }
        if reason.trim().chars().count() < policy.min_reason {
            return Err(AccessError::ReasonTooShort(policy.min_reason));
        }
        if hours == 0 || hours > policy.max_hours {
            return Err(AccessError::Hours(policy.max_hours));
        }
        let mut space = self.system_space();
        let role = space
            .query(SECURITY_ROLE, &Query::all())?
            .into_iter()
            .find(|role| role.get("name") == Value::Text(policy.role.clone()))
            .ok_or_else(|| AccessError::Role(format!("there is no role {}", policy.role)))?;
        if role.get("administrative") == Value::Bool(true) {
            return Err(AccessError::Role(format!("{} is administrative", policy.role)));
        }
        let now = policy.now();
        let until = hours_after(now, hours);
        let password = random_text(20);
        let hash = hash_password(&password).map_err(AccessError::Failed)?;
        let name = format!("{}{}", policy.name_prefix, random_text(8));
        let mut user = space.new_object(SECURITY_USER)?;
        user.set("name", name.clone());
        user.set("active", true);
        user.set("must_change_password", false);
        user.set("password_hash", hash);
        user.set("expires_at", Value::Date(until));
        let key = user.get("id");
        space.insert(user)?;
        let mut link = space.new_object(SECURITY_USER_ROLE)?;
        link.set("user_id", key.clone());
        link.set("role_id", role.get("id"));
        space.insert(link)?;
        let mut access = space.new_object(SECURITY_TEMPORARY_ACCESS)?;
        access.set("user_id", key.clone());
        access.set("reason", reason.trim());
        access.set("granted_by", self.identity().name);
        access.set("granted_at", Value::Date(now));
        access.set("expires_at", Value::Date(until));
        space.insert(access)?;
        if let Err(error) = space.commit() {
            space.rollback();
            return Err(error.into());
        }
        Ok(TemporaryAccess { user_name: name, user_key: key, password, expires_at: until })
    }

    /// An administrator turns a user off, with their API keys, and ends the accesses they had, in
    /// one commit; an open session of theirs keeps no permission from its next reload. Nobody
    /// revokes themselves, nor the last active administrator.
    pub fn revoke_user(&self, user: &Value) -> Result<(), AccessError> {
        if !self.with(Permissions::is_administrator) {
            return Err(AccessError::NotAllowed);
        }
        let me = self.identity();
        if *user == me.key {
            return Err(AccessError::OwnUser);
        }
        let mut space = self.system_space();
        let mut found = space.find(SECURITY_USER, user)?.ok_or(AccessError::NoSuchUser)?;
        found.set("active", false);
        space.save(found)?;
        for mut key in space.query(SECURITY_API_KEY, &Query::all())? {
            if key.get("user_id") == *user && key.get("active") != Value::Bool(false) {
                key.set("active", false);
                space.save(key)?;
            }
        }
        end_accesses(&mut *space, user, crate::local_now(), &format!("Revoked by {}", me.name))?;
        if !self.directory().has_active_administrator(&*space)? {
            space.rollback();
            return Err(AccessError::LastAdministrator);
        }
        if let Err(error) = space.commit() {
            space.rollback();
            return Err(error.into());
        }
        Ok(())
    }
}
