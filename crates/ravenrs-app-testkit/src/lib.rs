//! Synthetic data for testing a reusable `ravenrs-app` module, as `docs/APP-CONTRACTS.md` describes
//! it: two organizations, T1 and T2, that use **the same local keys** on purpose, so that mixing one
//! with the other shows; units and people in each; example roles; and a clock that moves only when
//! a test says so.
//!
//! Each organization keeps its records apart, in memory or in a SQLite file of its own. Nothing here
//! belongs to a sector: the module under test brings its own entities, and the roles are an example
//! policy of the test, not profiles built into RavenRS.
//!
//! ```
//! use ravenrs_app::{Identity, ObjectSpace, Query};
//! use ravenrs_app_testkit::{Fixture, PERSON, roles};
//!
//! let fixture = Fixture::memory(Vec::new()).unwrap();
//! let registrar = Identity::new("op", "op", vec![roles::registrar("U1")]);
//! let space = fixture.t1.space_as(registrar);
//! let people = space.query(PERSON, &Query::all()).unwrap();
//! assert_eq!(people.len(), 1); // only the person of U1
//! ```

use std::cell::Cell;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use ravenrs_app::{
    EntityInfo, FieldInfo, FieldKind, Identity, MemoryDatabase, ObjectSpace, RoleMerge, SecuredObjectSpace,
    SecurityContext, SpaceError, SpaceMaker, UserDirectory, Value, security_entities,
};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};

/// The entity of units, the scope of access inside an organization.
pub const UNIT: &str = "Unit";
/// The entity of people.
pub const PERSON: &str = "Person";

/// A unit: a key such as `U1` and a name.
pub fn unit_entity() -> EntityInfo {
    EntityInfo::new(UNIT)
        .field(FieldInfo::new("id", FieldKind::Text).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
}

/// A person: a number for a key, the unit they belong to, a name, a contact and a reserved note
/// that only some roles may read.
pub fn person_entity() -> EntityInfo {
    EntityInfo::new(PERSON)
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("unit", FieldKind::Text))
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("contact", FieldKind::Text))
        .field(FieldInfo::new("note", FieldKind::LongText))
}

/// The entities every organization of the fixture has: units, people and those of security.
pub fn fixture_entities() -> Vec<EntityInfo> {
    let mut all = vec![unit_entity(), person_entity()];
    all.extend(security_entities());
    all
}

/// One organization, with its records kept apart from every other.
pub struct Organization {
    name: String,
    entities: Vec<EntityInfo>,
    maker: SpaceMaker,
    directory: UserDirectory,
}

impl fmt::Debug for Organization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Organization").field("name", &self.name).finish_non_exhaustive()
    }
}

impl Organization {
    /// An organization kept in memory, with the fixture's entities and these others.
    pub fn memory(name: impl Into<String>, extra: Vec<EntityInfo>) -> Result<Self, String> {
        let entities = with_fixture(extra);
        let database = MemoryDatabase::new(entities.clone()).map_err(|e| e.to_string())?;
        let maker: SpaceMaker = Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>);
        Self::over(name.into(), entities, maker)
    }

    /// An organization kept in a SQLite file, created when it is not there. Opening the same file
    /// again is how a test restarts an application.
    pub fn sqlite(name: impl Into<String>, file: &Path, extra: Vec<EntityInfo>) -> Result<Self, String> {
        let entities = with_fixture(extra);
        let connection = Rc::new(rusqlite::Connection::open(file).map_err(|e| e.to_string())?);
        let mut builder = SqliteDatabase::builder(connection);
        for entity in &entities {
            let table = entity.name.to_lowercase();
            builder = builder.managed(entity.clone(), TableMap::new(table)).map_err(|e| e.to_string())?;
        }
        let database = builder.build();
        let maker: SpaceMaker = Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>);
        Self::over(name.into(), entities, maker)
    }

    fn over(name: String, entities: Vec<EntityInfo>, maker: SpaceMaker) -> Result<Self, String> {
        let directory = UserDirectory::new(maker.clone(), Vec::new()).map_err(|e| e.to_string())?;
        Ok(Self { name, entities, maker, directory })
    }

    /// What it is called, which is also what tells its keys apart from another organization's.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// A unit of work with no security, for putting records in place and for checking what was kept.
    pub fn raw(&self) -> Box<dyn ObjectSpace> {
        (self.maker)()
    }

    /// A unit of work as this person, every read and write checked by their roles.
    pub fn space_as(&self, identity: Identity) -> SecuredObjectSpace {
        let context =
            SecurityContext::new(identity, self.directory.clone(), self.entities.clone(), RoleMerge::AnyRole);
        SecuredObjectSpace::new(self.raw(), Rc::new(context))
    }

    /// Puts a unit in place, once.
    pub fn add_unit(&self, id: &str, name: &str) -> Result<(), SpaceError> {
        let mut space = self.raw();
        if space.find(UNIT, &Value::Text(id.to_owned()))?.is_none() {
            let mut unit = space.new_object(UNIT)?;
            unit.set("id", id);
            unit.set("name", name);
            space.save(unit)?;
            space.commit()?;
        }
        Ok(())
    }

    /// Puts a person in place, once.
    pub fn add_person(&self, id: i64, unit: &str, name: &str, note: &str) -> Result<(), SpaceError> {
        let mut space = self.raw();
        if space.find(PERSON, &Value::Number(id as f64))?.is_none() {
            let mut person = space.new_object(PERSON)?;
            person.set("id", id as f64);
            person.set("unit", unit);
            person.set("name", name);
            person.set("contact", format!("{name} contact"));
            person.set("note", note);
            space.save(person)?;
            space.commit()?;
        }
        Ok(())
    }
}

fn with_fixture(extra: Vec<EntityInfo>) -> Vec<EntityInfo> {
    let mut entities = fixture_entities();
    entities.extend(extra);
    entities
}

/// The two organizations of the fixture.
///
/// T1 has units U1 and U2, person 1 in U1 and person 2 in U2. T2 has unit U1 and its own person 1,
/// with the same key as the person 1 of T1.
#[derive(Debug)]
pub struct Fixture {
    /// The first organization.
    pub t1: Organization,
    /// The second, with the same local keys as the first.
    pub t2: Organization,
}

impl Fixture {
    /// Both kept in memory, with these entities besides the fixture's.
    pub fn memory(extra: Vec<EntityInfo>) -> Result<Self, String> {
        let fixture =
            Self { t1: Organization::memory("T1", extra.clone())?, t2: Organization::memory("T2", extra)? };
        fixture.seed().map_err(|e| e.to_string())?;
        Ok(fixture)
    }

    /// Each kept in its own SQLite file inside this folder. Opening the same folder again finds what
    /// was kept, as a restart would.
    pub fn sqlite(folder: &Path, extra: Vec<EntityInfo>) -> Result<Self, String> {
        let fixture = Self {
            t1: Organization::sqlite("T1", &folder.join("t1.sqlite"), extra.clone())?,
            t2: Organization::sqlite("T2", &folder.join("t2.sqlite"), extra)?,
        };
        fixture.seed().map_err(|e| e.to_string())?;
        Ok(fixture)
    }

    fn seed(&self) -> Result<(), SpaceError> {
        self.t1.add_unit("U1", "Unit 1 of T1")?;
        self.t1.add_unit("U2", "Unit 2 of T1")?;
        self.t1.add_person(1, "U1", "P1", "reserved note of P1")?;
        self.t1.add_person(2, "U2", "P2", "reserved note of P2")?;
        self.t2.add_unit("U1", "Unit 1 of T2")?;
        self.t2.add_person(1, "U1", "P1 of T2", "reserved note of the P1 of T2")
    }
}

/// Roles of the example policy. They exist for tests; an application writes its own.
pub mod roles {
    use ravenrs_app::{Access, Operation, Role};

    use super::{PERSON, UNIT};

    /// Administers everything of the organization it is used in.
    pub fn administrator() -> Role {
        Role::new("Administrator").administrative()
    }

    /// Reads, creates and changes the people of one unit, never their reserved note, and reads units.
    pub fn registrar(unit: &str) -> Role {
        let mine = format!("[unit] = '{unit}'");
        Role::new(format!("Registrar of {unit}"))
            .type_permission(UNIT, [Operation::Read], Access::Allow)
            .object_permission(
                PERSON,
                &mine,
                [Operation::Read, Operation::Write, Operation::Create],
                Access::Allow,
            )
            .member_permission(PERSON, ["note"], None, [Operation::Read, Operation::Write], Access::Deny)
    }

    /// Reads and writes only these entities, as a worker granted one job would.
    pub fn worker(entities: &[&str]) -> Role {
        entities.iter().fold(Role::new("Worker"), |role, entity| {
            role.type_permission(
                *entity,
                [Operation::Read, Operation::Write, Operation::Create],
                Access::Allow,
            )
        })
    }
}

/// A clock that moves only when told to, in whole seconds since the Unix epoch.
#[derive(Clone, Debug)]
pub struct Clock(Rc<Cell<i64>>);

impl Clock {
    /// A clock stopped at this instant.
    pub fn at(seconds: i64) -> Self {
        Self(Rc::new(Cell::new(seconds)))
    }

    /// What it says now.
    pub fn now(&self) -> i64 {
        self.0.get()
    }

    /// Moves it forward.
    pub fn advance(&self, seconds: i64) {
        self.0.set(self.0.get() + seconds);
    }
}

impl Default for Clock {
    /// 12 November 2026, 00:00 UTC.
    fn default() -> Self {
        Self::at(1_794_441_600)
    }
}
