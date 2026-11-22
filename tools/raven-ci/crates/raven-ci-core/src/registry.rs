//! The register of plugins: which there are, which go together, which are on, and why the others
//! are not.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use raven_ci_plugin_api::{CiError, CiPlugin, Permission, Point, check_settings, points_of};
use ravenrs_app::{Module, ModuleEntry, ModuleHost, ModuleRefusal, Target, plan_modules};
use ravenrs_flows::Catalog;
use serde::Serialize;
use serde_json::Value;

use crate::external::{ExternalPlugin, MANIFEST_FILE};
use crate::host::{Attachments, Lent, PluginHost, Secrets, ServerLog};
use crate::store::{PluginRow, Store};

/// Where a plugin is now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum PluginState {
    /// Switched on and started: its points are in use.
    On,
    /// Switched off by the administrator.
    Off,
    /// Switched on, and left out, for this reason.
    Refused(String),
}

impl PluginState {
    /// In words for people.
    pub fn words(&self) -> String {
        match self {
            Self::On => "ligado".to_owned(),
            Self::Off => "desligado".to_owned(),
            Self::Refused(why) => format!("recusado: {why}"),
        }
    }
}

/// Where a plugin came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Compiled with the server.
    Embedded,
    /// A program apart, in this folder.
    External(PathBuf),
}

/// How much memory a plugin holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "bytes", rename_all = "snake_case")]
pub enum Memory {
    /// Compiled in: it shares the server's process, and its part is not measured apart.
    Shared,
    /// An external plugin whose program is not running now.
    NotRunning,
    /// An external plugin's program, in bytes.
    Bytes(u64),
    /// An external plugin's program, running, on a system that does not say.
    Unknown,
}

/// A version a plugin accepts of another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Requirement {
    /// The other plugin.
    pub plugin: String,
    /// The versions it accepts, in words.
    pub accepts: String,
}

/// What the list and the detail of the plugins show.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PluginView {
    /// Its name.
    pub name: String,
    /// Its version, when it says one.
    pub version: Option<String>,
    /// What it is for.
    pub summary: String,
    /// Where it came from.
    pub origin: Origin,
    /// Switched on by the administrator.
    pub enabled: bool,
    /// Where it is.
    pub state: PluginState,
    /// The plugins that come before it.
    pub dependencies: Vec<String>,
    /// The versions it accepts of other plugins.
    pub requires: Vec<Requirement>,
    /// The permissions it declared.
    pub declared: Vec<Permission>,
    /// The ones it has: declared and not taken away.
    pub granted: Vec<Permission>,
    /// The points it offers: those in use, while it is on; those its manifest says, for an
    /// external plugin that is not.
    pub points: Vec<Point>,
    /// Its configuration.
    pub settings: Value,
    /// The memory it holds.
    pub memory: Memory,
}

/// A plugin, its external side when it has one, and where it is, taken out of the lock.
type Snapshot = (Arc<dyn CiPlugin>, Option<Arc<ExternalPlugin>>, PluginState);

struct Entry {
    plugin: Arc<dyn CiPlugin>,
    external: Option<Arc<ExternalPlugin>>,
    state: PluginState,
}

impl Entry {
    fn name(&self) -> &str {
        self.plugin.name()
    }
}

#[derive(Default)]
struct Inner {
    entries: Vec<Entry>,
    /// The plugins on, in the order they started.
    started: Vec<String>,
    /// What discovering external plugins found wrong.
    problems: Vec<String>,
}

/// The register of plugins: it holds every plugin the server knows, switched on or not, plans
/// them with [`plan_modules`] and keeps the ones switched on started, in order.
pub struct Registry {
    store: Arc<Store>,
    plugins_dir: PathBuf,
    secrets: Arc<dyn Secrets>,
    log: Arc<ServerLog>,
    /// What the hosts read the attachments of builds from, once the core lends itself.
    attachments: Lent,
    inner: Mutex<Inner>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<String> = self.lock().entries.iter().map(|e| e.name().to_owned()).collect();
        f.debug_struct("Registry").field("plugins", &names).finish_non_exhaustive()
    }
}

impl Registry {
    /// A register with no plugin, keeping its state in `store`, giving each plugin a folder of
    /// its own under `plugins_dir`.
    pub fn new(
        store: Arc<Store>,
        plugins_dir: PathBuf,
        secrets: Arc<dyn Secrets>,
        log: Arc<ServerLog>,
    ) -> Self {
        Self { store, plugins_dir, secrets, log, attachments: Lent::default(), inner: Mutex::default() }
    }

    /// Lets the hosts of plugins read the attachments of builds from `from`: the core, once it
    /// is shared ([`Ci::share`](crate::Ci::share)). The first one stays.
    pub(crate) fn lend_attachments(&self, from: Weak<dyn Attachments>) {
        let _ = self.attachments.set(from);
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Adds a plugin compiled in. It starts at the next [`apply`](Self::apply), if it is switched
    /// on and fits.
    pub fn add(&self, plugin: Arc<dyn CiPlugin>) {
        self.lock().entries.push(Entry { plugin, external: None, state: PluginState::Off });
    }

    /// Adds the external plugins in the folders of `folder` that have a `plugin.toml`; what is
    /// wrong with one is kept in [`problems`](Self::problems) and the others go on.
    pub fn discover(&self, folder: &Path) {
        let Ok(dirs) = std::fs::read_dir(folder) else { return };
        let mut dirs: Vec<PathBuf> =
            dirs.flatten().map(|entry| entry.path()).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for dir in dirs {
            if !dir.join(MANIFEST_FILE).is_file() {
                continue;
            }
            match ExternalPlugin::open(&dir) {
                Ok(plugin) => {
                    let plugin = Arc::new(plugin);
                    let mut inner = self.lock();
                    let known = inner.entries.iter().any(|entry| {
                        entry.external.as_ref().is_some_and(|other| other.folder() == plugin.folder())
                    });
                    if !known {
                        inner.entries.push(Entry {
                            plugin: plugin.clone(),
                            external: Some(plugin),
                            state: PluginState::Off,
                        });
                    }
                }
                Err(problem) => self.lock().problems.push(problem),
            }
        }
    }

    /// What discovering external plugins found wrong.
    pub fn problems(&self) -> Vec<String> {
        self.lock().problems.clone()
    }

    /// Puts the plugins in the state the administrator set: plans the ones switched on, stops
    /// what has to stop (those that require it first) and starts what has to start, in order.
    /// What changed, in words.
    pub fn apply(&self) -> Result<Vec<String>, CiError> {
        let mut inner = self.lock();
        let rows: Vec<PluginRow> =
            inner.entries.iter().map(|entry| self.store.plugin(entry.name())).collect::<Result<_, _>>()?;
        let enabled: Vec<usize> = (0..inner.entries.len()).filter(|&i| rows[i].enabled).collect();
        let entries: Vec<ModuleEntry> = enabled
            .iter()
            .map(|&i| ModuleEntry::of(inner.entries[i].plugin.as_ref() as &dyn Module))
            .collect();
        let host = ModuleHost {
            target: Some(Target::Server),
            database: None,
            capabilities: Permission::ALL.iter().map(|p| p.capability().to_owned()).collect(),
        };
        let plan = plan_modules(&entries, &host);
        let switched_off: Vec<String> = (0..inner.entries.len())
            .filter(|&i| !rows[i].enabled)
            .map(|i| inner.entries[i].name().to_owned())
            .collect();
        let mut refused: BTreeMap<usize, String> = BTreeMap::new();
        for (at, refusal) in &plan.refused {
            refused.insert(enabled[*at], in_words(refusal, &switched_off));
        }
        let wanted: Vec<usize> = plan.order.iter().map(|&at| enabled[at]).collect();
        let mut changes = Vec::new();

        // Stop what is on and no longer wanted, the last started first.
        let wanted_names: Vec<String> = wanted.iter().map(|&i| inner.entries[i].name().to_owned()).collect();
        let stopping: Vec<String> =
            inner.started.iter().rev().filter(|name| !wanted_names.contains(name)).cloned().collect();
        for name in stopping {
            if let Some(entry) = inner.entries.iter_mut().find(|entry| entry.name() == name) {
                entry.plugin.stop();
                entry.state = PluginState::Off;
            }
            inner.started.retain(|started| *started != name);
            changes.push(format!("{name} desligado"));
        }

        // Start what is wanted and not on, in the order of the plan.
        for i in wanted {
            if inner.entries[i].state == PluginState::On {
                continue;
            }
            let needs: Vec<String> = {
                let module = inner.entries[i].plugin.as_ref() as &dyn Module;
                let mut needs = module.dependencies();
                needs.extend(module.manifest().requires.into_iter().map(|(name, _)| name));
                needs
            };
            let not_on = needs.iter().find(|need| !inner.started.contains(need)).cloned();
            let result = match not_on {
                Some(need) => Err(format!("precisa de {need}, que não ligou")),
                None => self.start(&inner.entries[i], &rows[i]),
            };
            let name = inner.entries[i].name().to_owned();
            match result {
                Ok(()) => {
                    inner.entries[i].state = PluginState::On;
                    inner.started.push(name.clone());
                    changes.push(format!("{name} ligado"));
                }
                Err(why) => {
                    refused.insert(i, why);
                }
            }
        }

        for (i, entry) in inner.entries.iter_mut().enumerate() {
            if !rows[i].enabled {
                entry.state = PluginState::Off;
            } else if let Some(why) = refused.get(&i) {
                if entry.state != PluginState::Refused(why.clone()) {
                    changes.push(format!("{} recusado: {why}", entry.name()));
                }
                entry.state = PluginState::Refused(why.clone());
            }
        }
        for change in &changes {
            self.log.write(change);
        }
        Ok(changes)
    }

    /// Starts one plugin: checks its configuration, gives it its host, applies its migrations.
    fn start(&self, entry: &Entry, row: &PluginRow) -> Result<(), String> {
        let plugin = &entry.plugin;
        let name = plugin.name().to_owned();
        if let Some(entity) = plugin.config()
            && let Err(problems) = check_settings(&entity, &row.settings)
        {
            let said: Vec<String> = problems.iter().map(ToString::to_string).collect();
            return Err(said.join("; "));
        }
        let declared = Permission::declared(&plugin.manifest());
        let granted: Vec<Permission> = declared.into_iter().filter(|p| !row.revoked.contains(p)).collect();
        let host = Arc::new(PluginHost::new(
            &name,
            granted,
            row.settings.clone(),
            self.plugins_dir.join(&name),
            self.store.clone(),
            self.secrets.clone(),
            self.log.clone(),
            self.attachments.clone(),
        ));
        host.migrate(&plugin.migrations()).map_err(|e| e.to_string())?;
        plugin.start(host).map_err(|e| format!("não ligou: {e}"))
    }

    fn set(&self, name: &str, change: impl FnOnce(&mut PluginRow)) -> Result<(), CiError> {
        if !self.lock().entries.iter().any(|entry| entry.name() == name) {
            return Err(CiError::unavailable(format!("não há plugin com o nome {name}")));
        }
        let mut row = self.store.plugin(name)?;
        change(&mut row);
        self.store.set_plugin(name, &row)
    }

    /// Switches `name` on, and starts it if it fits; what changed.
    pub fn enable(&self, name: &str) -> Result<Vec<String>, CiError> {
        self.set(name, |row| row.enabled = true)?;
        self.apply()
    }

    /// Switches `name` off, with the plugins that require it; what changed.
    pub fn disable(&self, name: &str) -> Result<Vec<String>, CiError> {
        self.set(name, |row| row.enabled = false)?;
        self.apply()
    }

    /// Sets the configuration of `name`, checked against what it describes, and starts it again
    /// with it, with the plugins that require it.
    pub fn configure(&self, name: &str, settings: Value) -> Result<Vec<String>, CiError> {
        let entity = self.with(name, |plugin| plugin.config())?;
        if let Some(entity) = entity
            && let Err(problems) = check_settings(&entity, &settings)
        {
            return Err(problems
                .into_iter()
                .next()
                .unwrap_or_else(|| CiError::failed("configuração errada")));
        }
        self.set(name, |row| row.settings = settings)?;
        self.restart(name)
    }

    /// Takes `permission` away from `name`, or gives it back, and starts it again.
    pub fn revoke(&self, name: &str, permission: Permission, revoked: bool) -> Result<Vec<String>, CiError> {
        self.set(name, |row| {
            row.revoked.retain(|p| *p != permission);
            if revoked {
                row.revoked.push(permission);
            }
        })?;
        self.restart(name)
    }

    /// Stops `name` and what requires it, then puts everything back as set.
    fn restart(&self, name: &str) -> Result<Vec<String>, CiError> {
        {
            let mut inner = self.lock();
            let mut going = vec![name.to_owned()];
            // Everything started after it that needs something going, goes too.
            let started = inner.started.clone();
            for other in started.iter().skip_while(|n| *n != name).skip(1) {
                let needs = inner
                    .entries
                    .iter()
                    .find(|e| e.name() == other)
                    .map(|e| {
                        let module = e.plugin.as_ref() as &dyn Module;
                        let mut needs = module.dependencies();
                        needs.extend(module.manifest().requires.into_iter().map(|(n, _)| n));
                        needs
                    })
                    .unwrap_or_default();
                if needs.iter().any(|need| going.contains(need)) {
                    going.push(other.clone());
                }
            }
            for gone in going.iter().rev() {
                if let Some(entry) = inner.entries.iter_mut().find(|e| e.name() == gone)
                    && entry.state == PluginState::On
                {
                    entry.plugin.stop();
                    entry.state = PluginState::Off;
                }
                inner.started.retain(|n| n != gone);
            }
        }
        self.apply()
    }

    fn with<R>(&self, name: &str, f: impl FnOnce(&dyn CiPlugin) -> R) -> Result<R, CiError> {
        let plugin = self
            .lock()
            .entries
            .iter()
            .find(|entry| entry.name() == name)
            .map(|entry| entry.plugin.clone())
            .ok_or_else(|| CiError::unavailable(format!("não há plugin com o nome {name}")))?;
        Ok(f(plugin.as_ref()))
    }

    /// The plugins on, in the order they started.
    pub fn on(&self) -> Vec<Arc<dyn CiPlugin>> {
        let inner = self.lock();
        inner
            .started
            .iter()
            .filter_map(|name| inner.entries.iter().find(|e| e.name() == name).map(|e| e.plugin.clone()))
            .collect()
    }

    /// The kinds of step there are: the built-in nodes of `ravenrs-flows` and those of the
    /// plugins on.
    pub fn catalog(&self) -> Catalog {
        let mut catalog = Catalog::builtin();
        for plugin in self.on() {
            plugin.steps(&mut catalog);
        }
        catalog
    }

    /// Which plugin on brings each kind of step.
    pub fn step_owners(&self) -> BTreeMap<String, String> {
        let mut owners = BTreeMap::new();
        for plugin in self.on() {
            let mut catalog = Catalog::new();
            plugin.steps(&mut catalog);
            for spec in catalog.specs() {
                owners.insert(spec.kind, plugin.name().to_owned());
            }
        }
        owners
    }

    /// Every plugin, in the order they were added.
    pub fn list(&self) -> Result<Vec<PluginView>, CiError> {
        let snapshot: Vec<Snapshot> = self
            .lock()
            .entries
            .iter()
            .map(|entry| (entry.plugin.clone(), entry.external.clone(), entry.state.clone()))
            .collect();
        snapshot
            .into_iter()
            .map(|(plugin, external, state)| self.view(plugin.as_ref(), external, state))
            .collect()
    }

    /// The plugin `name`.
    pub fn info(&self, name: &str) -> Result<PluginView, CiError> {
        self.list()?
            .into_iter()
            .find(|view| view.name == name)
            .ok_or_else(|| CiError::unavailable(format!("não há plugin com o nome {name}")))
    }

    fn view(
        &self,
        plugin: &dyn CiPlugin,
        external: Option<Arc<ExternalPlugin>>,
        state: PluginState,
    ) -> Result<PluginView, CiError> {
        let manifest = plugin.manifest();
        let row = self.store.plugin(plugin.name())?;
        let declared = Permission::declared(&manifest);
        let granted = declared.iter().copied().filter(|p| !row.revoked.contains(p)).collect();
        let points = match (&state, &external) {
            (PluginState::On, _) => points_of(plugin),
            (_, Some(external)) => external.declared_points(),
            _ => Vec::new(),
        };
        let memory = match &external {
            None => Memory::Shared,
            Some(external) => match (external.pid(), external.memory()) {
                (None, _) => Memory::NotRunning,
                (Some(_), Some(bytes)) => Memory::Bytes(bytes),
                (Some(_), None) => Memory::Unknown,
            },
        };
        Ok(PluginView {
            name: plugin.name().to_owned(),
            version: manifest.version.map(|v| v.to_string()),
            summary: plugin.summary(),
            origin: external
                .as_ref()
                .map_or(Origin::Embedded, |e| Origin::External(e.folder().to_path_buf())),
            enabled: row.enabled,
            state,
            dependencies: plugin.dependencies(),
            requires: manifest
                .requires
                .iter()
                .map(|(plugin, range)| Requirement {
                    plugin: plugin.clone(),
                    accepts: format!("a partir da {} e abaixo da {}", range.from, range.below),
                })
                .collect(),
            declared,
            granted,
            points,
            settings: row.settings,
            memory,
        })
    }
}

/// A refusal of the plan, in Portuguese, knowing which plugins the administrator switched off.
fn in_words(refusal: &ModuleRefusal, switched_off: &[String]) -> String {
    match refusal {
        ModuleRefusal::AddedTwice { module } => format!("há outro plugin com o nome {module}"),
        ModuleRefusal::Missing { needs, .. } if switched_off.contains(needs) => {
            format!("precisa de {needs}, que está desligado")
        }
        ModuleRefusal::Missing { needs, .. } => format!("precisa de {needs}, que não está instalado"),
        ModuleRefusal::Cycle { modules, .. } => format!("depende em ciclo de {}", modules.join(", ")),
        ModuleRefusal::NeedsRefused { needs, .. } => format!("precisa de {needs}, que ficou de fora"),
        ModuleRefusal::WrongVersion { needs, range, found: Some(found), .. } => format!(
            "precisa de {needs} a partir da {} e abaixo da {}, e o instalado é {found}",
            range.from, range.below
        ),
        ModuleRefusal::WrongVersion { needs, range, found: None, .. } => format!(
            "precisa de {needs} a partir da {} e abaixo da {}, e {needs} não diz a versão",
            range.from, range.below
        ),
        ModuleRefusal::Target { .. } => "não roda num servidor".to_owned(),
        ModuleRefusal::Database { database, .. } => format!("não funciona com {database}"),
        ModuleRefusal::Capability { capability, .. } => {
            format!("precisa de {capability}, que o Raven CI não oferece")
        }
        ModuleRefusal::Duplicate { kind, id, first, .. } => {
            let kind = if *kind == "job" { "o job" } else { "a migração" };
            format!("{kind} {id} também vem de {first}")
        }
        other => other.to_string(),
    }
}
