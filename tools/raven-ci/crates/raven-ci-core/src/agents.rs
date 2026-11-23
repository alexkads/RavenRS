//! Build agents: the server's side of `raven-ci-agent`, the program that runs builds elsewhere
//! (another machine, another user) and connects out to the server.
//!
//! # Its role
//!
//! The server keeps the queue, the log, the attachments and the status of every build; an agent
//! takes a build by a lease of the queue and runs its life (see [`lifecycle`](crate::Ci::run_order))
//! in a folder of its own, with only the plugins the build needs. Nothing is opened where the
//! agent runs: it speaks HTTP to the server and hears it over Server-Sent Events.
//!
//! # How a job picks its agent
//!
//! - `agent = { label = "linux" }` in a pipeline is [`Pipeline::agent`](raven_ci_plugin_api::Pipeline::agent),
//!   kept with the build. Such a build leaves the queue only for an agent with the label; the
//!   server's own executors never take it.
//! - `agent = "any"`, or none: any agent, or an executor of the server.
//! - With no agent connected that has the label, the core looks for an [`AgentTemplate`] with the
//!   label and asks the [`AgentLauncher`] it names to bring one up, with a token used once. An
//!   agent brought up that stays idle past the template's idle time is taken down.
//! - `agent = { docker = {…} }` names a launcher itself: it brings up an agent for that build only,
//!   with the label `build:<job>#<number>`, taken down when the build ends.
//!
//! # How an agent gets in
//!
//! A token used once ([`Ci::agent_token`], or a launcher's [`LaunchRequest::token`]) is kept only
//! as its SHA-256 and lasts [`TOKEN_MS`]. `POST /api/ci/agentes/entrar` spends it and makes the
//! agent a key of `ravenrs-app-server` (`rvk_…`) for the user `agente:<name>` with the role
//! [`ROLE_AGENT`], which opens the routes of agents and nothing else. The agent keeps the key and
//! comes back with it after it falls. [`Ci::remove_agent`] revokes it.
//!
//! # When an agent falls
//!
//! Its builds stay held by `agente/<name>` in the queue. While it says hello (every few seconds),
//! the server renews their leases. When it goes quiet, the server goes on renewing them for the
//! grace time ([`Ci::set_agent_grace`]), so no one else takes them:
//!
//! - an agent that lost the network sends again what it kept, and the server drops what it already
//!   has (by the agent's session and the place of each line);
//! - an agent that came back as a new process gets its builds again from `pegar`, with the
//!   checkpoint of the queue, and resumes them (`Lease::run`, with the `reconcile` of the step that
//!   was running);
//! - past the grace time, the build ends as interrupted, with its commit's status and notices, and
//!   the agent that comes back later is told to forget it.
//!
//! # The routes
//!
//! Under `/api/ci/agentes/`, JSON unless said: `entrar` (open), `oi`, `pegar`, `passos`, `log`,
//! `codigo`, `anexo` (raw bytes, in parts), `desfecho`, `fim` and `segredo`, each for the agent the
//! key is of. `GET /api/ci/agents` lists the agents for anyone signed in. The doorbell is the topic
//! `agente-<name>` of the live channel: `trabalho` (there is a build for it) and `cancelar`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use raven_ci_plugin_api::{
    AgentArgs, AgentLauncher, Attachment, BuildStatus, CiError, LaunchRequest, LaunchedAgent, PLUGIN_KEY,
    Permission, Use,
};
use ravenrs_app::UserDirectory;
use ravenrs_app_server::{ApiRequest, ApiResponse, Caller, LiveChannels};
use ravenrs_flows::{Catalog, FlowDefinition, RunOutcome};
use ravenrs_flows_runtime::{Held, Lease, RunStatus, Runtime, StepRecord, StoreError};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::host::Secrets;
use crate::lifecycle::{BuildLogger, BuildOrder, Extras};
use crate::model::{self, BuildRow};
use crate::{Ci, runtime_error};

pub use raven_ci_plugin_api::ROLE_AGENT;

/// How long a token used once lasts.
pub const TOKEN_MS: i64 = 10 * 60 * 1000;

/// How long the builds of an agent that went quiet wait for it, unless the server says otherwise.
pub const DEFAULT_GRACE_MS: u64 = 5 * 60 * 1000;

/// How often an agent says hello.
pub const HELLO_MS: u64 = 5_000;

/// How long since its last hello an agent still counts as connected.
const CONNECTED_MS: i64 = 3 * HELLO_MS as i64;

/// The largest part of an attachment an agent sends at once: under the 4 MB a body may have.
pub const PART_BYTES: usize = 2 * 1024 * 1024;

/// The prefix of the name of an agent's user, and of the worker that holds its builds.
const USER_PREFIX: &str = "agente:";
const WORKER_PREFIX: &str = "agente/";

/// The label of the agent a launcher brings up for one build.
pub fn build_label(job: &str, number: u64) -> String {
    format!("build:{job}#{number}")
}

/// The worker that holds the builds of the agent `name` in the queue.
pub fn worker_of(name: &str) -> String {
    format!("{WORKER_PREFIX}{name}")
}

/// The topic of the live channel where the server tells how its agents are: events `agent`, with
/// `{name, state, labels, busy}`, `state` being `conectado`, `sumido` or `removido`. What the
/// screen of agents listens to.
pub const AGENTS_TOPIC: &str = "agentes";

/// The topic of the live channel that rings the agent `name`.
pub fn topic_of(name: &str) -> String {
    format!("agente-{name}")
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ci_agents (
    name TEXT PRIMARY KEY,
    labels TEXT NOT NULL DEFAULT '[]',
    executors INTEGER NOT NULL DEFAULT 1,
    key_id TEXT,
    created_ms INTEGER NOT NULL,
    launched_by TEXT
);
CREATE TABLE IF NOT EXISTS ci_agent_tokens (
    hash TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    labels TEXT NOT NULL DEFAULT '[]',
    executors INTEGER NOT NULL DEFAULT 1,
    expires_ms INTEGER NOT NULL,
    launched_by TEXT
);
CREATE TABLE IF NOT EXISTS ci_agent_templates (
    name TEXT PRIMARY KEY,
    body TEXT NOT NULL
);
";

pub(crate) fn prepare(conn: &Connection) -> Result<(), CiError> {
    conn.execute_batch(SCHEMA).map_err(db)
}

fn db(e: rusqlite::Error) -> CiError {
    CiError::failed(format!("os agentes: {e}"))
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn random() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// Whether `name` may name an agent: letters, digits, `-`, `_` and `.`, up to 64.
fn check_name(name: &str) -> Result<(), CiError> {
    let fits = !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if fits {
        Ok(())
    } else {
        Err(CiError::settings(
            "nome",
            format!("{name} não serve de nome de agente: use letras, números, - _ e ."),
        ))
    }
}

// ----- What goes over the wire -----

/// What an agent says of itself, at every hello.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    /// Its labels.
    #[serde(default)]
    pub labels: Vec<String>,
    /// How many builds it runs at once.
    #[serde(default)]
    pub executors: u32,
    /// How many of them are free now.
    #[serde(default)]
    pub free: u32,
    /// The runs of the builds it runs now.
    #[serde(default)]
    pub running: Vec<String>,
    /// The runs of builds whose folders it still has from a life before, and does not run: the
    /// server says which of them to forget (they ended without it) and gives back the rest.
    #[serde(default)]
    pub left: Vec<String>,
    /// The memory it holds with what it runs, in bytes, when it can tell.
    #[serde(default)]
    pub memory_bytes: Option<u64>,
    /// Its limits and how much of them it uses (cores, memory, disk), as it says them.
    #[serde(default)]
    pub limits: Value,
    /// Its version.
    #[serde(default)]
    pub version: String,
    /// The life of its process: a new one each time it starts, so the server knows which builds it
    /// lost and has to be given again.
    pub session: String,
}

/// What an agent sends to get in: its name, the token used once, and its hello.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Join {
    /// Its name.
    pub name: String,
    /// The token used once.
    pub token: String,
    /// What it says of itself.
    pub hello: Hello,
}

/// What an agent got in with: its key, to keep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Joined {
    /// The key it presents from now on.
    pub key: String,
}

/// A build an agent is told to stop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cancel {
    /// The run.
    pub run: String,
    /// Why.
    pub reason: String,
}

/// What the server answers a hello.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HelloAnswer {
    /// Builds someone asked to cancel.
    pub cancel: Vec<Cancel>,
    /// Builds that are no longer the agent's (they ended without it): it stops them and removes
    /// their folders.
    pub forget: Vec<String>,
}

/// A plugin a build needs, with what the server set for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginUse {
    /// Its name.
    pub name: String,
    /// Its configuration on the server.
    pub settings: Value,
    /// The permissions the administrator took away from it.
    #[serde(default)]
    pub revoked: Vec<Permission>,
}

/// A build handed to an agent: what to run, with what, and from where.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// The build.
    pub order: BuildOrder,
    /// The plugins it needs, and only those.
    pub plugins: Vec<PluginUse>,
    /// Its run, from where the queue has it.
    pub lease: Lease,
}

/// A batch of lines of a build's log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogBatch {
    /// The run.
    pub run: String,
    /// The agent's session.
    pub session: String,
    /// The place of the first line among all the agent sent for this build in this session.
    pub from: u64,
    /// The lines.
    pub lines: Vec<AgentLine>,
}

/// A line of a build's log, as an agent sends it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentLine {
    /// The step that wrote it; empty for the core.
    pub step: String,
    /// The text, with what the agent knows to hide already hidden.
    pub text: String,
}

/// How a build an agent ran ended, once its post steps, publishers and hooks are done.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct End {
    /// The run.
    pub run: String,
    /// How it ended.
    pub status: BuildStatus,
    /// The commit it built, when there was one.
    pub commit: Option<String>,
}

/// An agent as the list shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentView {
    /// Its name.
    pub name: String,
    /// Its labels.
    pub labels: Vec<String>,
    /// How many builds it runs at once.
    pub executors: u32,
    /// Whether it said hello lately.
    pub connected: bool,
    /// How long ago it said hello last, since this server started.
    pub seen_ms_ago: Option<u64>,
    /// What its last hello said.
    pub hello: Option<Hello>,
    /// The template that brought it up, when one did.
    pub launched_by: Option<String>,
}

/// Why a build waits in the queue: [`Ci::waiting_for`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Waiting {
    /// What it waits for, in words.
    pub why: String,
    /// The plugin switched off that would let it go, when that is the reason.
    pub plugin: Option<String>,
}

impl Waiting {
    fn new(why: impl Into<String>) -> Self {
        Self { why: why.into(), plugin: None }
    }

    fn off(plugin: &str, why: String) -> Self {
        Self { why, plugin: Some(plugin.to_owned()) }
    }
}

/// How agents of some labels are brought up when none is connected: like a cloud of Jenkins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentTemplate {
    /// Its name; the agents it brings up are named after it.
    pub name: String,
    /// The labels its agents have.
    pub labels: Vec<String>,
    /// The id of the launcher that brings them up: `agent-local`, `agent-remote`.
    pub launcher: String,
    /// The launcher's settings: user, folder, machine, limits.
    #[serde(default)]
    pub settings: Value,
    /// How many builds each agent runs at once.
    #[serde(default = "one")]
    pub executors: u32,
    /// How many of its agents may be up at once.
    #[serde(default = "one")]
    pub max_agents: u32,
    /// How long one of its agents may stay idle before it is taken down, in milliseconds.
    #[serde(default = "idle")]
    pub idle_ms: u64,
}

fn one() -> u32 {
    1
}

fn idle() -> u64 {
    10 * 60 * 1000
}

// ----- The state the server keeps of its agents -----

#[derive(Debug)]
struct Seen {
    at_ms: i64,
    hello: Hello,
    /// The runs given to it in its session.
    assigned: BTreeSet<String>,
    /// What the topic of agents was last told of it: its state and how many builds it ran.
    told: (String, usize),
}

struct Launched {
    name: String,
    template: Option<String>,
    /// The run of the one build it was brought up for.
    build: Option<String>,
    agent: Box<dyn LaunchedAgent>,
    since_ms: i64,
    idle_since_ms: Option<i64>,
}

#[derive(Default)]
struct State {
    seen: HashMap<String, Seen>,
    /// How many lines of each build's log the server took, by run and session.
    lines: HashMap<(String, String), u64>,
    /// The parts of attachments on their way, by run and name.
    parts: HashMap<(String, String), Vec<u8>>,
    launched: Vec<Launched>,
    /// Until when a template (or a build's launcher) is not tried again, after a launch that
    /// failed or an agent that died before it came in.
    backoff: HashMap<String, i64>,
}

/// How long a template that failed waits before the next try.
const BACKOFF_MS: i64 = 30_000;

/// What the core keeps of its agents. See the [module](self).
pub(crate) struct Agents {
    state: Mutex<State>,
    live: Mutex<Option<LiveChannels>>,
    server: Mutex<String>,
    users: Option<PathBuf>,
    secrets: Arc<dyn Secrets>,
    grace_ms: AtomicU64,
    /// When the server came up: an agent not seen since then gets the grace from here.
    base_ms: i64,
}

impl Agents {
    pub(crate) fn new(users: Option<PathBuf>, secrets: Arc<dyn Secrets>, now_ms: i64) -> Self {
        Self {
            state: Mutex::default(),
            live: Mutex::default(),
            server: Mutex::new("http://127.0.0.1:8080".to_owned()),
            users,
            secrets,
            grace_ms: AtomicU64::new(DEFAULT_GRACE_MS),
            base_ms: now_ms,
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tells whoever listens to the topic [`AGENTS_TOPIC`] how the agent `name` is now.
    fn tell(&self, name: &str, state: &str, labels: &[String], busy: usize) {
        if let Some(live) = self.live.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
            live.open(AGENTS_TOPIC);
            live.publish(
                AGENTS_TOPIC,
                "agent",
                json!({ "name": name, "state": state, "labels": labels, "busy": busy }),
            );
        }
    }

    fn ring(&self, name: &str, kind: &str, data: Value) {
        if let Some(live) = self.live.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
            let topic = topic_of(name);
            live.open(topic.clone());
            live.publish(&topic, kind, data);
        }
    }
}

/// The agent a key is of, from who sent the request.
fn agent_of(caller: &Caller) -> Option<String> {
    let name = caller.name.strip_prefix(USER_PREFIX)?;
    // A key's caller is named "user (key)".
    let name = name.split(' ').next().unwrap_or(name);
    caller.roles.iter().any(|role| role == ROLE_AGENT).then(|| name.to_owned())
}

impl Ci {
    /// Rings agents through `live`, the server's live channel, and opens the topic of every agent
    /// it knows.
    pub fn set_agent_live(&self, live: LiveChannels) {
        if let Ok(names) = self.agent_names() {
            for name in names {
                live.open(topic_of(&name));
            }
        }
        live.open(AGENTS_TOPIC);
        *self.agents.live.lock().unwrap_or_else(PoisonError::into_inner) = Some(live);
    }

    /// Where agents brought up by a launcher connect to: the server's address, as they reach it.
    pub fn set_agent_server(&self, url: &str) {
        *self.agents.server.lock().unwrap_or_else(PoisonError::into_inner) =
            url.trim_end_matches('/').to_owned();
    }

    /// Leases of the queue of `lease` (60 s unless said): how long a worker that went quiet holds a
    /// build before another may take it. The server's executors renew them while a step runs.
    pub fn set_lease(&self, lease: Duration) {
        let ms = u64::try_from(lease.as_millis()).unwrap_or(u64::MAX);
        let mut runtime = self.runtime.write().unwrap_or_else(PoisonError::into_inner);
        *runtime = Arc::new(runtime.with_catalog(self.registry.catalog()).lease_ms(ms));
    }

    /// How long the builds of an agent that went quiet wait for it before they end as
    /// interrupted ([`DEFAULT_GRACE_MS`] unless said).
    pub fn set_agent_grace(&self, grace: Duration) {
        self.agents.grace_ms.store(u64::try_from(grace.as_millis()).unwrap_or(u64::MAX), Ordering::SeqCst);
    }

    fn agent_names(&self) -> Result<Vec<String>, CiError> {
        let db = self.db();
        let mut stmt = db.prepare("SELECT name FROM ci_agents ORDER BY name").map_err(db_error)?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(db_error)?;
        rows.collect::<Result<_, _>>().map_err(db_error)
    }

    /// A token used once for the agent `name`, with `labels` and `executors`, lasting [`TOKEN_MS`]:
    /// what `raven-ci agentes token` prints for an agent installed by hand.
    pub fn agent_token(&self, name: &str, labels: &[String], executors: u32) -> Result<String, CiError> {
        self.token_for(name, labels, executors, None)
    }

    fn token_for(
        &self,
        name: &str,
        labels: &[String],
        executors: u32,
        launched_by: Option<&str>,
    ) -> Result<String, CiError> {
        check_name(name)?;
        let token = random();
        let expires = self.store.now_ms().saturating_add(TOKEN_MS);
        self.db()
            .execute(
                "INSERT INTO ci_agent_tokens (hash, name, labels, executors, expires_ms, launched_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    sha256(&token),
                    name,
                    json!(labels).to_string(),
                    executors.max(1),
                    expires,
                    launched_by
                ],
            )
            .map_err(db_error)?;
        Ok(format!("rva_{token}"))
    }

    /// Lets the agent of `join` in, spending its token; the key it keeps. A token used, run out
    /// or of another name is refused as [`CiError::Denied`].
    pub fn agent_join(&self, join: &Join) -> Result<Joined, CiError> {
        check_name(&join.name)?;
        let refuse = || denied("entrar sem um token válido");
        let token = join.token.strip_prefix("rva_").ok_or_else(refuse)?;
        let now = self.store.now_ms();
        let found: Option<(String, i64, Option<String>)> = {
            let db = self.db();
            let found = db
                .query_row(
                    "SELECT name, expires_ms, launched_by FROM ci_agent_tokens WHERE hash = ?1",
                    params![sha256(token)],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(db_error)?;
            // Used once: it goes whatever comes of it.
            db.execute(
                "DELETE FROM ci_agent_tokens WHERE hash = ?1 OR expires_ms < ?2",
                params![sha256(token), now],
            )
            .map_err(db_error)?;
            found
        };
        let (name, expires, launched_by) = found.ok_or_else(refuse)?;
        if name != join.name || expires < now {
            return Err(refuse());
        }
        let key = self.issue_agent_key(&name)?;
        let old: Option<String> = self
            .db()
            .query_row("SELECT key_id FROM ci_agents WHERE name = ?1", params![name], |row| row.get(0))
            .optional()
            .map_err(db_error)?
            .flatten();
        if let (Some(old), Some(file)) = (old, &self.agents.users) {
            let maker = crate::people::users(file)?;
            let _ = ravenrs_app_server::revoke_key(&maker, &old);
        }
        self.db()
            .execute(
                "INSERT INTO ci_agents (name, labels, executors, key_id, created_ms, launched_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (name) DO UPDATE SET labels = excluded.labels, executors = excluded.executors,
                     key_id = excluded.key_id, launched_by = excluded.launched_by",
                params![
                    name,
                    json!(join.hello.labels).to_string(),
                    join.hello.executors.max(1),
                    key.id,
                    now,
                    launched_by
                ],
            )
            .map_err(db_error)?;
        self.log.write(&format!("agente {name} entrou, com os rótulos {}", join.hello.labels.join(", ")));
        self.agent_hello(&name, &join.hello)?;
        Ok(Joined { key: key.key })
    }

    fn issue_agent_key(&self, name: &str) -> Result<ravenrs_app_server::IssuedKey, CiError> {
        let file = self
            .agents
            .users
            .as_ref()
            .ok_or_else(|| CiError::unavailable("um núcleo em memória não recebe agentes"))?;
        let maker = crate::people::users(file)?;
        let directory = UserDirectory::new(maker.clone(), crate::people::roles())
            .map_err(|e| CiError::failed(e.to_string()))?;
        if directory.is_empty().map_err(|e| CiError::failed(e.to_string()))? {
            return Err(CiError::unavailable(
                "crie antes o primeiro usuário (raven-ci usuario criar): ele é o administrador, e um agente não pode ser",
            ));
        }
        let user = format!("{USER_PREFIX}{name}");
        if directory.identity_of(&user).map_err(|e| CiError::failed(e.to_string()))?.is_none() {
            crate::people::create_user(file, &user, &random(), ROLE_AGENT)?;
        }
        ravenrs_app_server::issue_key(
            &maker,
            crate::people::roles(),
            &user,
            &format!("agente {name}"),
            &[],
            None,
        )
        .map_err(|e| CiError::failed(format!("a chave do agente {name}: {e}")))
    }

    /// Forgets the agent `name`: its key stops working, and so does its user.
    pub fn remove_agent(&self, name: &str) -> Result<(), CiError> {
        let key: Option<Option<String>> = self
            .db()
            .query_row("SELECT key_id FROM ci_agents WHERE name = ?1", params![name], |row| row.get(0))
            .optional()
            .map_err(db_error)?;
        let Some(key) = key else { return Err(CiError::unavailable(format!("não há o agente {name}"))) };
        if let (Some(key), Some(file)) = (key, &self.agents.users) {
            let maker = crate::people::users(file)?;
            ravenrs_app_server::revoke_key(&maker, &key).map_err(|e| CiError::failed(e.to_string()))?;
        }
        self.db().execute("DELETE FROM ci_agents WHERE name = ?1", params![name]).map_err(db_error)?;
        self.agents.state().seen.remove(name);
        self.agents.tell(name, "removido", &[], 0);
        self.log.write(&format!("agente {name} removido"));
        Ok(())
    }

    /// Every agent the server knows, by name.
    pub fn agents(&self) -> Result<Vec<AgentView>, CiError> {
        let rows: Vec<(String, String, u32, Option<String>)> = {
            let db = self.db();
            let mut stmt = db
                .prepare("SELECT name, labels, executors, launched_by FROM ci_agents ORDER BY name")
                .map_err(db_error)?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
                .map_err(db_error)?;
            rows.collect::<Result<_, _>>().map_err(db_error)?
        };
        let now = self.store.now_ms();
        let state = self.agents.state();
        Ok(rows
            .into_iter()
            .map(|(name, labels, executors, launched_by)| {
                let seen = state.seen.get(&name);
                AgentView {
                    labels: serde_json::from_str(&labels).unwrap_or_default(),
                    executors,
                    connected: seen.is_some_and(|seen| now - seen.at_ms <= CONNECTED_MS),
                    seen_ms_ago: seen.map(|seen| u64::try_from(now - seen.at_ms).unwrap_or(0)),
                    hello: seen.map(|seen| seen.hello.clone()),
                    launched_by,
                    name,
                }
            })
            .collect())
    }

    fn known(&self, name: &str) -> Result<(), CiError> {
        let found: Option<i64> = self
            .db()
            .query_row("SELECT 1 FROM ci_agents WHERE name = ?1", params![name], |row| row.get(0))
            .optional()
            .map_err(db_error)?;
        found.map(|_| ()).ok_or_else(|| denied(format!("o agente {name} foi removido")))
    }

    /// The agent `name` says `hello`: the server renews the leases of what it runs, and answers
    /// what to cancel and what to forget.
    pub fn agent_hello(&self, name: &str, hello: &Hello) -> Result<HelloAnswer, CiError> {
        self.known(name)?;
        let now = self.store.now_ms();
        {
            let mut state = self.agents.state();
            let seen = state.seen.entry(name.to_owned()).or_insert_with(|| Seen {
                at_ms: now,
                hello: hello.clone(),
                assigned: BTreeSet::new(),
                told: (String::new(), 0),
            });
            if seen.hello.session != hello.session {
                seen.assigned.clear();
            }
            seen.assigned.extend(hello.running.iter().cloned());
            seen.at_ms = now;
            seen.hello = hello.clone();
            let now_told = ("conectado".to_owned(), hello.running.len());
            if seen.told != now_told {
                seen.told = now_told;
                self.agents.tell(name, "conectado", &hello.labels, hello.running.len());
            }
        }
        if let Some(live) = self.agents.live.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
            live.open(topic_of(name));
        }
        let runtime = self.runtime();
        let worker = worker_of(name);
        let mut answer = HelloAnswer::default();
        for run in &hello.running {
            match runtime.renew(run, &worker) {
                Ok(Held { held: true, cancel }) => {
                    if let Some(reason) = cancel {
                        answer.cancel.push(Cancel { run: run.clone(), reason });
                    }
                }
                // An ended build's post steps may still be running there: only one not ended, or
                // ended without it, is forgotten.
                Ok(_) | Err(_) => {
                    let record = runtime.get(run).ok();
                    let ours = record.as_ref().is_some_and(|r| r.worker.as_deref() == Some(worker.as_str()));
                    let interrupted = record.as_ref().is_some_and(|r| {
                        matches!(&r.outcome, Some(RunOutcome::Cancelled { reason }) if reason.starts_with(INTERRUPTED))
                    });
                    if !ours || interrupted || record.is_none() {
                        answer.forget.push(run.clone());
                    }
                }
            }
        }
        for run in &hello.left {
            let record = runtime.get(run).ok();
            let ours = record.as_ref().is_some_and(|r| {
                r.status == RunStatus::Running && r.worker.as_deref() == Some(worker.as_str())
            });
            if !ours {
                answer.forget.push(run.clone());
            }
        }
        Ok(answer)
    }

    /// The next build for the agent `name`: first one of its own it lost (it came back as a new
    /// process), else the build due first that it may take by its labels; `None` when there is
    /// none.
    pub fn agent_take(&self, name: &str) -> Result<Option<Task>, CiError> {
        self.known(name)?;
        let worker = worker_of(name);
        let (labels, assigned) = {
            let state = self.agents.state();
            let seen = state
                .seen
                .get(name)
                .ok_or_else(|| CiError::failed(format!("o agente {name} pega build antes de dizer oi")))?;
            (seen.hello.labels.clone(), seen.assigned.clone())
        };
        let runtime = self.runtime();
        // Its own first: builds it held before it fell, in this session not given back yet.
        let own: Vec<String> = runtime
            .held_by(&worker)
            .map_err(runtime_error)?
            .into_iter()
            .filter(|run| !assigned.contains(run))
            .collect();
        for run in own {
            let Ok(record) = runtime.get(&run) else { continue };
            if record.status == RunStatus::Running && record.worker.as_deref() == Some(worker.as_str()) {
                runtime.renew(&run, &worker).map_err(runtime_error)?;
                return self.task_of(&runtime, name, &run).map(Some);
            }
        }
        for run in runtime.due().map_err(runtime_error)? {
            let Some(row) = model::build_by_run(&self.db(), &run)? else { continue };
            let record = runtime.get(&run).map_err(runtime_error)?;
            // A build an agent holds, waiting for it to come back, is the watch's to decide.
            if record.status == RunStatus::Running
                && record.worker.as_deref().is_some_and(|w| w.starts_with(WORKER_PREFIX))
            {
                continue;
            }
            if !may_take(&row, &labels) {
                continue;
            }
            if runtime.claim_id(&run, &worker).map_err(runtime_error)? {
                return self.task_of(&runtime, name, &run).map(Some);
            }
        }
        Ok(None)
    }

    fn task_of(&self, runtime: &Runtime, name: &str, run: &str) -> Result<Task, CiError> {
        let row = model::build_by_run(&self.db(), run)?
            .ok_or_else(|| CiError::unavailable(format!("o run {run} não é de um build")))?;
        let lease = runtime.lease(run, &worker_of(name)).map_err(runtime_error)?;
        let order = self.order_of(&row);
        let plugins = self.plugins_for(&order, &row.flow)?;
        if let Some(seen) = self.agents.state().seen.get_mut(name) {
            seen.assigned.insert(run.to_owned());
        }
        self.log.write(&format!("{} #{} vai para o agente {name}", row.job, row.number));
        Ok(Task { order, plugins, lease })
    }

    /// The plugins on that a build needs: those of its steps and post steps, its `scm`, its
    /// publishers, and every one with a hook. Each with what the server set for it.
    fn plugins_for(&self, order: &BuildOrder, flow: &str) -> Result<Vec<PluginUse>, CiError> {
        let mut names: BTreeSet<String> = BTreeSet::new();
        let owners = self.registry.step_owners();
        let extras = Extras::from_value(&order.extras);
        let mut flows = vec![FlowDefinition::from_json(flow).map_err(|e| CiError::failed(e.to_string()))?];
        for post in &extras.post {
            if let Ok(flow) = FlowDefinition::from_json(&post.flow) {
                flows.push(flow);
            }
        }
        let builtin = Catalog::builtin();
        for flow in &flows {
            for node in &flow.nodes {
                if let Some(owner) = owners.get(&node.kind) {
                    names.insert(owner.clone());
                } else if let Some(plugin) = node.parameters.get(PLUGIN_KEY).and_then(Value::as_str)
                    && builtin.get(&node.kind, node.version).is_none()
                {
                    names.insert(plugin.to_owned());
                }
            }
        }
        let on = self.registry.on();
        let scm = order.settings.pointer("/scm/id").and_then(Value::as_str);
        for plugin in &on {
            let has_scm = scm.is_some_and(|id| plugin.scms().iter().any(|s| s.id() == id));
            let has_publisher = extras
                .publishers
                .iter()
                .any(|wanted| plugin.publishers().iter().any(|p| p.id() == wanted.id));
            if has_scm || has_publisher || !plugin.build_hooks().is_empty() {
                names.insert(plugin.name().to_owned());
            }
        }
        names
            .into_iter()
            .map(|name| {
                let view = self.registry.info(&name)?;
                let revoked = view.declared.iter().copied().filter(|p| !view.granted.contains(p)).collect();
                Ok(PluginUse { name, settings: view.settings, revoked })
            })
            .collect()
    }

    fn held_by(&self, name: &str, run: &str) -> Result<(Arc<Runtime>, String), CiError> {
        let runtime = self.runtime();
        let worker = worker_of(name);
        let record = runtime.get(run).map_err(runtime_error)?;
        if record.worker.as_deref() != Some(worker.as_str()) {
            return Err(denied(format!("o run {run} não é do agente {name}")));
        }
        Ok((runtime, worker))
    }

    /// Writes the steps the agent `name` ran of `run`, in order; whether it still holds it.
    pub fn agent_steps(&self, name: &str, run: &str, records: &[StepRecord]) -> Result<Held, CiError> {
        let runtime = self.runtime();
        let worker = worker_of(name);
        let mut held = Held { held: true, cancel: None };
        for record in records {
            held = runtime.record(run, &worker, record).map_err(runtime_error)?;
            if !held.held {
                break;
            }
        }
        if records.is_empty() {
            held = runtime.renew(run, &worker).map_err(runtime_error)?;
        }
        Ok(held)
    }

    fn logger_of(&self, run: &str) -> Result<(BuildRow, BuildLogger), CiError> {
        let row = model::build_by_run(&self.db(), run)?
            .ok_or_else(|| CiError::unavailable(format!("o run {run} não é de um build")))?;
        let logger = BuildLogger::new(self.store.clone(), &row.job, row.number, self.log.clone())?;
        Ok((row, logger))
    }

    /// Keeps the lines of `batch` the server does not have yet; how many it has of the session.
    pub fn agent_log(&self, name: &str, batch: &LogBatch) -> Result<u64, CiError> {
        self.held_by(name, &batch.run)?;
        let (_, logger) = self.logger_of(&batch.run)?;
        let key = (batch.run.clone(), batch.session.clone());
        let mut taken = self.agents.state().lines.get(&key).copied().unwrap_or(0);
        for (place, line) in (batch.from..).zip(&batch.lines) {
            if place < taken {
                continue;
            }
            raven_ci_plugin_api::BuildLog::line(&logger, &line.step, &line.text);
            taken = place + 1;
        }
        self.agents.state().lines.insert(key, taken);
        Ok(taken)
    }

    /// The agent `name` fetched the code of `run` at `commit`: pending, and the notices.
    pub fn agent_code(&self, name: &str, run: &str, commit: Option<&str>) -> Result<(), CiError> {
        self.held_by(name, run)?;
        let (row, logger) = self.logger_of(run)?;
        self.announce_start(&self.order_of(&row), commit, &logger);
        Ok(())
    }

    /// A part of the attachment `attachment` (its bytes, so far, in `part`) of `run`; with `last`,
    /// the server keeps it with the build.
    pub fn agent_attach(
        &self,
        name: &str,
        run: &str,
        attachment: &Attachment,
        last: bool,
    ) -> Result<Option<i64>, CiError> {
        self.held_by(name, run)?;
        let key = (run.to_owned(), attachment.name.clone());
        let bytes = {
            let mut state = self.agents.state();
            let bytes = state.parts.entry(key.clone()).or_default();
            bytes.extend_from_slice(&attachment.bytes);
            if !last {
                return Ok(None);
            }
            state.parts.remove(&key).unwrap_or_default()
        };
        let row = model::build_by_run(&self.db(), run)?
            .ok_or_else(|| CiError::unavailable(format!("o run {run} não é de um build")))?;
        let whole = Attachment { bytes, ..attachment.clone() };
        self.attach(&row.job, row.number, &whole).map(Some)
    }

    /// The steps of `run` ended on the agent `name` as `outcome`; how the build ended.
    pub fn agent_outcome(&self, name: &str, run: &str, outcome: &RunOutcome) -> Result<BuildStatus, CiError> {
        let runtime = self.runtime();
        match runtime.finish(run, &worker_of(name), outcome) {
            Ok(_) => {}
            Err(StoreError::NotHeld(_)) => {
                return Err(denied(format!("o run {run} não é mais do agente {name}")));
            }
            Err(e) => return Err(runtime_error(e)),
        }
        let row = model::build_by_run(&self.db(), run)?
            .ok_or_else(|| CiError::unavailable(format!("o run {run} não é de um build")))?;
        Ok(self.build(&row.job, row.number)?.status)
    }

    /// The build of `end` ended on the agent `name`, with its post steps, publishers and hooks:
    /// its commit's status, the notices and the retention. The agent brought up for it goes.
    pub fn agent_end(&self, name: &str, end: &End) -> Result<(), CiError> {
        self.held_by(name, &end.run)?;
        let (row, logger) = self.logger_of(&end.run)?;
        self.announce_end(&self.order_of(&row), end.commit.as_deref(), end.status, &logger);
        {
            let mut state = self.agents.state();
            if let Some(seen) = state.seen.get_mut(name) {
                seen.assigned.remove(&end.run);
            }
            state.lines.retain(|(run, _), _| *run != end.run);
            let mut going = Vec::new();
            state.launched.retain(|launched| {
                if launched.build.as_deref() == Some(end.run.as_str()) {
                    going.push(launched.agent.name());
                    launched.agent.stop();
                    false
                } else {
                    true
                }
            });
            drop(state);
            for gone in going {
                self.log.write(&format!("agente {gone} parado: o build dele terminou"));
            }
        }
        self.prune_job(&row.job)?;
        Ok(())
    }

    /// The secret `secret`, for the agent `name` while it runs a build; what else the log hides
    /// with it. Every read goes to the audit.
    pub fn agent_secret(&self, name: &str, secret: &str) -> Result<(String, Vec<String>), CiError> {
        let running = self.agents.state().seen.get(name).is_some_and(|seen| !seen.assigned.is_empty());
        let who = format!("{USER_PREFIX}{name}");
        if !running {
            self.store.audit(&who, Permission::Secrets, &format!("ler o segredo {secret} sem build"), false);
            return Err(denied("um agente só lê segredos enquanto roda um build"));
        }
        let value = self.agents.secrets.get(secret);
        self.store.audit(&who, Permission::Secrets, &format!("ler o segredo {secret}"), value.is_some());
        let value = value.ok_or_else(|| CiError::unavailable(format!("não há o segredo {secret}")))?;
        let hidden = self.agents.secrets.hidden(secret);
        self.log.add_hidden(&value);
        for other in &hidden {
            self.log.add_hidden(other);
        }
        Ok((value, hidden))
    }

    /// Keeps `template`, made or changed.
    pub fn save_agent_template(&self, template: &AgentTemplate) -> Result<(), CiError> {
        check_name(&template.name)?;
        if template.labels.is_empty() {
            return Err(CiError::settings("rotulos", "um modelo de agente tem pelo menos um rótulo"));
        }
        let body = serde_json::to_string(template).map_err(|e| CiError::failed(e.to_string()))?;
        self.db()
            .execute(
                "INSERT INTO ci_agent_templates (name, body) VALUES (?1, ?2)
                 ON CONFLICT (name) DO UPDATE SET body = excluded.body",
                params![template.name, body],
            )
            .map_err(db_error)?;
        Ok(())
    }

    /// Every template of agents, by name.
    pub fn agent_templates(&self) -> Result<Vec<AgentTemplate>, CiError> {
        let db = self.db();
        let mut stmt = db.prepare("SELECT body FROM ci_agent_templates ORDER BY name").map_err(db_error)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0)).map_err(db_error)?;
        let bodies: Vec<String> = rows.collect::<Result<_, _>>().map_err(db_error)?;
        Ok(bodies.iter().filter_map(|body| serde_json::from_str(body).ok()).collect())
    }

    /// Why a build of the queue still waits, in words for the panel, and the plugin that would let
    /// it go when one is switched off: a label no agent connected has, a template whose launcher is
    /// off, or only a free executor (#919).
    pub fn waiting_for(&self, job: &str, number: u64) -> Result<Waiting, CiError> {
        let row = model::build(&self.db(), job, number)?;
        let Some(wanted) = Extras::from_value(&row.extras).agent else {
            return Ok(Waiting::new("espera um executor livre"));
        };
        let plugin_off = |name: &str| -> Result<bool, CiError> {
            let known = self.registry.list()?.into_iter().any(|p| p.name == name);
            Ok(known && !self.registry.on().iter().any(|p| p.name() == name))
        };
        if wanted.id != "label" {
            let launcher = wanted.plugin.clone().unwrap_or_else(|| wanted.id.clone());
            if plugin_off(&launcher)? {
                return Ok(Waiting::off(&launcher, format!("pede um agente {launcher} · o plugin {launcher} está desligado")));
            }
            return Ok(Waiting::new(format!("espera o agente {launcher} que sobe só para ele")));
        }
        let label = wanted.settings.get("label").and_then(Value::as_str).unwrap_or_default().to_owned();
        let agents = self.agents()?;
        let with_label = |connected: bool| {
            agents.iter().filter(|a| a.connected == connected).any(|a| a.labels.iter().any(|l| *l == label))
        };
        if with_label(true) {
            return Ok(Waiting::new(format!("espera um agente com o rótulo {label} ficar livre")));
        }
        match self.agent_templates()?.into_iter().find(|t| t.labels.iter().any(|l| *l == label)) {
            Some(template) if plugin_off(&template.launcher)? => Ok(Waiting::off(
                &template.launcher,
                format!("pede o rótulo {label} · o plugin {} está desligado", template.launcher),
            )),
            Some(template) => Ok(Waiting::new(format!("pede o rótulo {label} · o modelo {} sobe um agente", template.name))),
            None => Ok(Waiting::new(format!("pede o rótulo {label}, e nenhum agente conectado o tem"))),
        }
    }

    /// Forgets the template `name`; the agents it brought up stay until they are idle.
    pub fn delete_agent_template(&self, name: &str) -> Result<(), CiError> {
        self.db()
            .execute("DELETE FROM ci_agent_templates WHERE name = ?1", params![name])
            .map_err(db_error)?;
        Ok(())
    }

    /// Takes the build due first that the server's own executors may run, for `worker`: one that
    /// names no agent, and is not held by an agent that went quiet. What [`Ci::work_once`] does.
    pub(crate) fn claim_local(&self, runtime: &Runtime, worker: &str) -> Result<Option<String>, CiError> {
        for run in runtime.due().map_err(runtime_error)? {
            let record = runtime.get(&run).map_err(runtime_error)?;
            if record.status == RunStatus::Running
                && record.worker.as_deref().is_some_and(|w| w.starts_with(WORKER_PREFIX))
            {
                continue;
            }
            if let Some(row) = model::build_by_run(&self.db(), &run)?
                && needs_agent(&row)
            {
                continue;
            }
            if runtime.claim_id(&run, worker).map_err(runtime_error)? {
                return Ok(Some(run));
            }
        }
        Ok(None)
    }

    /// One look at the agents: the leases of quiet agents within their grace, the builds past it
    /// (interrupted), the doorbells, agents to bring up and agents to take down. The watch
    /// ([`Ci::watch_agents`]) does it every second; a test does it when it moves its clock.
    pub fn tick_agents(&self) {
        if let Err(e) = self.tick() {
            self.log.write(&format!("os agentes: {e}"));
        }
    }

    fn tick(&self) -> Result<(), CiError> {
        let runtime = self.runtime();
        let now = self.store.now_ms();
        let grace = i64::try_from(self.agents.grace_ms.load(Ordering::SeqCst)).unwrap_or(i64::MAX);
        // The builds each agent holds: renewed while it is within its grace, interrupted past it.
        for name in self.agent_names()? {
            let worker = worker_of(&name);
            let held = runtime.held_by(&worker).map_err(runtime_error)?;
            if held.is_empty() {
                continue;
            }
            let last = self.agents.state().seen.get(&name).map_or(self.agents.base_ms, |seen| seen.at_ms);
            for run in held {
                if now - last <= grace {
                    runtime.renew(&run, &worker).map_err(runtime_error)?;
                } else {
                    self.interrupt(&runtime, &run, &name, now - last)?;
                }
            }
        }
        let mut waiting: Vec<BuildRow> = Vec::new();
        for run in runtime.due().map_err(runtime_error)? {
            let record = runtime.get(&run).map_err(runtime_error)?;
            let by_agent = record.worker.as_deref().is_some_and(|w| w.starts_with(WORKER_PREFIX));
            if record.status == RunStatus::Running && by_agent {
                continue;
            }
            if let Some(row) = model::build_by_run(&self.db(), &run)? {
                waiting.push(row);
            }
        }
        // Who went quiet: the topic of agents hears it once.
        {
            let mut state = self.agents.state();
            for (name, seen) in &mut state.seen {
                if now - seen.at_ms > CONNECTED_MS && seen.told.0 != "sumido" {
                    seen.told = ("sumido".to_owned(), seen.told.1);
                    self.agents.tell(name, "sumido", &seen.hello.labels, seen.told.1);
                }
            }
        }
        self.dispatch(&waiting, now)?;
        self.tidy_launched(now);
        Ok(())
    }

    /// Ends `run`, held by the agent `name` that went quiet `ago` ms ago, as interrupted.
    fn interrupt(&self, runtime: &Runtime, run: &str, name: &str, ago: i64) -> Result<(), CiError> {
        let reason = format!("{INTERRUPTED} o agente {name} sumiu há {} s", ago / 1000);
        match runtime.finish(run, &worker_of(name), &RunOutcome::Cancelled { reason: reason.clone() }) {
            Ok(_) | Err(StoreError::NotHeld(_)) => {}
            Err(e) => return Err(runtime_error(e)),
        }
        let (row, logger) = self.logger_of(run)?;
        raven_ci_plugin_api::BuildLog::line(&logger, "", &reason);
        let status = self.build(&row.job, row.number)?.status;
        self.announce_end(&self.order_of(&row), None, status, &logger);
        raven_ci_plugin_api::BuildLog::line(&logger, "", &format!("terminou: {}", status.words()));
        if let Some(seen) = self.agents.state().seen.get_mut(name) {
            seen.assigned.remove(run);
        }
        self.log.write(&format!("{} #{}: {reason}", row.job, row.number));
        self.prune_job(&row.job)?;
        Ok(())
    }

    /// Rings the agents that may take a waiting build, and brings one up for a build none may.
    fn dispatch(&self, waiting: &[BuildRow], now: i64) -> Result<(), CiError> {
        let agents = self.agents()?;
        let mut rung: BTreeSet<String> = BTreeSet::new();
        for row in waiting {
            let free: Vec<&AgentView> = agents
                .iter()
                .filter(|agent| agent.connected)
                .filter(|agent| {
                    agent.hello.as_ref().is_some_and(|hello| hello.free > 0 && may_take(row, &hello.labels))
                })
                .collect();
            if !free.is_empty() {
                for agent in free {
                    if rung.insert(agent.name.clone()) {
                        self.agents.ring(&agent.name, "trabalho", json!({}));
                    }
                }
                continue;
            }
            let Some(wanted) = Extras::from_value(&row.extras).agent else { continue };
            // A connected agent with the label that is only busy takes it when it is free.
            let any_with_label = agents
                .iter()
                .filter(|a| a.connected)
                .any(|a| a.hello.as_ref().is_some_and(|hello| may_take(row, &hello.labels)));
            if any_with_label {
                continue;
            }
            let key = launch_key(row, &wanted, &self.agent_templates()?);
            if self.agents.state().backoff.get(&key).is_some_and(|until| now < *until) {
                continue;
            }
            if let Err(e) = self.launch_for(row, &wanted, now) {
                let said = format!("{} #{}: subir um agente: {e}", row.job, row.number);
                self.log.write(&said);
                self.agents.state().backoff.insert(key, now + BACKOFF_MS);
            }
        }
        Ok(())
    }

    fn launcher(&self, id: &str) -> Result<Arc<dyn AgentLauncher>, CiError> {
        self.registry
            .on()
            .iter()
            .flat_map(|plugin| plugin.agent_launchers())
            .find(|launcher| launcher.id() == id)
            .ok_or_else(|| CiError::unavailable(format!("falta o plugin que sobe agentes com {id}")))
    }

    /// Brings an agent up for `row`, which wants `wanted`, unless one is on its way.
    fn launch_for(&self, row: &BuildRow, wanted: &Use, now: i64) -> Result<(), CiError> {
        if wanted.id == "label" {
            let label = wanted.settings.get("label").and_then(Value::as_str).unwrap_or_default();
            let template = self
                .agent_templates()?
                .into_iter()
                .find(|template| template.labels.iter().any(|l| l == label))
                .ok_or_else(|| {
                    CiError::unavailable(format!(
                        "nenhum agente conectado tem o rótulo {label}, e nenhum modelo o sobe"
                    ))
                })?;
            let (up, coming) = {
                let state = self.agents.state();
                let mine: Vec<&Launched> = state
                    .launched
                    .iter()
                    .filter(|l| l.template.as_deref() == Some(template.name.as_str()))
                    .collect();
                let coming = mine.iter().any(|l| !state.seen.contains_key(&l.name));
                (mine.len(), coming)
            };
            if coming || up >= template.max_agents as usize {
                return Ok(());
            }
            let name = format!("{}-{}", template.name, &uuid::Uuid::new_v4().simple().to_string()[..8]);
            let request = LaunchRequest {
                build: None,
                labels: template.labels.clone(),
                settings: with_agent(&template.settings, &name, template.executors),
                server: self.agents.server.lock().unwrap_or_else(PoisonError::into_inner).clone(),
                token: self.token_for(&name, &template.labels, template.executors, Some(&template.name))?,
            };
            let agent = self.launcher(&template.launcher)?.launch(&request)?;
            self.log.write(&format!(
                "agente {name} subindo pelo modelo {} ({})",
                template.name, template.launcher
            ));
            self.agents.state().launched.push(Launched {
                name,
                template: Some(template.name),
                build: None,
                agent,
                since_ms: now,
                idle_since_ms: None,
            });
            return Ok(());
        }
        if self.agents.state().launched.iter().any(|l| l.build.as_deref() == Some(row.run_id.as_str())) {
            return Ok(());
        }
        let label = build_label(&row.job, row.number);
        let name =
            format!("{}-{}-{}", wanted.id, row.number, &uuid::Uuid::new_v4().simple().to_string()[..6]);
        let request = LaunchRequest {
            build: Some(format!("{}#{}", row.job, row.number)),
            labels: vec![label.clone()],
            settings: with_agent(&wanted.settings, &name, 1),
            server: self.agents.server.lock().unwrap_or_else(PoisonError::into_inner).clone(),
            token: self.token_for(&name, std::slice::from_ref(&label), 1, Some(&wanted.id))?,
        };
        let agent = self.launcher(&wanted.id)?.launch(&request)?;
        self.log.write(&format!("agente {name} subindo para {} #{} ({})", row.job, row.number, wanted.id));
        self.agents.state().launched.push(Launched {
            name,
            template: None,
            build: Some(row.run_id.clone()),
            agent,
            since_ms: now,
            idle_since_ms: None,
        });
        Ok(())
    }

    /// Takes down the agents brought up that stayed idle past their template's time, that never
    /// came in, or that died.
    fn tidy_launched(&self, now: i64) {
        let templates: BTreeMap<String, AgentTemplate> =
            self.agent_templates().unwrap_or_default().into_iter().map(|t| (t.name.clone(), t)).collect();
        let mut going: Vec<(String, String)> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        {
            let mut state = self.agents.state();
            let seen: HashMap<String, (bool, bool)> = state
                .seen
                .iter()
                .map(|(name, seen)| {
                    (name.clone(), (seen.assigned.is_empty(), now - seen.at_ms <= CONNECTED_MS))
                })
                .collect();
            state.launched.retain_mut(|launched| {
                let (idle, connected) = seen.get(&launched.name).copied().unwrap_or((true, false));
                if !launched.agent.alive() {
                    going.push((launched.name.clone(), "o processo dele acabou".to_owned()));
                    if !seen.contains_key(&launched.name) {
                        let key =
                            launched.template.clone().or_else(|| launched.build.clone()).unwrap_or_default();
                        failed.push(key);
                    }
                    launched.agent.stop();
                    return false;
                }
                if !seen.contains_key(&launched.name) && now - launched.since_ms > TOKEN_MS {
                    going.push((launched.name.clone(), "não conectou a tempo".to_owned()));
                    launched.agent.stop();
                    return false;
                }
                let Some(template) = launched.template.as_ref().and_then(|t| templates.get(t)) else {
                    return true;
                };
                if idle && connected {
                    let since = *launched.idle_since_ms.get_or_insert(now);
                    if now - since > i64::try_from(template.idle_ms).unwrap_or(i64::MAX) {
                        going.push((launched.name.clone(), "ficou ocioso".to_owned()));
                        launched.agent.stop();
                        return false;
                    }
                } else {
                    launched.idle_since_ms = None;
                }
                true
            });
            for key in failed {
                state.backoff.insert(key, now + BACKOFF_MS);
            }
        }
        for (name, why) in going {
            self.log.write(&format!("agente {name} parado: {why}"));
            let _ = self.remove_agent(&name);
        }
    }

    /// Takes down every agent the server brought up: what it does before it stops.
    pub fn stop_launched_agents(&self) {
        let launched = std::mem::take(&mut self.agents.state().launched);
        for launched in launched {
            launched.agent.stop();
        }
    }

    /// Looks at the agents every `every`, on a thread of its own, until [`AgentWatch::stop`].
    pub fn watch_agents(self: &Arc<Self>, every: Duration) -> AgentWatch {
        let stop = Arc::new(AtomicBool::new(false));
        let (ci, stopping) = (self.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("raven-ci-agentes".to_owned())
            .spawn(move || {
                while !stopping.load(Ordering::SeqCst) {
                    ci.tick_agents();
                    let mut slept = Duration::ZERO;
                    while slept < every && !stopping.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(50));
                        slept += Duration::from_millis(50);
                    }
                }
            })
            .ok();
        AgentWatch { stop, thread }
    }
}

/// The name of the agent a launch is for: what the core put in its settings.
pub fn launch_name(request: &LaunchRequest) -> String {
    request.settings.get("name").and_then(Value::as_str).unwrap_or("agente").to_owned()
}

/// A text setting of a launch.
pub fn launch_text<'r>(request: &'r LaunchRequest, key: &str) -> Option<&'r str> {
    request.settings.get(key).and_then(Value::as_str).filter(|text| !text.is_empty())
}

/// The contract of the agent a launcher of the agent's own program starts for `request`, with its
/// folder `dir` where it runs: [`AgentArgs`] with the name and executors the core put in the
/// settings. Its words come from [`AgentArgs::to_args`] and the token from [`AgentArgs::env`].
pub fn launch_args(request: &LaunchRequest, dir: &str) -> AgentArgs {
    let executors = request.settings.get("executors").and_then(Value::as_u64).unwrap_or(1);
    AgentArgs::from_request(request, &launch_name(request), executors, dir)
}

/// The words of the limits a template gives its agents (`nucleos`, `memoria`, `disco`), which
/// `raven-ci-agent` takes after the words of its contract.
pub fn limit_arguments(request: &LaunchRequest) -> Vec<String> {
    let mut args = Vec::new();
    for (key, flag) in [("nucleos", "--nucleos"), ("memoria", "--memoria"), ("disco", "--disco")] {
        let value = match request.settings.get(key) {
            Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        };
        if let Some(value) = value {
            args.push(flag.to_owned());
            args.push(value);
        }
    }
    args
}

/// An agent that is a program this server started: the agent itself (`agent-local`) or what
/// reaches it on another machine (`ssh`, for `agent-remote`). What it writes goes to `log`, a line
/// at a time; [`LaunchedAgent::stop`] stops it with what it started.
pub struct ChildAgent {
    name: String,
    control: ravenrs_process::Control,
    alive: Arc<AtomicBool>,
    at_stop: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

impl std::fmt::Debug for ChildAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildAgent").field("name", &self.name).finish_non_exhaustive()
    }
}

impl ChildAgent {
    /// Starts `spec` as the agent `name`, telling `log` each line it writes.
    pub fn start(
        name: &str,
        spec: ravenrs_process::Spec,
        log: impl Fn(&str) + Send + 'static,
    ) -> Result<Self, CiError> {
        let running = spec.spawn().map_err(|e| CiError::failed(format!("subir o agente {name}: {e}")))?;
        let control = running.control();
        let alive = Arc::new(AtomicBool::new(true));
        let (watching, who) = (alive.clone(), name.to_owned());
        std::thread::Builder::new()
            .name(format!("raven-ci-agente-{name}"))
            .spawn(move || {
                for event in running.events() {
                    match event {
                        ravenrs_process::Event::Out(line) | ravenrs_process::Event::Err(line) => {
                            log(&format!("[{who}] {line}"));
                        }
                        ravenrs_process::Event::Ended(end) => log(&format!("[{who}] terminou: {end:?}")),
                    }
                }
                watching.store(false, Ordering::SeqCst);
            })
            .map_err(|e| CiError::failed(e.to_string()))?;
        Ok(Self { name: name.to_owned(), control, alive, at_stop: Mutex::default() })
    }

    /// Writes `line` to its input, when it was started with one: a token it reads there.
    pub fn send(&self, line: &str) -> bool {
        self.control.send(line)
    }

    /// Runs `undo` once it stops: a key file removed, say.
    pub fn at_stop(self, undo: impl FnOnce() + Send + 'static) -> Self {
        self.at_stop.lock().unwrap_or_else(PoisonError::into_inner).push(Box::new(undo));
        self
    }
}

impl LaunchedAgent for ChildAgent {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Stops the agent and waits, a little past the grace of the polite stop, for it to be gone
    /// (#908): a server stopped by its service manager that only asked would leave its agents
    /// running on their own once it ends.
    fn stop(&self) {
        self.control.stop();
        let until = Instant::now() + ravenrs_process::GRACE + Duration::from_secs(1);
        while self.alive.load(Ordering::SeqCst) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        for undo in std::mem::take(&mut *self.at_stop.lock().unwrap_or_else(PoisonError::into_inner)) {
            undo();
        }
    }
}

/// The start of the reason of a build whose agent went quiet past its grace: what the model reads
/// as [`BuildStatus::Interrupted`].
pub use crate::model::INTERRUPTED;

/// The settings a launcher gets: the template's, with the agent's name and executors.
fn with_agent(settings: &Value, name: &str, executors: u32) -> Value {
    let mut settings = match settings {
        Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    settings.insert("name".to_owned(), json!(name));
    settings.entry("executors").or_insert(json!(executors));
    Value::Object(settings)
}

/// What a failed launch for `row` waits by: its template, or its run for a launcher of one build.
fn launch_key(row: &BuildRow, wanted: &Use, templates: &[AgentTemplate]) -> String {
    if wanted.id == "label" {
        let label = wanted.settings.get("label").and_then(Value::as_str).unwrap_or_default();
        if let Some(template) = templates.iter().find(|t| t.labels.iter().any(|l| l == label)) {
            return template.name.clone();
        }
    }
    row.run_id.clone()
}

/// Whether a build names an agent, so the server's own executors leave it.
fn needs_agent(row: &BuildRow) -> bool {
    Extras::from_value(&row.extras).agent.is_some()
}

/// Whether an agent with `labels` may take the build of `row`.
fn may_take(row: &BuildRow, labels: &[String]) -> bool {
    match Extras::from_value(&row.extras).agent {
        None => true,
        Some(wanted) if wanted.id == "label" => {
            let label = wanted.settings.get("label").and_then(Value::as_str).unwrap_or_default();
            labels.iter().any(|l| l == label)
        }
        Some(_) => labels.iter().any(|l| *l == build_label(&row.job, row.number)),
    }
}

/// The field a refusal of access is said in, so the routes answer it with 403.
const ACCESS: &str = "acesso";

/// A refusal of access: a token or a run that is not the agent's.
fn denied(why: impl Into<String>) -> CiError {
    CiError::settings(ACCESS, why)
}

fn db_error(e: rusqlite::Error) -> CiError {
    db(e)
}

/// The thread that looks at the agents. Dropping it does not stop it; [`stop`](Self::stop) does.
#[derive(Debug)]
pub struct AgentWatch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AgentWatch {
    /// Stops looking, and waits for the look that runs.
    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread {
            let _ = thread.join();
        }
    }
}

// ----- The routes -----

fn reply(status: u16, value: &Value) -> ApiResponse {
    ApiResponse::json(status, value.clone())
}

fn refused(e: &CiError) -> ApiResponse {
    let status = match e {
        CiError::Unavailable(_) => 404,
        CiError::Settings { field, .. } if field == ACCESS => 403,
        CiError::Settings { .. } => 400,
        CiError::Denied { .. } => 403,
        _ => 500,
    };
    reply(status, &json!({ "error": "ci", "message": e.to_string() }))
}

fn parsed<T: for<'de> Deserialize<'de>>(request: &ApiRequest) -> Result<T, CiError> {
    serde_json::from_slice(&request.body)
        .map_err(|e| CiError::settings("corpo", format!("não deu para ler: {e}")))
}

fn answered<T: Serialize>(result: Result<T, CiError>) -> ApiResponse {
    match result {
        Ok(value) => reply(200, &serde_json::to_value(value).unwrap_or(Value::Null)),
        Err(e) => refused(&e),
    }
}

#[derive(Deserialize)]
struct RunOnly {
    run: String,
}

#[derive(Deserialize)]
struct StepsBody {
    run: String,
    #[serde(default)]
    records: Vec<StepRecord>,
}

#[derive(Deserialize)]
struct CodeBody {
    run: String,
    commit: Option<String>,
}

#[derive(Deserialize)]
struct OutcomeBody {
    run: String,
    outcome: RunOutcome,
}

#[derive(Deserialize)]
struct SecretBody {
    name: String,
}

/// The path of the route that lets an agent in with no key.
pub const JOIN_PATH: &str = "/api/ci/agentes/entrar";

/// What the routes of agents answer `request`, or `None` for a path that is not theirs.
pub(crate) fn answer(
    ci: &Ci,
    request: &ApiRequest,
    path: &[&str],
    who: &mut dyn FnMut() -> Option<Caller>,
) -> Option<ApiResponse> {
    if path == ["api", "ci", "agents"] && request.method == "GET" {
        return Some(match who() {
            None => reply(401, &json!({ "error": "unauthorized", "message": "Entre para ver isto." })),
            Some(_) => answered(ci.agents()),
        });
    }
    let ["api", "ci", "agentes", what] = path else { return None };
    if request.method != "POST" {
        return None;
    }
    if *what == "entrar" {
        return Some(
            answered(parsed::<Join>(request).and_then(|join| ci.agent_join(&join))).map_status_denied(),
        );
    }
    let Some(caller) = who() else {
        return Some(reply(
            401,
            &json!({ "error": "unauthorized", "message": "O agente apresenta a chave dele." }),
        ));
    };
    let Some(name) = agent_of(&caller) else {
        return Some(reply(403, &json!({ "error": "denied", "message": "só agentes usam estas rotas" })));
    };
    if let Err(e) = ci.known(&name) {
        return Some(refused(&e));
    }
    Some(match *what {
        "oi" => answered(parsed::<Hello>(request).and_then(|hello| ci.agent_hello(&name, &hello))),
        "pegar" => answered(ci.agent_take(&name)),
        "passos" => {
            answered(parsed::<StepsBody>(request).and_then(|b| ci.agent_steps(&name, &b.run, &b.records)))
        }
        "log" => answered(parsed::<LogBatch>(request).and_then(|b| ci.agent_log(&name, &b))),
        "codigo" => answered(
            parsed::<CodeBody>(request).and_then(|b| ci.agent_code(&name, &b.run, b.commit.as_deref())),
        ),
        "anexo" => {
            let query = |key: &str| request.query.get(key).cloned().unwrap_or_default();
            let attachment = Attachment {
                kind: query("tipo"),
                name: query("nome"),
                mime: query("mime"),
                bytes: request.body.clone(),
                summary: serde_json::from_str(&query("resumo")).unwrap_or(Value::Null),
            };
            answered(ci.agent_attach(&name, &query("run"), &attachment, query("ultima") == "1"))
        }
        "desfecho" => {
            answered(parsed::<OutcomeBody>(request).and_then(|b| ci.agent_outcome(&name, &b.run, &b.outcome)))
        }
        "fim" => answered(parsed::<End>(request).and_then(|end| ci.agent_end(&name, &end))),
        "segredo" => answered(
            parsed::<SecretBody>(request)
                .and_then(|b| ci.agent_secret(&name, &b.name))
                .map(|(value, hidden)| json!({ "value": value, "hidden": hidden })),
        ),
        "esquecer" => answered(parsed::<RunOnly>(request).map(|b| {
            if let Some(seen) = ci.agents.state().seen.get_mut(&name) {
                seen.assigned.remove(&b.run);
            }
        })),
        _ => reply(
            404,
            &json!({ "error": "not_found", "message": format!("não há a rota de agentes {what}") }),
        ),
    })
}

/// A refusal to let an agent in is a 401, as any request without a good token.
trait DeniedIsUnauthorized {
    fn map_status_denied(self) -> Self;
}

impl DeniedIsUnauthorized for ApiResponse {
    fn map_status_denied(mut self) -> Self {
        if self.status == 403 {
            self.status = 401;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_of_an_agent_is_known_by_its_user_and_its_role() {
        let mut caller = Caller::default();
        caller.name = "agente:linux-1 (agente linux-1)".into();
        caller.roles = vec![ROLE_AGENT.into()];
        assert_eq!(agent_of(&caller).as_deref(), Some("linux-1"));
        let mut person = Caller::default();
        person.name = "agente:linux-1".into();
        person.roles = vec!["ci-leitura".into()];
        assert_eq!(agent_of(&person), None, "the name is not enough without the role");
    }

    #[test]
    fn names_of_agents_are_plain() {
        assert!(check_name("linux-1.local_2").is_ok());
        assert!(check_name("").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name("um agente").is_err());
    }

    #[test]
    fn the_settings_of_a_launch_carry_the_agent() {
        let settings = with_agent(&json!({ "usuario": "ci", "executors": 3 }), "local-1", 1);
        assert_eq!(settings, json!({ "usuario": "ci", "executors": 3, "name": "local-1" }));
    }
}
