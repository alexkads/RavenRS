//! The contract every [`ObjectSpace`] adapter keeps, as a battery any adapter runs with one call:
//! SQLite, PostgreSQL, PostgREST and the ones to come behave the same before they are called
//! supported.
//!
//! An adapter says how to make fresh storage for a case, and how to reach the same storage a second
//! time, as another program would. Each case makes its own entities and storage and does not depend
//! on the order of the others. What an adapter does not guarantee is declared in its
//! [`Capabilities`], and the case then checks that it fails the declared way, never that it passed.
//!
//! ```
//! use ravenrs_app::{EntityInfo, MemoryDatabase, ObjectSpace, SpaceMaker};
//! use ravenrs_app_testkit::contract::{self, Adapter, Capabilities, Store};
//! use std::rc::Rc;
//!
//! struct Memory;
//! impl Adapter for Memory {
//!     fn name(&self) -> &str {
//!         "memory"
//!     }
//!     fn capabilities(&self) -> Capabilities {
//!         Capabilities::MEMORY
//!     }
//!     fn open(&self, _case: &str, entities: &[EntityInfo]) -> Result<Store, String> {
//!         let database = MemoryDatabase::new(entities.to_vec()).map_err(|e| e.to_string())?;
//!         let space: SpaceMaker = Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>);
//!         Ok(Store { space: space.clone(), again: space })
//!     }
//! }
//!
//! let report = contract::run(&Memory);
//! assert!(report.passed.len() > 10);
//! ```

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use ravenrs_app::{
    Access, AuditMode, AuditSettings, AuditTrail, AuditedObjectSpace, Criteria, EntityInfo, EnumOption,
    FieldInfo, FieldKind, Identity, ObjectSpace, Operation, PermissionPolicy, Query, Role, RoleMerge, Rule,
    SECURITY_AUDIT_ENTRY, SecuredObjectSpace, SecurityContext, SpaceError, SpaceEvent, SpaceMaker,
    StateMachine, UserDirectory, Value, audit_entity, security_entities,
};
use ravenrs_app::{CONTEXT_SAVE, RuleWords, validate};
use ravenrs_criteria::DateTime;

/// What happens when two programs make a record at once on the same storage, each numbering keys
/// on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRace {
    /// They get different keys: the storage hands them out.
    Distinct,
    /// Both take the same next key, and the second commit fails as a whole, with nothing written:
    /// one program at a time should create records there (#241).
    SecondCommitFails,
}

/// What an adapter guarantees where adapters differ. A case whose guarantee is missing checks that
/// the adapter fails or behaves the declared way, and is reported as a limitation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// Two programs creating records at once.
    pub keys: KeyRace,
    /// A change to a record another program changed since it was read is refused as a
    /// [`SpaceError::Conflict`]; without it, the last commit wins.
    pub lost_updates_detected: bool,
}

impl Capabilities {
    /// `MemoryDatabase`: one program, so its spaces share the numbering.
    pub const MEMORY: Self = Self { keys: KeyRace::Distinct, lost_updates_detected: false };
    /// `ravenrs-app-sqlite`.
    pub const SQLITE: Self = Self { keys: KeyRace::SecondCommitFails, lost_updates_detected: false };
    /// `ravenrs-app-postgres`.
    pub const POSTGRES: Self = Self { keys: KeyRace::SecondCommitFails, lost_updates_detected: false };
    /// `ravenrs-app-postgrest`.
    pub const POSTGREST: Self = Self { keys: KeyRace::SecondCommitFails, lost_updates_detected: false };

    /// Every adapter that comes with RavenRS, by the name `docs/APP-PARITY.md` gives it.
    pub const ALL: [(&'static str, Self); 4] = [
        ("MemoryDatabase", Self::MEMORY),
        ("ravenrs-app-sqlite", Self::SQLITE),
        ("ravenrs-app-postgres", Self::POSTGRES),
        ("ravenrs-app-postgrest", Self::POSTGREST),
    ];

    /// The two cells this adapter's row has in the table of `docs/APP-PARITY.md`.
    pub fn cells(self) -> [&'static str; 2] {
        [
            match self.keys {
                KeyRace::Distinct => "different keys",
                KeyRace::SecondCommitFails => "the second commit fails",
            },
            if self.lost_updates_detected { "conflict" } else { "the last commit wins" },
        ]
    }
}

/// Storage made for one case.
pub struct Store {
    /// Units of work over it.
    pub space: SpaceMaker,
    /// Units of work over the same records as another program would have them: another connection,
    /// with its own numbering of keys. An adapter that only lives in one program gives `space` again.
    pub again: SpaceMaker,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

/// An adapter under the contract.
pub trait Adapter {
    /// What reports call it.
    fn name(&self) -> &str;
    /// What it guarantees where adapters differ.
    fn capabilities(&self) -> Capabilities;
    /// Fresh, empty storage for these entities, apart from every other case's. `case` is short,
    /// lowercase and fit for a table or schema name.
    fn open(&self, case: &str, entities: &[EntityInfo]) -> Result<Store, String>;
}

/// What a run found.
#[derive(Debug, Default)]
pub struct Report {
    /// The cases that passed in full.
    pub passed: Vec<&'static str>,
    /// The cases that met a limitation the adapter declared, and behaved as declared.
    pub limited: Vec<(&'static str, &'static str)>,
}

type Case = fn(&dyn Adapter) -> Result<Outcome, String>;

/// How a case ended when it did not fail.
enum Outcome {
    Passed,
    Limited(&'static str),
}

/// Every case, by name.
const CASES: [(&str, Case); 15] = [
    ("types", types),
    ("filters_and_order", filters_and_order),
    ("pages_only_and_calculated", pages_only_and_calculated),
    ("uncommitted_is_seen_and_rollback", uncommitted_is_seen_and_rollback),
    ("all_or_nothing", all_or_nothing),
    ("gone_record_conflicts", gone_record_conflicts),
    ("state_machine", state_machine),
    ("rules_and_watchers", rules_and_watchers),
    ("audit_in_the_same_commit", audit_in_the_same_commit),
    ("permission_denied", permission_denied),
    ("keys_from_two_programs", keys_from_two_programs),
    ("update_from_two_programs", update_from_two_programs),
    ("delete_what_another_changed", delete_what_another_changed),
    ("delete", delete),
    ("decimals", decimals),
];

/// The names of the cases, in the order they run, for [`run_only`].
pub fn cases() -> Vec<&'static str> {
    CASES.iter().map(|(name, _)| *name).collect()
}

/// Runs every case on the adapter, each on storage of its own, and says what passed and what met a
/// declared limitation. Panics with every case that failed and why, after running them all.
pub fn run(adapter: &dyn Adapter) -> Report {
    run_only(adapter, |_| true)
}

/// As [`run`], with only the cases whose name `which` accepts.
pub fn run_only(adapter: &dyn Adapter, which: impl Fn(&str) -> bool) -> Report {
    let mut report = Report::default();
    let mut failed = Vec::new();
    for (name, case) in CASES {
        if !which(name) {
            continue;
        }
        match catch_unwind(AssertUnwindSafe(|| case(adapter))) {
            Ok(Ok(Outcome::Passed)) => report.passed.push(name),
            Ok(Ok(Outcome::Limited(why))) => report.limited.push((name, why)),
            Ok(Err(why)) => failed.push(format!("{name}: {why}")),
            Err(panic) => {
                let why = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                    .unwrap_or_default();
                failed.push(format!("{name}: {why}"));
            }
        }
    }
    for (name, why) in &report.limited {
        eprintln!("{}: {name} is a declared limitation: {why}", adapter.name());
    }
    assert!(failed.is_empty(), "{} broke the contract:\n{}", adapter.name(), failed.join("\n"));
    report
}

/// Fails the case with this, unless it holds.
macro_rules! ensure {
    ($holds:expr, $($why:tt)+) => {
        if !$holds {
            return Err(format!($($why)+));
        }
    };
}

/// Fails the case unless the two are equal.
macro_rules! same {
    ($left:expr, $right:expr) => {{
        let (left, right) = (&$left, &$right);
        if left != right {
            return Err(format!("{} is {left:?}, not {right:?}", stringify!($left)));
        }
    }};
    ($left:expr, $right:expr, $what:expr) => {{
        let (left, right) = (&$left, &$right);
        if left != right {
            return Err(format!("{}: {left:?}, not {right:?}", $what));
        }
    }};
}

fn wrong(error: impl fmt::Display) -> String {
    error.to_string()
}

fn store(adapter: &dyn Adapter, case: &str, entities: &[EntityInfo]) -> Result<Store, String> {
    adapter.open(case, entities).map_err(|why| format!("the storage could not be made: {why}"))
}

fn add(space: &mut dyn ObjectSpace, entity: &str, values: &[(&str, Value)]) -> Result<Value, String> {
    let mut object = space.new_object(entity).map_err(wrong)?;
    for (field, value) in values {
        object.set(field, value.clone());
    }
    let key = object.get("id");
    space.save(object).map_err(wrong)?;
    Ok(key)
}

fn count(space: &dyn ObjectSpace, entity: &str) -> Result<usize, String> {
    space.query(entity, &Query::all()).map(|found| found.len()).map_err(wrong)
}

fn names(space: &dyn ObjectSpace, entity: &str, query: &Query) -> Result<Vec<String>, String> {
    let found = space.query(entity, query).map_err(wrong)?;
    Ok(found
        .iter()
        .map(|object| match object.get("name") {
            Value::Text(text) => text,
            other => format!("{other:?}"),
        })
        .collect())
}

fn filtered(text: &str) -> Query {
    Query::all().filter(Criteria::parse(text).unwrap_or_else(|e| panic!("{text}: {e}")))
}

/// A plain record, with what filters and pages need.
fn plain() -> EntityInfo {
    EntityInfo::new("Plain")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(FieldInfo::new("seats", FieldKind::Integer))
        .field(FieldInfo::new("rate", FieldKind::Decimal(2)))
        .field(FieldInfo::new("active", FieldKind::Boolean))
        .field(FieldInfo::new("twice", FieldKind::Integer).calculated("[seats] * 2"))
}

fn three_plain(space: &mut dyn ObjectSpace) -> Result<(), String> {
    for (name, seats, rate, active) in
        [("Alfa", 1.0, 0.5, true), ("Beta", 2.0, 2.5, false), ("Gama", 3.0, 3.0, true)]
    {
        add(
            space,
            "Plain",
            &[
                ("name", name.into()),
                ("seats", Value::Number(seats)),
                ("rate", Value::Number(rate)),
                ("active", Value::Bool(active)),
            ],
        )?;
    }
    space.commit().map_err(wrong)
}

fn types(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let entity = EntityInfo::new("Kinds")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("whole", FieldKind::Integer))
        .field(FieldInfo::new("rate", FieldKind::Decimal(2)))
        .field(FieldInfo::new("cost", FieldKind::Money))
        .field(FieldInfo::new("on", FieldKind::Boolean))
        .field(FieldInfo::new("day", FieldKind::Date))
        .field(FieldInfo::new("moment", FieldKind::DateTime))
        .field(FieldInfo::new("text", FieldKind::Text));
    let store = store(adapter, "types", &[entity])?;
    let day = DateTime::parse("2026-11-13").ok_or("a date")?;
    let moment = DateTime::parse("2026-11-13 14:35:20").ok_or("a moment")?;
    let full = [
        ("whole", Value::Number(-42.0)),
        ("rate", Value::Number(16.47)),
        ("cost", Value::Number(1_234_567.0)),
        ("on", Value::Bool(true)),
        ("day", Value::Date(day)),
        ("moment", Value::Date(moment)),
        ("text", "Ação, 日本語, emoji 🦀, aspas \" e ' e vírgula, (parênteses)".into()),
    ];
    let mut space = (store.space)();
    let kept = add(&mut *space, "Kinds", &full)?;
    let empty = add(&mut *space, "Kinds", &[])?;
    let off = add(&mut *space, "Kinds", &[("on", Value::Bool(false)), ("whole", Value::Number(0.0))])?;
    space.commit().map_err(wrong)?;
    let reader = (store.space)();
    let found = reader.find("Kinds", &kept).map_err(wrong)?.ok_or("the record is not found by its key")?;
    for (field, value) in &full {
        same!(found.get(field), *value, field.to_string());
    }
    let nothing = reader.find("Kinds", &empty).map_err(wrong)?.ok_or("the empty record is not found")?;
    for (field, _) in &full {
        same!(nothing.get(field), Value::Null, format!("{field} left empty"));
    }
    let zero = reader.find("Kinds", &off).map_err(wrong)?.ok_or("the third record is not found")?;
    same!(zero.get("on"), Value::Bool(false));
    same!(zero.get("whole"), Value::Number(0.0));
    same!(reader.find("Kinds", &Value::Number(999.0)).map_err(wrong)?, None, "a key nobody has");
    Ok(Outcome::Passed)
}

fn filters_and_order(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "filters", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let space = (store.space)();
    let run = |text: &str| names(&*space, "Plain", &filtered(text));
    same!(run("[seats] > 1")?, ["Beta", "Gama"]);
    same!(run("[rate] > 2")?, ["Beta", "Gama"], "a whole number against a decimal");
    same!(run("[rate] = 2.5")?, ["Beta"]);
    same!(run("[active] = True")?, ["Alfa", "Gama"]);
    same!(run("[name] Like 'B%'")?, ["Beta"]);
    same!(run("[name] In ('Alfa', 'Gama')")?, ["Alfa", "Gama"]);
    same!(run("[seats] Between(2, 3)")?, ["Beta", "Gama"]);
    same!(run("[seats] >= 2 And [active] = False")?, ["Beta"]);
    same!(run("[seats] = 1 Or [seats] = 3")?, ["Alfa", "Gama"]);
    same!(run("Not [active]")?, ["Beta"]);
    same!(run("[name] = 'Ninguém'")?, Vec::<String>::new());
    let unknown = space.query("Plain", &filtered("[nope] = 1"));
    ensure!(
        matches!(unknown, Err(SpaceError::BadQuery(_))),
        "a filter on a field nobody has gave {unknown:?}"
    );
    let unknown = space.query("Plain", &Query::all().sort("nope", true));
    ensure!(matches!(unknown, Err(SpaceError::BadQuery(_))), "a sort on a field nobody has gave {unknown:?}");
    same!(names(&*space, "Plain", &Query::all().sort("seats", false))?, ["Gama", "Beta", "Alfa"]);
    same!(
        names(&*space, "Plain", &Query::all().sort("active", true).sort("name", false))?,
        ["Beta", "Gama", "Alfa"]
    );
    let mut space_w = (store.space)();
    add(
        &mut *space_w,
        "Plain",
        &[("name", "B'; DROP TABLE plain; --".into()), ("seats", Value::Number(9.0))],
    )?;
    space_w.commit().map_err(wrong)?;
    same!(run("[name] = 'B''; DROP TABLE plain; --'")?, ["B'; DROP TABLE plain; --"], "data stays data");
    same!(count(&*space, "Plain")?, 4);
    Ok(Outcome::Passed)
}

fn pages_only_and_calculated(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "pages", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let space = (store.space)();
    let page = Query { limit: Some(1), offset: 1, ..Query::all() };
    same!(names(&*space, "Plain", &page)?, ["Beta"]);
    same!(names(&*space, "Plain", &Query { offset: 2, ..Query::all() })?, ["Gama"]);
    same!(names(&*space, "Plain", &Query { limit: Some(2), ..Query::all() })?, ["Alfa", "Beta"]);
    same!(names(&*space, "Plain", &Query { offset: 5, ..Query::all() })?, Vec::<String>::new());
    let only = Query { only: Some(vec!["name".into()]), ..Query::all() };
    let few = space.query("Plain", &only).map_err(wrong)?;
    same!(few.len(), 3);
    same!(few[0].get("name"), Value::Text("Alfa".into()));
    ensure!(!matches!(few[0].get("id"), Value::Null), "the key comes with only: {:?}", few[0]);
    same!(few[0].get("seats"), Value::Null, "a field left out by only");
    let all = space.query("Plain", &Query::all()).map_err(wrong)?;
    same!(all[2].get("twice"), Value::Number(6.0), "calculated");
    same!(
        names(&*space, "Plain", &filtered("[twice] > 2"))?,
        ["Beta", "Gama"],
        "filtered on a calculated field"
    );
    Ok(Outcome::Passed)
}

fn uncommitted_is_seen_and_rollback(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "pending", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let mut space = (store.space)();
    add(&mut *space, "Plain", &[("name", "Delta".into()), ("seats", Value::Number(4.0))])?;
    same!(names(&*space, "Plain", &filtered("[seats] > 2"))?, ["Gama", "Delta"], "a space sees what waits");
    same!(count(&*(store.space)(), "Plain")?, 3, "another space does not");
    let alfa = space.query("Plain", &filtered("[name] = 'Alfa'")).map_err(wrong)?;
    let key = alfa.first().ok_or("Alfa")?.get("id");
    space.delete("Plain", &key).map_err(wrong)?;
    let mut beta = space.query("Plain", &filtered("[name] = 'Beta'")).map_err(wrong)?.remove(0);
    beta.set("name", "Beta 2");
    space.save(beta).map_err(wrong)?;
    same!(
        names(&*space, "Plain", &Query::all())?,
        ["Beta 2", "Gama", "Delta"],
        "new, changed and deleted, waiting"
    );
    ensure!(space.is_modified(), "a space with changes says so");
    space.rollback();
    ensure!(!space.is_modified(), "after a rollback nothing waits");
    same!(names(&*space, "Plain", &Query::all())?, ["Alfa", "Beta", "Gama"], "all thrown away");
    same!(count(&*(store.space)(), "Plain")?, 3, "nothing reached the storage");
    Ok(Outcome::Passed)
}

/// A commit with a new record and an update of a record another program deletes after the update
/// was saved and before the commit, so the update finds nothing to change.
fn half_good(store: &Store, space: &mut dyn ObjectSpace) -> Result<(), String> {
    let gone = space.query("Plain", &filtered("[name] = 'Alfa'")).map_err(wrong)?;
    let mut gone = gone.into_iter().next().ok_or("Alfa")?;
    let key = gone.get("id");
    add(space, "Plain", &[("name", "Nova".into())])?;
    gone.set("seats", Value::Number(10.0));
    space.save(gone).map_err(wrong)?;
    let mut other = (store.again)();
    other.delete("Plain", &key).map_err(wrong)?;
    other.commit().map_err(wrong)
}

fn all_or_nothing(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "allornothing", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let mut space = (store.space)();
    half_good(&store, &mut *space)?;
    let failed = space.commit();
    ensure!(failed.is_err(), "a commit whose second change fails went through");
    same!(
        names(&*(store.space)(), "Plain", &Query::all())?,
        ["Beta", "Gama"],
        "nothing of the commit stayed"
    );
    space.rollback();
    add(&mut *space, "Plain", &[("name", "Depois".into())])?;
    space.commit().map_err(|e| format!("the storage does not work after a failed commit: {e}"))?;
    same!(count(&*(store.space)(), "Plain")?, 3);
    Ok(Outcome::Passed)
}

fn gone_record_conflicts(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "gone", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let mut space = (store.space)();
    let mut beta = space.query("Plain", &filtered("[name] = 'Beta'")).map_err(wrong)?.remove(0);
    let key = beta.get("id");
    beta.set("seats", Value::Number(9.0));
    space.save(beta).map_err(wrong)?;
    // Deleted after the update was saved: `save` decides between a new record and an update when
    // it is called, so a record already gone by then would be made again.
    let mut other = (store.again)();
    other.delete("Plain", &key).map_err(wrong)?;
    other.commit().map_err(wrong)?;
    let error = space.commit();
    ensure!(matches!(error, Err(SpaceError::Conflict(_))), "an update of a deleted record gave {error:?}");
    same!(
        names(&*(store.space)(), "Plain", &Query::all())?,
        ["Alfa", "Gama"],
        "the deleted record stays deleted"
    );
    Ok(Outcome::Passed)
}

fn order() -> EntityInfo {
    EntityInfo::new("Order")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(FieldInfo::new(
            "state",
            FieldKind::Enum(vec![EnumOption::new("o", "Open"), EnumOption::new("c", "Closed")]),
        ))
        .rule(Rule::required("name"))
        .state_machine(
            StateMachine::new("state")
                .state("o", "Open")
                .state("c", "Closed")
                .start("o")
                .transition("o", "c", "Close"),
        )
}

fn state_machine(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "states", &[order()])?;
    let mut space = (store.space)();
    let key = add(&mut *space, "Order", &[("name", "Um".into()), ("state", "o".into())])?;
    space.commit().map_err(wrong)?;
    let mut space = (store.space)();
    let mut found = space.find("Order", &key).map_err(wrong)?.ok_or("the order")?;
    found.set("state", "c");
    space.save(found).map_err(wrong)?;
    space.commit().map_err(|e| format!("a transition the machine has was refused: {e}"))?;
    let mut space = (store.space)();
    let mut found = space.find("Order", &key).map_err(wrong)?.ok_or("the order")?;
    found.set("state", "o");
    let refused = space.save(found).and_then(|()| space.commit());
    ensure!(refused.is_err(), "a transition the machine does not have went through");
    let kept = (store.space)().find("Order", &key).map_err(wrong)?.ok_or("the order")?;
    same!(kept.get("state"), Value::Text("c".into()));
    Ok(Outcome::Passed)
}

/// Rules do not run by themselves: forms, the web API and actions call `validate` before keeping,
/// and a watcher on the commit can refuse it. What the adapter owes them is reading other records
/// for a rule such as `unique`, and keeping nothing when a watcher refuses.
fn rules_and_watchers(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let entity = order().rule(Rule::unique("name"));
    let store = store(adapter, "rules", std::slice::from_ref(&entity))?;
    let mut space = (store.space)();
    add(&mut *space, "Order", &[("name", "Um".into()), ("state", "o".into())])?;
    space.commit().map_err(wrong)?;
    let words = RuleWords::default();
    let mut space = (store.space)();
    let mut empty = space.new_object("Order").map_err(wrong)?;
    empty.set("state", "o");
    same!(validate(&entity, &empty, CONTEXT_SAVE, &*space, &words).len(), 1, "a missing name");
    let mut twin = space.new_object("Order").map_err(wrong)?;
    twin.set("name", "Um");
    twin.set("state", "o");
    same!(validate(&entity, &twin, CONTEXT_SAVE, &*space, &words).len(), 1, "a name another record has");
    twin.set("name", "Dois");
    same!(validate(&entity, &twin, CONTEXT_SAVE, &*space, &words).len(), 0, "a good record");
    space.on_event(Box::new(|event| match event {
        SpaceEvent::Committing(_) => Err("não hoje".to_owned()),
        _ => Ok(()),
    }));
    space.save(twin).map_err(wrong)?;
    same!(space.commit(), Err(SpaceError::Refused("não hoje".into())), "a watcher refusing");
    same!(count(&*(store.space)(), "Order")?, 1, "nothing of the refused commit stayed");
    Ok(Outcome::Passed)
}

fn with_security(entity: EntityInfo) -> Vec<EntityInfo> {
    let mut all = vec![entity];
    all.extend(security_entities());
    all
}

fn audit_in_the_same_commit(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "audit", &[plain(), audit_entity()])?;
    three_plain(&mut *(store.space)())?;
    let trail = Rc::new(AuditTrail::new(AuditSettings::new(AuditMode::Full), || "contrato".to_owned()));
    let mut audited = AuditedObjectSpace::new((store.space)(), trail.clone());
    add(&mut audited, "Plain", &[("name", "Delta".into())])?;
    audited.commit().map_err(wrong)?;
    let written = count(&*(store.space)(), SECURITY_AUDIT_ENTRY)?;
    ensure!(written > 0, "a commit left no trail");

    let mut audited = AuditedObjectSpace::new((store.space)(), trail);
    half_good(&store, &mut audited)?;
    ensure!(audited.commit().is_err(), "a commit whose second change fails went through");
    same!(count(&*(store.space)(), SECURITY_AUDIT_ENTRY)?, written, "the trail of a failed commit stayed");
    same!(count(&*(store.space)(), "Plain")?, 3, "the records of a failed commit stayed");
    Ok(Outcome::Passed)
}

fn permission_denied(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let entities = with_security(plain());
    let store = store(adapter, "denied", &entities)?;
    three_plain(&mut *(store.space)())?;
    let reader = Role::new("Leitura").policy(PermissionPolicy::ReadOnlyAll);
    let hidden = Role::new("Sem custo").policy(PermissionPolicy::AllowAll).type_permission(
        "Plain",
        [Operation::Delete],
        Access::Deny,
    );
    let directory = UserDirectory::new(store.space.clone(), Vec::new()).map_err(wrong)?;
    let as_role = |role: Role| {
        let identity = Identity::new("u", "u", vec![role]);
        let context = SecurityContext::new(identity, directory.clone(), entities.clone(), RoleMerge::AnyRole);
        SecuredObjectSpace::new((store.space)(), Rc::new(context))
    };
    let mut space = as_role(reader);
    same!(count(&space, "Plain")?, 3, "a reader reads");
    let tried = space.new_object("Plain").and_then(|object| space.save(object)).and_then(|()| space.commit());
    ensure!(matches!(tried, Err(SpaceError::Denied(_))), "a reader writing gave {tried:?}");
    let mut space = as_role(hidden);
    let first = space.query("Plain", &Query::all()).map_err(wrong)?.remove(0).get("id");
    let tried = space.delete("Plain", &first).and_then(|()| space.commit());
    ensure!(matches!(tried, Err(SpaceError::Denied(_))), "a delete the role denies gave {tried:?}");
    same!(count(&*(store.space)(), "Plain")?, 3, "nothing was written");
    Ok(Outcome::Passed)
}

fn keys_from_two_programs(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "keys", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    // Two units of work of one program never get the same key.
    let (mut a, mut b) = ((store.space)(), (store.space)());
    let (x, y) =
        (add(&mut *a, "Plain", &[("name", "X".into())])?, add(&mut *b, "Plain", &[("name", "Y".into())])?);
    ensure!(x != y, "two spaces of one program got the key {x:?}");
    a.commit().map_err(wrong)?;
    b.commit().map_err(wrong)?;
    let mut tidy = (store.space)();
    tidy.delete("Plain", &x).map_err(wrong)?;
    tidy.delete("Plain", &y).map_err(wrong)?;
    tidy.commit().map_err(wrong)?;
    let mut one = (store.space)();
    let mut two = (store.again)();
    // Each asks for the next key before the other commits.
    let first = add(&mut *one, "Plain", &[("name", "Um".into())])?;
    let second = add(&mut *two, "Plain", &[("name", "Dois".into())])?;
    let also = add(&mut *two, "Plain", &[("name", "Tres".into())])?;
    one.commit().map_err(wrong)?;
    let committed = two.commit();
    let kept = names(&*(store.space)(), "Plain", &Query::all())?;
    match adapter.capabilities().keys {
        KeyRace::Distinct => {
            ensure!(first != second && first != also, "two programs got the same key {first:?}");
            committed.map_err(wrong)?;
            same!(kept.len(), 6);
            Ok(Outcome::Passed)
        }
        KeyRace::SecondCommitFails => {
            if first != second {
                committed.map_err(wrong)?;
                same!(kept.len(), 6);
                return Ok(Outcome::Passed);
            }
            ensure!(committed.is_err(), "two records with the key {first:?}: the second commit went through");
            same!(
                kept,
                ["Alfa", "Beta", "Gama", "Um"],
                "nothing of the second commit stayed, and the first is whole"
            );
            Ok(Outcome::Limited(
                "two programs creating records at once: the second commit fails, whole (#241)",
            ))
        }
    }
}

fn update_from_two_programs(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "updates", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let mut one = (store.space)();
    let mut two = (store.again)();
    let mut mine = one.query("Plain", &filtered("[name] = 'Beta'")).map_err(wrong)?.remove(0);
    let mut theirs = two.query("Plain", &filtered("[name] = 'Beta'")).map_err(wrong)?.remove(0);
    let key = mine.get("id");
    theirs.set("seats", Value::Number(20.0));
    two.save(theirs).map_err(wrong)?;
    two.commit().map_err(wrong)?;
    mine.set("seats", Value::Number(10.0));
    one.save(mine).map_err(wrong)?;
    let committed = one.commit();
    let kept = (store.space)().find("Plain", &key).map_err(wrong)?.ok_or("Beta")?.get("seats");
    if adapter.capabilities().lost_updates_detected {
        ensure!(matches!(committed, Err(SpaceError::Conflict(_))), "a lost update gave {committed:?}");
        same!(kept, Value::Number(20.0));
        return Ok(Outcome::Passed);
    }
    committed.map_err(wrong)?;
    same!(kept, Value::Number(10.0), "the last commit wins");
    Ok(Outcome::Limited("an update over another program's: the last commit wins (#241)"))
}

fn delete_what_another_changed(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "deletechanged", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let mut one = (store.space)();
    let mut two = (store.again)();
    let key = one.query("Plain", &filtered("[name] = 'Beta'")).map_err(wrong)?.remove(0).get("id");
    let mut theirs = two.find("Plain", &key).map_err(wrong)?.ok_or("Beta")?;
    theirs.set("seats", Value::Number(20.0));
    two.save(theirs).map_err(wrong)?;
    two.commit().map_err(wrong)?;
    one.delete("Plain", &key).map_err(wrong)?;
    let committed = one.commit();
    let kept = (store.space)().find("Plain", &key).map_err(wrong)?;
    if adapter.capabilities().lost_updates_detected {
        ensure!(
            matches!(committed, Err(SpaceError::Conflict(_))),
            "deleting a changed record gave {committed:?}"
        );
        ensure!(kept.is_some(), "the changed record is gone");
        return Ok(Outcome::Passed);
    }
    committed.map_err(wrong)?;
    ensure!(kept.is_none(), "the record is still there");
    Ok(Outcome::Limited("deleting what another program changed since: it is deleted (#241)"))
}

fn delete(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let store = store(adapter, "deletes", &[plain()])?;
    three_plain(&mut *(store.space)())?;
    let mut space = (store.space)();
    let found = space.query("Plain", &Query::all()).map_err(wrong)?;
    for object in found.iter().take(2) {
        space.delete("Plain", &object.get("id")).map_err(wrong)?;
    }
    same!(names(&*space, "Plain", &Query::all())?, ["Gama"], "a space sees its own deletes");
    space.commit().map_err(wrong)?;
    same!(names(&*(store.space)(), "Plain", &Query::all())?, ["Gama"]);
    let mut space = (store.space)();
    let next = add(&mut *space, "Plain", &[("name", "Nova".into())])?;
    ensure!(next != found[0].get("id") && next != found[1].get("id"), "a deleted key was given again");
    Ok(Outcome::Passed)
}

fn decimals(adapter: &dyn Adapter) -> Result<Outcome, String> {
    let entity = EntityInfo::new("Rate")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("two", FieldKind::Decimal(2)))
        .field(FieldInfo::new("six", FieldKind::Decimal(6)));
    let store = store(adapter, "decimals", &[entity])?;
    let exact =
        [(0.1, "two"), (5.49, "two"), (16.47, "two"), (123_456_789.123_456, "six"), (0.000_001, "six")];
    let mut space = (store.space)();
    for (number, field) in exact {
        add(&mut *space, "Rate", &[(field, Value::Number(number))])?;
    }
    add(&mut *space, "Rate", &[("two", Value::Number(0.1 + 0.2))])?;
    space.commit().map_err(wrong)?;
    let reader = (store.space)();
    let found = reader.query("Rate", &Query::all()).map_err(wrong)?;
    for (at, (number, field)) in exact.iter().enumerate() {
        same!(found[at].get(field), Value::Number(*number), format!("{number}"));
    }
    same!(found[5].get("two"), Value::Number(0.3), "rounded to its places");
    let hits = |text: &str| reader.query("Rate", &filtered(text)).map(|found| found.len()).map_err(wrong);
    same!(hits("[two] = 16.47")?, 1);
    same!(hits("[two] > 5.49")?, 1);
    same!(hits("[six] = 0.000001")?, 1);
    let mut space = (store.space)();
    add(&mut *space, "Rate", &[("six", Value::Number(1_234_567_890.123_456))])?;
    let refused = space.commit();
    ensure!(
        matches!(refused, Err(SpaceError::Refused(_))),
        "more digits than a number keeps gave {refused:?}"
    );
    Ok(Outcome::Passed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table of `docs/APP-PARITY.md` says what [`Capabilities::ALL`] says.
    #[test]
    fn the_parity_table_says_what_the_adapters_declare() {
        let parity = include_str!("../../../docs/APP-PARITY.md");
        for (adapter, capabilities) in Capabilities::ALL {
            let row = parity
                .lines()
                .find(|line| line.starts_with(&format!("| `{adapter}` |")))
                .unwrap_or_else(|| panic!("docs/APP-PARITY.md has no contract row for {adapter}"));
            let cells: Vec<&str> = row.split('|').map(str::trim).filter(|cell| !cell.is_empty()).collect();
            let [keys, updates] = capabilities.cells();
            assert_eq!(cells.get(1), Some(&keys), "{adapter}: two programs creating records");
            assert_eq!(cells.get(2), Some(&updates), "{adapter}: an update over another's");
        }
    }
}
