//! What a module says about itself, so that putting an application together refuses what does not
//! fit before anyone reaches a screen.

use std::fmt;

use crate::space::SpaceWatcher;
use crate::{Change, EntityInfo, Object, ObjectSpace, Query, SpaceError, Value};

/// A version as `major.minor.patch`; what is missing counts as zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Version {
    /// Changes that break what came before.
    pub major: u32,
    /// What is added and keeps what was there.
    pub minor: u32,
    /// Corrections.
    pub patch: u32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Version {
    /// Reads `1`, `1.2` or `1.2.3`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parts = text.trim().split('.').map(|part| part.parse::<u32>());
        let mut next = |required: bool| match parts.next() {
            Some(Ok(number)) => Ok(number),
            None if !required => Ok(0),
            _ => Err(format!("{text} is not a version")),
        };
        let version = Self { major: next(true)?, minor: next(false)?, patch: next(false)? };
        match parts.next() {
            None => Ok(version),
            Some(_) => Err(format!("{text} is not a version")),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The versions of another module that one accepts: from one, included, up to another, left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct VersionRange {
    /// The first accepted.
    pub from: Version,
    /// The first no longer accepted.
    pub below: Version,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl VersionRange {
    /// From this version up to the next major one, as `^1.2` says.
    pub fn compatible(from: &str) -> Result<Self, String> {
        let from = Version::parse(from)?;
        Ok(Self { from, below: Version { major: from.major + 1, minor: 0, patch: 0 } })
    }

    /// Whether it accepts this version.
    pub fn accepts(&self, version: Version) -> bool {
        self.from <= version && version < self.below
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "from {} and below {}", self.from, self.below)
    }
}

/// Where an application runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Target {
    /// A native window.
    Desktop,
    /// A browser.
    Web,
    /// A service answering others.
    Server,
    /// A process that runs jobs, with no one in front of it.
    Worker,
}

/// What a module says about itself. Everything is optional: a module that says nothing is accepted
/// everywhere, as before.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ModuleManifest {
    /// Its own version.
    pub version: Option<Version>,
    /// The versions it accepts of the modules it depends on, by name.
    pub requires: Vec<(String, VersionRange)>,
    /// Where it runs; none means anywhere.
    pub targets: Vec<Target>,
    /// The storage it works on, by name such as `sqlite` or `postgres`; none means any.
    pub databases: Vec<String>,
    /// Capabilities of providers the host has to offer, such as `events.status`.
    pub capabilities: Vec<String>,
    /// Roles it adds, by name.
    pub roles: Vec<String>,
    /// Migrations it brings, by id.
    pub migrations: Vec<String>,
    /// Jobs it runs, by id.
    pub jobs: Vec<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ModuleManifest {
    /// Says nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Its own version, as `1.2.3`.
    pub fn version(mut self, version: &str) -> Result<Self, String> {
        self.version = Some(Version::parse(version)?);
        Ok(self)
    }

    /// Accepts these versions of a module it depends on.
    pub fn requires(mut self, module: impl Into<String>, range: VersionRange) -> Self {
        self.requires.push((module.into(), range));
        self
    }

    /// Runs here.
    pub fn target(mut self, target: Target) -> Self {
        self.targets.push(target);
        self
    }

    /// Works on this storage.
    pub fn database(mut self, database: impl Into<String>) -> Self {
        self.databases.push(database.into());
        self
    }

    /// Needs the host to offer this capability.
    pub fn capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    /// Adds this role.
    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    /// Brings this migration.
    pub fn migration(mut self, id: impl Into<String>) -> Self {
        self.migrations.push(id.into());
        self
    }

    /// Runs this job.
    pub fn job(mut self, id: impl Into<String>) -> Self {
        self.jobs.push(id.into());
        self
    }
}

/// A unit of work that keeps the records of switched-off modules for reading and refuses to write
/// them, with [`SpaceError::Unavailable`].
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct GuardedObjectSpace {
    inner: Box<dyn ObjectSpace>,
    closed: Vec<String>,
}

impl fmt::Debug for GuardedObjectSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuardedObjectSpace").field("closed", &self.closed).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl GuardedObjectSpace {
    /// Over this space, closing these entities to writing.
    pub fn new(inner: Box<dyn ObjectSpace>, closed: Vec<String>) -> Self {
        Self { inner, closed }
    }

    fn open(&self, entity: &str) -> Result<(), SpaceError> {
        if self.closed.iter().any(|name| name == entity) {
            Err(SpaceError::Unavailable(format!(
                "{entity} belongs to a module that is switched off: it can be read, not changed"
            )))
        } else {
            Ok(())
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "objectspace"))]
impl ObjectSpace for GuardedObjectSpace {
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
        self.open(entity)?;
        self.inner.new_object(entity)
    }

    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        self.open(object.entity())?;
        self.inner.save(object)
    }

    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        self.open(object.entity())?;
        self.inner.insert(object)
    }

    fn delete(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError> {
        self.open(entity)?;
        self.inner.delete(entity, key)
    }

    fn changes(&self) -> Vec<Change> {
        self.inner.changes()
    }

    fn commit(&mut self) -> Result<(), SpaceError> {
        self.inner.commit()
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
