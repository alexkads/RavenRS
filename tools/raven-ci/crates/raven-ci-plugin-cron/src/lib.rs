//! The plugin `cron` of Raven CI: builds on a schedule, and when a branch moves.
//!
//! # Its role
//!
//! Some builds are asked for by time (every night at three, every hour) and some by a change in a
//! repository that sends no webhook, or whose webhook is lost. The multibranch job of the reference
//! Jenkins polls GitHub every five minutes for that. This plugin brings both, as two [`Trigger`]s.
//!
//! # How it works
//!
//! - **`cron`**: `{"id": "cron", "schedule": "0 3 * * *", "zone": "America/Sao_Paulo"}` (the zone of
//!   the machine when none is said), or `{"id": "cron", "every": "1h"}`. When it comes due, a build
//!   is asked for with the cause "agenda 0 3 * * *". A run missed while the server was stopped runs
//!   once, if it was due less than an hour ago.
//! - **`poll`**: `{"id": "poll", "url": "https://…", "branch": "main", "every": "5m",
//!   "credential": "github"}`. Each time it reads the commit of the branch with `git ls-remote`
//!   (through `ravenrs-git`, with the credential as a header), and asks for a build with the cause
//!   "mudança em main: abc1234" when it changed, or on its first look.
//! - **`scan`**: `{"id": "scan", "every": "5m"}` on a job of many branches: each time, the core
//!   looks again at its branches and pull requests, and asks for the builds of what moved.
//! - **One thread** looks at every job's triggers each `tick_ms` (1000 unless the plugin's
//!   configuration says), and asks for builds and polls with no lock held, so a slow repository
//!   does not hold the others back. Times come from `ravenrs-schedule` (`check`, `Misfire`).
//! - **Wrong settings** (a schedule it cannot read, a poll with no `url`) are refused in `watch`,
//!   with the field and why, and the core shows it on the job.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use raven_ci_plugin_api::{BuildRequest, Builds, CiError, CiPlugin, Host, Permission, Trigger};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest, Rule};
use ravenrs_git::{Credential, Git};
use ravenrs_schedule::{Check, Misfire, SignedDuration, Timestamp, Trigger as When, check, parse_period};
use serde_json::Value;

/// How often the thread looks, unless the configuration says.
pub const DEFAULT_TICK_MS: u64 = 1000;
/// How often a poll looks, unless its settings say.
pub const DEFAULT_POLL: &str = "5m";

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a poll reads.
#[derive(Clone)]
struct Poll {
    url: String,
    branch: String,
    credential: Option<String>,
    /// The commit seen last time.
    last: Option<String>,
}

/// One trigger of one job.
struct Watch {
    /// Which of the two: `cron` or `poll`.
    kind: &'static str,
    when: When,
    misfire: Misfire,
    next: Option<Timestamp>,
    builds: Arc<dyn Builds>,
    /// The cause of the builds a schedule asks for.
    cause: String,
    poll: Option<Poll>,
    /// It asks for a scan of a job of many branches, not for a build.
    scan: bool,
}

#[derive(Default)]
struct State {
    host: Mutex<Option<Arc<dyn Host>>>,
    watches: Mutex<HashMap<String, Vec<Watch>>>,
    ticker: Mutex<Option<(Arc<AtomicBool>, JoinHandle<()>)>>,
}

/// The plugin `cron`.
#[derive(Default)]
pub struct CronPlugin {
    state: Arc<State>,
}

impl std::fmt::Debug for CronPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CronPlugin").finish_non_exhaustive()
    }
}

impl CronPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for CronPlugin {
    fn name(&self) -> &str {
        "cron"
    }

    fn manifest(&self) -> ModuleManifest {
        let mut manifest = ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default();
        for permission in [Permission::Process, Permission::Network, Permission::Secrets] {
            manifest = manifest.capability(permission.capability());
        }
        manifest
    }
}

impl CiPlugin for CronPlugin {
    fn summary(&self) -> String {
        "Builds por agenda (cron ou a cada período) e quando uma branch muda (consulta ao repositório)."
            .to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(
            EntityInfo::new("CronSettings")
                .field(FieldInfo::new("tick_ms", FieldKind::Integer))
                .rule(Rule::range("tick_ms", Some(10.0), None)),
        )
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o git ls-remote das consultas")?;
        let tick = host.settings().get("tick_ms").and_then(Value::as_u64).unwrap_or(DEFAULT_TICK_MS);
        *lock(&self.state.host) = Some(host);
        let stop = Arc::new(AtomicBool::new(false));
        let (state, stopping) = (self.state.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("raven-ci-cron".to_owned())
            .spawn(move || {
                while !stopping.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(tick));
                    if !stopping.load(Ordering::SeqCst) {
                        pass(&state, Timestamp::now());
                    }
                }
            })
            .map_err(|e| CiError::failed(format!("a thread do cron: {e}")))?;
        *lock(&self.state.ticker) = Some((stop, thread));
        Ok(())
    }

    fn stop(&self) {
        if let Some((stop, thread)) = lock(&self.state.ticker).take() {
            stop.store(true, Ordering::SeqCst);
            let _ = thread.join();
        }
        lock(&self.state.watches).clear();
        lock(&self.state.host).take();
    }

    fn triggers(&self) -> Vec<Arc<dyn Trigger>> {
        ["cron", "poll", "scan"]
            .into_iter()
            .map(|kind| Arc::new(Kind { kind, state: self.state.clone() }) as Arc<dyn Trigger>)
            .collect()
    }
}

/// The trigger `cron`, `poll` or `scan`.
struct Kind {
    kind: &'static str,
    state: Arc<State>,
}

fn text<'v>(settings: &'v Value, field: &str) -> Option<&'v str> {
    settings.get(field).and_then(Value::as_str).filter(|t| !t.trim().is_empty())
}

fn period(settings: &Value, default: Option<&str>) -> Result<Option<SignedDuration>, CiError> {
    match text(settings, "every").or(default) {
        Some(every) => parse_period(every).map(Some).map_err(|why| CiError::settings("every", why)),
        None => Ok(None),
    }
}

impl Kind {
    fn cron(&self, settings: &Value, builds: Arc<dyn Builds>) -> Result<Watch, CiError> {
        let (when, cause) = match (text(settings, "schedule"), period(settings, None)?) {
            (Some(schedule), None) => {
                let when = match text(settings, "zone") {
                    Some(zone) => When::cron(schedule, zone),
                    None => When::cron_local(schedule),
                }
                .map_err(|why| CiError::settings("schedule", format!("{schedule:?} não serve: {why}")))?;
                (when, format!("agenda {schedule}"))
            }
            (None, Some(every)) => {
                let when =
                    When::every(every, Timestamp::now()).map_err(|why| CiError::settings("every", why))?;
                (when, format!("agenda a cada {every:#}"))
            }
            _ => {
                return Err(CiError::settings(
                    "schedule",
                    "diga schedule (cron) ou every (um período), um só",
                ));
            }
        };
        let misfire = Misfire::Within(SignedDuration::from_hours(1));
        Ok(Watch { kind: "cron", when, misfire, next: None, builds, cause, poll: None, scan: false })
    }

    fn poll(&self, settings: &Value, builds: Arc<dyn Builds>) -> Result<Watch, CiError> {
        let url = text(settings, "url")
            .ok_or_else(|| CiError::settings("url", "a consulta precisa do endereço do repositório"))?;
        let every = period(settings, Some(DEFAULT_POLL))?.unwrap_or(SignedDuration::from_mins(5));
        let when = When::every(every, Timestamp::now()).map_err(|why| CiError::settings("every", why))?;
        let poll = Poll {
            url: url.to_owned(),
            branch: text(settings, "branch").unwrap_or("main").to_owned(),
            credential: text(settings, "credential").map(str::to_owned),
            last: None,
        };
        let cause = format!("consulta a {url}");
        Ok(Watch {
            kind: "poll",
            when,
            misfire: Misfire::Always,
            next: None,
            builds,
            cause,
            poll: Some(poll),
            scan: false,
        })
    }
}

impl Kind {
    fn scan(&self, settings: &Value, builds: Arc<dyn Builds>) -> Result<Watch, CiError> {
        let every = period(settings, Some(DEFAULT_POLL))?.unwrap_or(SignedDuration::from_mins(5));
        let when = When::every(every, Timestamp::now()).map_err(|why| CiError::settings("every", why))?;
        let cause = format!("varredura a cada {every:#}");
        Ok(Watch {
            kind: "scan",
            when,
            misfire: Misfire::Always,
            next: None,
            builds,
            cause,
            poll: None,
            scan: true,
        })
    }
}

impl Trigger for Kind {
    fn id(&self) -> &str {
        self.kind
    }

    fn watch(&self, job: &str, settings: &Value, builds: Arc<dyn Builds>) -> Result<(), CiError> {
        let watch = match self.kind {
            "cron" => self.cron(settings, builds)?,
            "poll" => self.poll(settings, builds)?,
            _ => self.scan(settings, builds)?,
        };
        lock(&self.state.watches).entry(job.to_owned()).or_default().push(watch);
        Ok(())
    }

    fn unwatch(&self, job: &str) {
        let mut watches = lock(&self.state.watches);
        if let Some(list) = watches.get_mut(job) {
            list.retain(|watch| watch.kind != self.kind);
            if list.is_empty() {
                watches.remove(job);
            }
        }
    }
}

/// What a pass decided to do for one trigger, done with no lock held.
enum Due {
    Ask { builds: Arc<dyn Builds>, job: String, cause: String },
    Scan { builds: Arc<dyn Builds>, job: String },
    Poll { builds: Arc<dyn Builds>, job: String, index: usize, poll: Poll },
}

/// One look at every trigger, at `now`.
fn pass(state: &State, now: Timestamp) {
    let host = lock(&state.host).clone();
    let mut due = Vec::new();
    {
        let mut watches = lock(&state.watches);
        for (job, list) in watches.iter_mut() {
            for (index, watch) in list.iter_mut().enumerate() {
                let decided = check(&watch.when, watch.misfire, watch.next, now);
                let runs = match decided {
                    Check::Seeded { next } => {
                        watch.next = next;
                        // A poll and a scan look at once; a schedule waits for its first time.
                        watch.poll.is_some() || watch.scan
                    }
                    Check::Run { next, .. } => {
                        watch.next = next;
                        true
                    }
                    Check::Missed { due: missed, next } => {
                        watch.next = next;
                        if let Some(host) = &host {
                            host.log(&format!(
                                "{job}: a agenda de {missed} passou com o servidor parado e foi pulada"
                            ));
                        }
                        false
                    }
                    Check::Wait { .. } | Check::Never => false,
                };
                if !runs {
                    continue;
                }
                let builds = watch.builds.clone();
                due.push(match &watch.poll {
                    Some(poll) => Due::Poll { builds, job: job.clone(), index, poll: poll.clone() },
                    None if watch.scan => Due::Scan { builds, job: job.clone() },
                    None => Due::Ask { builds, job: job.clone(), cause: watch.cause.clone() },
                });
            }
        }
    }
    for one in due {
        match one {
            Due::Ask { builds, job, cause } => ask(&builds, &job, cause, host.as_deref()),
            Due::Scan { builds, job } => {
                if let Err(why) = builds.scan(&job)
                    && let Some(host) = &host
                {
                    host.log(&format!("{job}: a varredura falhou: {why}"));
                }
            }
            Due::Poll { builds, job, index, poll } => match head_of(&poll, host.as_deref()) {
                Ok(commit) if poll.last.as_deref() != Some(commit.as_str()) => {
                    let short: String = commit.chars().take(7).collect();
                    ask(&builds, &job, format!("mudança em {}: {short}", poll.branch), host.as_deref());
                    if let Some(watch) =
                        lock(&state.watches).get_mut(&job).and_then(|list| list.get_mut(index))
                        && let Some(kept) = &mut watch.poll
                    {
                        kept.last = Some(commit);
                    }
                }
                Ok(_) => {}
                Err(why) => {
                    if let Some(host) = &host {
                        host.log(&format!("{job}: a consulta a {} falhou: {why}", poll.url));
                    }
                }
            },
        }
    }
}

fn ask(builds: &Arc<dyn Builds>, job: &str, cause: String, host: Option<&dyn Host>) {
    if let Err(why) = builds.request(BuildRequest::new(job, cause))
        && let Some(host) = host
    {
        host.log(&format!("{job}: o build não pôde ser pedido: {why}"));
    }
}

/// The commit at the tip of the branch the poll reads.
fn head_of(poll: &Poll, host: Option<&dyn Host>) -> Result<String, CiError> {
    let mut git = Git::at(std::env::temp_dir());
    if let Some(name) = &poll.credential {
        let host = host.ok_or_else(|| CiError::unavailable("o plugin cron não está ligado"))?;
        let value = host.secret(name)?;
        git = git.with_credential(match value.split_once(':') {
            Some((user, token)) => Credential::basic(user, token),
            None => Credential::basic("raven-ci", &value),
        });
    }
    let wanted = format!("refs/heads/{}", poll.branch);
    let refs = git.ls_remote(&poll.url, &[wanted.as_str()]).map_err(|e| CiError::failed(e.to_string()))?;
    refs.into_iter()
        .find(|(_, name)| *name == wanted)
        .map(|(commit, _)| commit)
        .ok_or_else(|| CiError::unavailable(format!("não há a branch {} em {}", poll.branch, poll.url)))
}
