//! Which modules can go together, and in what order: each comes after what it depends on, and a
//! module that does not fit is left out with the reason, while the others go on.
//!
//! [`ApplicationBuilder::build`](crate::ApplicationBuilder::build) refuses the whole application at
//! the first reason, since an application with a part missing should not open. A host that loads
//! parts while it runs, such as a server with plugins, keeps what fits and says why the rest stayed
//! out. Both ask [`plan_modules`], so a module is accepted or refused by the same rules everywhere.
//!
//! ```
//! use ravenrs_app::{ModuleEntry, ModuleHost, ModuleManifest, VersionRange, plan_modules};
//!
//! let git = ModuleEntry::new("git", ModuleManifest::new().version("1.4.0")?);
//! let github = ModuleEntry::new(
//!     "github",
//!     ModuleManifest::new().requires("git", VersionRange::compatible("2")?),
//! );
//! let cron = ModuleEntry::new("cron", ModuleManifest::new());
//! let plan = plan_modules(&[github, git, cron], &ModuleHost::default());
//! assert_eq!(plan.order, vec![1, 2]);
//! assert_eq!(
//!     plan.refused[0].1.to_string(),
//!     "the module github needs git from 2.0.0 and below 3.0.0, and git is 1.4.0"
//! );
//! # Ok::<(), String>(())
//! ```

use std::fmt;

use crate::manifest::{ModuleManifest, Target, Version, VersionRange};
use crate::module_api::Module;

/// What [`plan_modules`] needs to know about a module.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleEntry {
    /// What it is known by.
    pub name: String,
    /// The modules that have to come before it, by name.
    pub dependencies: Vec<String>,
    /// What it says about itself.
    pub manifest: ModuleManifest,
}

impl ModuleEntry {
    /// A module by this name that says this about itself, with no dependencies besides the
    /// modules its manifest requires.
    pub fn new(name: impl Into<String>, manifest: ModuleManifest) -> Self {
        Self { name: name.into(), dependencies: Vec::new(), manifest }
    }

    /// What `module` says about itself.
    pub fn of(module: &dyn Module) -> Self {
        Self {
            name: module.name().to_owned(),
            dependencies: module.dependencies(),
            manifest: module.manifest(),
        }
    }

    /// The modules it waits for: its dependencies and the modules its manifest requires.
    fn needs(&self) -> Vec<&str> {
        let mut needs: Vec<&str> = self.dependencies.iter().map(String::as_str).collect();
        for (name, _) in &self.manifest.requires {
            if !needs.contains(&name.as_str()) {
                needs.push(name);
            }
        }
        needs
    }
}

/// What the host says about where the modules run. What it leaves empty it does not check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleHost {
    /// Where the application runs.
    pub target: Option<Target>,
    /// The storage it works on, by name such as `sqlite`.
    pub database: Option<String>,
    /// The capabilities the host offers.
    pub capabilities: Vec<String>,
}

/// Why a module was left out.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModuleRefusal {
    /// Another module of the same name came first.
    AddedTwice {
        /// The module.
        module: String,
    },
    /// It depends on a module that is not there.
    Missing {
        /// The module.
        module: String,
        /// What it depends on.
        needs: String,
    },
    /// These modules wait for each other, or for one of them, so none can come first.
    Cycle {
        /// The module.
        module: String,
        /// Every module left waiting, in the order they were given.
        modules: Vec<String>,
    },
    /// It depends on a module that was refused.
    NeedsRefused {
        /// The module.
        module: String,
        /// The refused module it depends on.
        needs: String,
    },
    /// It accepts other versions of a module than the one there is.
    WrongVersion {
        /// The module.
        module: String,
        /// The module it requires.
        needs: String,
        /// The versions it accepts.
        range: VersionRange,
        /// The version there is, when that module says one.
        found: Option<Version>,
    },
    /// It does not run where the application runs.
    Target {
        /// The module.
        module: String,
        /// Where the application runs.
        target: Target,
    },
    /// It does not work on the application's storage.
    Database {
        /// The module.
        module: String,
        /// The storage.
        database: String,
    },
    /// It needs a capability the host does not offer.
    Capability {
        /// The module.
        module: String,
        /// The capability.
        capability: String,
    },
    /// It brings a migration or a job that a module before it brought already.
    Duplicate {
        /// The module.
        module: String,
        /// `migration` or `job`.
        kind: &'static str,
        /// The id both bring.
        id: String,
        /// The module that brought it first.
        first: String,
    },
}

impl ModuleRefusal {
    /// The module refused.
    pub fn module(&self) -> &str {
        match self {
            Self::AddedTwice { module }
            | Self::Missing { module, .. }
            | Self::Cycle { module, .. }
            | Self::NeedsRefused { module, .. }
            | Self::WrongVersion { module, .. }
            | Self::Target { module, .. }
            | Self::Database { module, .. }
            | Self::Capability { module, .. }
            | Self::Duplicate { module, .. } => module,
        }
    }
}

impl fmt::Display for ModuleRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddedTwice { module } => write!(f, "the module {module} was added twice"),
            Self::Missing { module, needs } => {
                write!(f, "the module {module} depends on {needs}, which was not added")
            }
            Self::Cycle { modules, .. } => {
                write!(f, "these modules depend on each other: {}", modules.join(", "))
            }
            Self::NeedsRefused { module, needs } => {
                write!(f, "the module {module} depends on {needs}, which was refused")
            }
            Self::WrongVersion { module, needs, range, found: Some(version) } => {
                write!(f, "the module {module} needs {needs} {range}, and {needs} is {version}")
            }
            Self::WrongVersion { module, needs, range, found: None } => {
                write!(f, "the module {module} needs {needs} {range}, and {needs} says no version")
            }
            Self::Target { module, target } => write!(f, "the module {module} does not run on {target:?}"),
            Self::Database { module, database } => {
                write!(f, "the module {module} does not work on {database}")
            }
            Self::Capability { module, capability } => write!(
                f,
                "the module {module} needs the capability {capability}, which the host does not offer"
            ),
            Self::Duplicate { module, kind, id, first } => {
                write!(f, "the {kind} {id} comes from both {first} and {module}")
            }
        }
    }
}

impl std::error::Error for ModuleRefusal {}

/// The modules that go together and the ones left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModulePlan {
    /// The accepted modules, as indexes into the entries given, each after what it depends on,
    /// keeping the order they were given in as far as possible.
    pub order: Vec<usize>,
    /// The modules left out, as indexes into the entries given, each with its reason, in the
    /// order they were found: first what is wrong with how they depend on each other, then what
    /// the manifests say.
    pub refused: Vec<(usize, ModuleRefusal)>,
}

impl ModulePlan {
    /// Whether every module was accepted.
    pub fn accepted_all(&self) -> bool {
        self.refused.is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Waiting,
    Placed,
    Refused,
}

/// Which of `entries` go together on `host`, in an order where each comes after what it depends
/// on, and why the others are left out. A module that depends on one left out is left out too.
pub fn plan_modules(entries: &[ModuleEntry], host: &ModuleHost) -> ModulePlan {
    let index_of = |name: &str| entries.iter().position(|entry| entry.name == name);
    let mut state = vec![State::Waiting; entries.len()];
    let mut refused: Vec<(usize, ModuleRefusal)> = Vec::new();
    let mut order: Vec<usize> = Vec::with_capacity(entries.len());
    for (i, entry) in entries.iter().enumerate() {
        if index_of(&entry.name) != Some(i) {
            state[i] = State::Refused;
            refused.push((i, ModuleRefusal::AddedTwice { module: entry.name.clone() }));
        }
    }
    // How they depend on each other: each pass places or refuses one module, the first in the
    // order given that can be, so the order given is kept where nothing forces another.
    loop {
        let mut progressed = false;
        for (i, entry) in entries.iter().enumerate() {
            if state[i] != State::Waiting {
                continue;
            }
            let needs = entry.needs();
            if let Some(missing) = needs.iter().find(|name| index_of(name).is_none()) {
                state[i] = State::Refused;
                let refusal =
                    ModuleRefusal::Missing { module: entry.name.clone(), needs: (*missing).to_owned() };
                refused.push((i, refusal));
            } else if let Some(gone) =
                needs.iter().find(|name| index_of(name).is_some_and(|at| state[at] == State::Refused))
            {
                state[i] = State::Refused;
                let refusal =
                    ModuleRefusal::NeedsRefused { module: entry.name.clone(), needs: (*gone).to_owned() };
                refused.push((i, refusal));
            } else if needs.iter().all(|name| index_of(name).is_some_and(|at| state[at] == State::Placed)) {
                state[i] = State::Placed;
                order.push(i);
            } else {
                continue;
            }
            progressed = true;
            break;
        }
        if !progressed {
            break;
        }
    }
    let stuck: Vec<usize> = (0..entries.len()).filter(|&i| state[i] == State::Waiting).collect();
    let names: Vec<String> = stuck.iter().map(|&i| entries[i].name.clone()).collect();
    for &i in &stuck {
        state[i] = State::Refused;
        refused.push((i, ModuleRefusal::Cycle { module: entries[i].name.clone(), modules: names.clone() }));
    }
    // What the manifests say, in the order they go in.
    let mut accepted = Vec::with_capacity(order.len());
    let mut brought: Vec<(&'static str, String, String)> = Vec::new();
    for i in order {
        match manifest_refusal(entries, i, host, &state, &brought) {
            Some(refusal) => {
                state[i] = State::Refused;
                refused.push((i, refusal));
            }
            None => {
                let entry = &entries[i];
                for id in &entry.manifest.migrations {
                    brought.push(("migration", id.clone(), entry.name.clone()));
                }
                for id in &entry.manifest.jobs {
                    brought.push(("job", id.clone(), entry.name.clone()));
                }
                accepted.push(i);
            }
        }
    }
    ModulePlan { order: accepted, refused }
}

/// Why the manifest of `entries[i]`, or a module before it, keeps it out; `None` when it fits.
fn manifest_refusal(
    entries: &[ModuleEntry],
    i: usize,
    host: &ModuleHost,
    state: &[State],
    brought: &[(&'static str, String, String)],
) -> Option<ModuleRefusal> {
    let entry = &entries[i];
    let module = entry.name.clone();
    let found = |name: &str| entries.iter().position(|other| other.name == name);
    if let Some(gone) =
        entry.needs().into_iter().find(|name| found(name).is_some_and(|at| state[at] == State::Refused))
    {
        return Some(ModuleRefusal::NeedsRefused { module, needs: gone.to_owned() });
    }
    let manifest = &entry.manifest;
    for (other, range) in &manifest.requires {
        let version = found(other).and_then(|at| entries[at].manifest.version);
        if !version.is_some_and(|version| range.accepts(version)) {
            return Some(ModuleRefusal::WrongVersion {
                module,
                needs: other.clone(),
                range: *range,
                found: version,
            });
        }
    }
    if let Some(target) = host.target
        && !manifest.targets.is_empty()
        && !manifest.targets.contains(&target)
    {
        return Some(ModuleRefusal::Target { module, target });
    }
    if let Some(database) = &host.database
        && !manifest.databases.is_empty()
        && !manifest.databases.contains(database)
    {
        return Some(ModuleRefusal::Database { module, database: database.clone() });
    }
    if let Some(missing) = manifest.capabilities.iter().find(|c| !host.capabilities.contains(c)) {
        return Some(ModuleRefusal::Capability { module, capability: missing.clone() });
    }
    for (kind, ids) in [("migration", &manifest.migrations), ("job", &manifest.jobs)] {
        for id in ids {
            if let Some((_, _, first)) = brought.iter().find(|(k, other, _)| *k == kind && other == id) {
                return Some(ModuleRefusal::Duplicate { module, kind, id: id.clone(), first: first.clone() });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> ModuleEntry {
        ModuleEntry::new(name, ModuleManifest::new())
    }

    fn after(name: &str, needs: &[&str]) -> ModuleEntry {
        ModuleEntry { dependencies: needs.iter().map(|n| (*n).to_owned()).collect(), ..entry(name) }
    }

    #[test]
    fn keeps_the_order_given_where_nothing_forces_another() {
        let plan = plan_modules(&[after("b", &["a"]), entry("a"), entry("c")], &ModuleHost::default());
        assert_eq!(plan.order, vec![1, 0, 2]);
        assert!(plan.accepted_all());
    }

    #[test]
    fn leaves_out_what_does_not_fit_and_keeps_the_rest() {
        let entries = [entry("a"), after("b", &["x"]), after("c", &["b"]), entry("a"), entry("d")];
        let plan = plan_modules(&entries, &ModuleHost::default());
        assert_eq!(plan.order, vec![0, 4]);
        let reasons: Vec<String> = plan.refused.iter().map(|(_, r)| r.to_string()).collect();
        assert_eq!(
            reasons,
            [
                "the module a was added twice",
                "the module b depends on x, which was not added",
                "the module c depends on b, which was refused",
            ]
        );
    }

    #[test]
    fn modules_that_wait_for_each_other_are_all_left_out() {
        let plan =
            plan_modules(&[after("a", &["b"]), after("b", &["a"]), entry("c")], &ModuleHost::default());
        assert_eq!(plan.order, vec![2]);
        assert_eq!(plan.refused.len(), 2);
        assert_eq!(plan.refused[0].1.to_string(), "these modules depend on each other: a, b");
        assert_eq!(plan.refused[1].1.module(), "b");
    }

    #[test]
    fn a_module_refused_by_its_manifest_takes_those_after_it() {
        let host = ModuleHost { capabilities: vec!["ci.disk".into()], ..ModuleHost::default() };
        let net = ModuleEntry::new("net", ModuleManifest::new().capability("ci.network"));
        let entries = [net, after("uses-net", &["net"]), entry("free")];
        let plan = plan_modules(&entries, &host);
        assert_eq!(plan.order, vec![2]);
        assert_eq!(
            plan.refused[0].1,
            ModuleRefusal::Capability { module: "net".into(), capability: "ci.network".into() }
        );
        assert_eq!(
            plan.refused[1].1,
            ModuleRefusal::NeedsRefused { module: "uses-net".into(), needs: "net".into() }
        );
    }

    #[test]
    fn the_second_to_bring_a_migration_is_left_out() {
        let one = ModuleEntry::new("one", ModuleManifest::new().migration("m1"));
        let two = ModuleEntry::new("two", ModuleManifest::new().migration("m1"));
        let plan = plan_modules(&[one, two], &ModuleHost::default());
        assert_eq!(plan.order, vec![0]);
        assert_eq!(plan.refused[0].1.to_string(), "the migration m1 comes from both one and two");
    }

    #[test]
    fn a_required_module_that_says_no_version_is_not_accepted() {
        let range = VersionRange::compatible("1").expect("a range");
        let entries = [entry("base"), ModuleEntry::new("top", ModuleManifest::new().requires("base", range))];
        let plan = plan_modules(&entries, &ModuleHost::default());
        assert_eq!(
            plan.refused[0].1.to_string(),
            "the module top needs base from 1.0.0 and below 2.0.0, and base says no version"
        );
    }
}
