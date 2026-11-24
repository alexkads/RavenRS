//! The build agent of Raven CI: `raven-ci-agent`.
//!
//! # Its role
//!
//! The server keeps the queue, the log, the attachments and the status of every build; the agent
//! runs builds, on another machine or as another user, so the server does not compile and a build
//! does not read the server's files. It connects out to the server: nothing has to be open where
//! it runs. It brings compiled in the plugins of steps, of fetching code and of hooks (`shell`,
//! `git`, `lock`, `workspace-cleanup`), and the external ones of its folder `plugins/`, all switched
//! off: each build switches on only those it needs, with the settings the server has for them.
//!
//! # How it works
//!
//! 1. **It gets in.** The first time, with the token used once the server gave
//!    (`raven-ci agentes token NOME`, or the launcher that brought it up), it asks
//!    `POST /api/ci/agentes/entrar` and keeps the key it gets in its folder (`chave`, read only by
//!    its owner). After that it comes back with the key, even after it falls.
//! 2. **It says hello** every few seconds: its labels, its executors and how many are free, the
//!    builds it runs, its memory. The server renews the leases of those builds and says which to
//!    cancel or forget.
//! 3. **It hears the doorbell** on the topic `agente-<name>` of the server's live channel (SSE,
//!    through `ravenrs-app-remote`, which comes back by itself when the stream is lost).
//! 4. **Each free executor takes a build** (`pegar`): first one of its own it lost when it fell, else
//!    the one due first that its labels allow. It runs the build's life
//!    ([`Ci::run_order`](raven_ci_core::Ci::run_order)) with an [`AgentSite`]: the steps here, from
//!    the checkpoint of the queue; the log, the steps, the attachments and the end to the server.
//!
//! # How to use it
//!
//! ```text
//! raven-ci agentes token linux-1 --rotulos linux,x64        # on the server, prints the token
//! RAVEN_CI_AGENT_TOKEN=rva_… raven-ci-agent --servidor https://ci.local \
//!     --nome linux-1 --rotulos linux,x64 --executores 2 --pasta /var/lib/raven-ci-agente
//! ```

pub mod limits;
pub mod link;
pub mod plugins;
pub mod site;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_core::agents::{HELLO_MS, Hello, HelloAnswer, Join, Joined, Task, topic_of};
use raven_ci_core::{Ci, CiOptions};
use raven_ci_plugin_api::AgentArgs;
use ravenrs_flows::CancelToken;
use serde_json::json;

pub use crate::limits::Limits;
use crate::link::{Answer, Link};
use crate::plugins::Plugins;
pub use crate::site::AgentSite;
use crate::site::RemoteSecrets;

pub use raven_ci_plugin_api::TOKEN_VARIABLE;

/// The file, in the agent's folder, that keeps its key.
pub const KEY_FILE: &str = "chave";

/// How an agent runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The server's address: `https://ci.local`.
    pub server: String,
    /// The agent's name.
    pub name: String,
    /// The token used once, for the first time.
    pub token: Option<String>,
    /// Its folder: the key, the builds' folders, its plugins.
    pub dir: PathBuf,
    /// Its labels.
    pub labels: Vec<String>,
    /// How many builds it runs at once.
    pub executors: u32,
    /// Its limits.
    pub limits: Limits,
    /// The settings of plugins that are its own, by plugin (`--plugins JSON`, from its template),
    /// over what the server has for them (#930).
    pub plugins: serde_json::Map<String, serde_json::Value>,
}

/// The words of the contract between a launcher and the agent ([`AgentArgs`]); every other word
/// is the agent's own (its limits).
const CONTRACT: [&str; 6] = ["--servidor", "--nome", "--token", "--rotulos", "--executores", "--pasta"];

impl Config {
    /// What `args` say, with the variables of `env`: the words of the contract,
    /// [`AgentArgs::from_args`] (`--servidor URL --nome NOME --pasta DIR [--rotulos a,b]
    /// [--executores N]`, and the token in `--token T`, `--token -` for the first line of the input,
    /// or [`TOKEN_VARIABLE`]), then the agent's own words, its limits ([`Limits::from_args`]). The
    /// words of the limits are taken apart first: the contract stays strict, so a word mistyped is
    /// refused, and the limits are the agent's own business (in a container or a pod, the container
    /// limits). The token is needed only the first time: an agent with no token and no key kept in
    /// its folder is refused when it starts.
    pub fn from_args(args: &[String], env: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let (mut contract, mut own) = (Vec::new(), Vec::new());
        let mut plugins = serde_json::Map::new();
        let mut words = args.iter();
        while let Some(word) = words.next() {
            if word == "--plugins" {
                let text = words.next().ok_or("--plugins precisa de um JSON")?;
                match serde_json::from_str(text) {
                    Ok(serde_json::Value::Object(map)) => plugins = map,
                    _ => return Err(format!("--plugins é um objeto JSON, por plugin: {text}")),
                }
                continue;
            }
            let to = if CONTRACT.contains(&word.as_str()) { &mut contract } else { &mut own };
            to.push(word.clone());
            if let Some(value) = words.next() {
                to.push(value.clone());
            }
        }
        if !contract.iter().any(|w| w == "--pasta") {
            contract.extend(["--pasta".to_owned(), "raven-ci-agente".to_owned()]);
        }
        let parsed = AgentArgs::from_args(&contract, env).map_err(|e| e.to_string())?;
        let token = match parsed.token.as_deref() {
            // From the first line of the input: what a launcher through SSH sends, so the token
            // shows in no list of processes on either machine.
            Some("-") => {
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).map_err(|e| format!("o token na entrada: {e}"))?;
                Some(line.trim().to_owned()).filter(|t| !t.is_empty())
            }
            other => other.map(str::to_owned),
        };
        let limits = Limits::from_args(&own)?;
        let executors = limits.executors(u32::try_from(parsed.executors).unwrap_or(u32::MAX));
        Ok(Self {
            server: parsed.server,
            name: parsed.name,
            token,
            dir: PathBuf::from(parsed.folder),
            labels: parsed.labels,
            executors,
            limits,
            plugins,
        })
    }
}

/// Rings the executors: there may be a build.
#[derive(Debug, Default)]
struct Doorbell {
    rung: Mutex<bool>,
    heard: Condvar,
}

impl Doorbell {
    fn ring(&self) {
        *self.rung.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.heard.notify_all();
    }

    /// Waits for a ring or `within`.
    fn wait(&self, within: Duration) {
        let rung = self.rung.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut rung, _) = self
            .heard
            .wait_timeout_while(rung, within, |rung| !*rung)
            .unwrap_or_else(PoisonError::into_inner);
        *rung = false;
    }
}

/// A running agent: what its threads share.
struct Agent {
    config: Config,
    link: Arc<Link>,
    ci: Arc<Ci>,
    plugins: Plugins,
    running: Mutex<HashMap<String, Arc<AgentSite>>>,
    /// The folders of builds a life before left, by run: given back or forgotten.
    left: Mutex<HashMap<String, PathBuf>>,
    watching: Arc<limits::Watching>,
    doorbell: Arc<Doorbell>,
    stopping: Arc<AtomicBool>,
}

fn say(name: &str, text: &str) {
    eprintln!("raven-ci-agent {name}: {text}");
}

/// Reads the key the agent keeps in `dir`.
fn read_key(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(KEY_FILE))
        .ok()
        .map(|key| key.trim().to_owned())
        .filter(|k| !k.is_empty())
}

/// Keeps `key` in `dir`, read only by the agent's user where the system says so.
fn write_key(dir: &Path, key: &str) -> Result<(), String> {
    let file = dir.join(KEY_FILE);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut out = options.open(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    std::io::Write::write_all(&mut out, key.as_bytes()).map_err(|e| format!("{}: {e}", file.display()))
}

impl Agent {
    fn hello(&self) -> Hello {
        let running: Vec<String> =
            self.running.lock().unwrap_or_else(PoisonError::into_inner).keys().cloned().collect();
        let busy = u32::try_from(running.len()).unwrap_or(u32::MAX);
        Hello {
            labels: self.config.labels.clone(),
            executors: self.config.executors,
            free: self.config.executors.saturating_sub(busy),
            running,
            left: self.left.lock().unwrap_or_else(PoisonError::into_inner).keys().cloned().collect(),
            memory_bytes: limits::memory_now(),
            limits: self.config.limits.report(&self.config.dir),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            session: self.link.session.clone(),
        }
    }

    /// Gets in with the token, keeping the key; or comes back with the key it kept.
    fn get_in(&self) -> Result<(), String> {
        if let Some(key) = read_key(&self.config.dir) {
            self.link.use_key(&key);
            return Ok(());
        }
        self.join()
    }

    fn join(&self) -> Result<(), String> {
        let token = self.config.token.clone().ok_or_else(|| {
            format!(
                "o agente não tem chave em {} e não veio com token ({TOKEN_VARIABLE} ou --token)",
                self.config.dir.display()
            )
        })?;
        let join = Join { name: self.config.name.clone(), token, hello: self.hello() };
        match self.link.call::<Joined>("entrar", &join) {
            Some(Ok(joined)) => {
                write_key(&self.config.dir, &joined.key)?;
                self.link.use_key(&joined.key);
                say(&self.config.name, &format!("entrou em {}", self.link.base()));
                Ok(())
            }
            Some(Err((status, said))) => {
                Err(format!("o servidor não deixou o agente entrar ({status}): {said}"))
            }
            None => Err("o agente parou antes de entrar".to_owned()),
        }
    }

    /// Says hello once and does what the server answers.
    fn say_hello(&self) -> Result<(), String> {
        match self.link.post("oi", &self.hello()) {
            Answer::Ok(value) => {
                let answer: HelloAnswer = serde_json::from_value(value).unwrap_or_default();
                let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
                for cancel in answer.cancel {
                    if let Some(site) = running.get(&cancel.run) {
                        site.cancel_steps(&cancel.reason);
                    }
                }
                for other in &answer.leftovers {
                    if let Some(said) = remove_leftover(&self.config.dir, other) {
                        say(&self.config.name, &said);
                    }
                }
                for run in answer.forget {
                    if let Some(site) = running.get(&run) {
                        site.forget("o servidor terminou o build sem este agente");
                    }
                    let left = self.left.lock().unwrap_or_else(PoisonError::into_inner).remove(&run);
                    if let Some(workspace) = left {
                        let _ = std::fs::remove_dir_all(&workspace);
                        let _ = std::fs::remove_file(workspace.with_extension("estado"));
                        say(
                            &self.config.name,
                            &format!("{}: esquecido, a pasta foi apagada", workspace.display()),
                        );
                    }
                }
                Ok(())
            }
            Answer::Refused(401 | 403, said) => {
                // A key that no longer works: removed, or the server lost it. A token gets in again.
                say(&self.config.name, &format!("a chave foi recusada: {said}"));
                let _ = std::fs::remove_file(self.config.dir.join(KEY_FILE));
                self.join()
            }
            Answer::Refused(status, said) => Err(format!("oi recusado ({status}): {said}")),
            Answer::Down(why) => {
                say(&self.config.name, &format!("o servidor não respondeu: {why}"));
                Ok(())
            }
        }
    }

    /// Takes builds while there are, as one executor.
    fn take_builds(self: &Arc<Self>) {
        while !self.stopping.load(Ordering::SeqCst) {
            let free = {
                let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
                running.len() < self.config.executors as usize
            };
            if !free || !self.config.limits.room(&self.config.dir) {
                return;
            }
            match self.link.post("pegar", &json!({})) {
                Answer::Ok(value) => match serde_json::from_value::<Option<Task>>(value) {
                    Ok(Some(task)) => self.run_task(task),
                    Ok(None) => return,
                    Err(e) => {
                        say(&self.config.name, &format!("não entendi o build que o servidor mandou: {e}"));
                        return;
                    }
                },
                Answer::Refused(status, said) => {
                    say(&self.config.name, &format!("pegar recusado ({status}): {said}"));
                    return;
                }
                Answer::Down(_) => return,
            }
        }
    }

    fn run_task(self: &Arc<Self>, task: Task) {
        let run = task.lease.id.clone();
        self.left.lock().unwrap_or_else(PoisonError::into_inner).remove(&run);
        let token = CancelToken::new();
        let environment = self.config.limits.environment();
        let site = Arc::new(AgentSite::new(
            self.ci.clone(),
            self.link.clone(),
            task.lease.clone(),
            token,
            environment,
        ));
        self.running.lock().unwrap_or_else(PoisonError::into_inner).insert(run.clone(), site.clone());
        let watch = self.config.limits.watch(
            &self.ci.workspace(&task.order.job, task.order.number),
            site.clone(),
            &self.watching,
        );
        let order = &task.order;
        say(&self.config.name, &format!("{} #{}", order.job, order.number));
        match self.plugins.take(&task.plugins) {
            Ok(()) => {
                if let Err(e) = self.ci.run_order(order, site.as_ref()) {
                    say(&self.config.name, &format!("{} #{}: {e}", order.job, order.number));
                }
                self.plugins.give(&task.plugins);
            }
            Err(e) => {
                let reason = e.to_string();
                raven_ci_core::BuildSite::log(site.as_ref()).line("", &reason);
                let _ = raven_ci_core::BuildSite::fail(site.as_ref(), &reason);
            }
        }
        drop(watch);
        if site.lost() {
            let workspace = self.ci.workspace(&order.job, order.number);
            let _ = std::fs::remove_dir_all(&workspace);
            let _ = std::fs::remove_file(workspace.with_extension("estado"));
            say(
                &self.config.name,
                &format!("{} #{} esquecido: a pasta foi apagada", order.job, order.number),
            );
        }
        site.close();
        self.running.lock().unwrap_or_else(PoisonError::into_inner).remove(&run);
    }
}

/// How long a key not written makes its agent's folder free: an agent says hello every few
/// seconds and rewrites nothing, so the key is not the clock; it is the time since the folder was
/// last touched by anything (#938).
const LEFT_FOR: std::time::Duration = std::time::Duration::from_secs(120);

/// Removes `<folder of the templates>/<other>`, the folder of an agent of the same template the
/// server took down (#938), when nothing uses it: nothing under it written in the last two minutes
/// and, on Linux, no process with its directory there. What it says, when it removed it.
fn remove_leftover(own: &std::path::Path, other: &str) -> Option<String> {
    if other.is_empty() || other.contains(['/', '\\']) || other.starts_with('.') {
        return None;
    }
    let folder = own.parent()?.join(other);
    if folder == own || !folder.is_dir() || touched_lately(&folder) || in_use(&folder) {
        return None;
    }
    let bytes = size_of(&folder);
    std::fs::remove_dir_all(&folder).ok()?;
    Some(format!(
        "a pasta do agente {other} ({}) saiu: {} MB liberados",
        folder.display(),
        bytes / (1024 * 1024)
    ))
}

fn touched_lately(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let recent = |path: &std::path::Path| {
        std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < LEFT_FOR)
    };
    recent(dir)
        || entries.filter_map(Result::ok).any(|entry| {
            let path = entry.path();
            recent(&path) || (path.is_dir() && touched_lately(&path))
        })
}

/// Whether a process has its directory under `dir` (Linux, through `/proc`).
fn in_use(dir: &std::path::Path) -> bool {
    let Ok(processes) = std::fs::read_dir("/proc") else { return false };
    processes.filter_map(Result::ok).any(|p| {
        std::fs::read_link(p.path().join("cwd")).is_ok_and(|cwd| cwd.starts_with(dir))
    })
}

fn size_of(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => size_of(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod leftovers {
    use super::remove_leftover;

    #[test]
    fn a_folder_left_long_ago_goes_and_one_touched_lately_or_named_wrong_stays() {
        let base = std::env::temp_dir().join(format!("raven-ci-sobras-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let own = base.join("modelo-b");
        let old = base.join("modelo-a");
        let fresh = base.join("modelo-c");
        for dir in [&own, &old, &fresh] {
            std::fs::create_dir_all(dir.join("espacos")).expect("a folder");
            std::fs::write(dir.join("chave"), "x").expect("a key");
        }
        let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
        for path in [old.join("espacos"), old.join("chave"), old.clone()] {
            std::fs::File::options()
                .write(true)
                .open(&path)
                .or_else(|_| std::fs::File::open(&path))
                .and_then(|f| f.set_modified(long_ago))
                .expect("aged");
        }
        assert!(remove_leftover(&own, "modelo-c").is_none(), "touched in the last two minutes: it stays");
        assert!(fresh.is_dir());
        assert!(remove_leftover(&own, "../fora").is_none(), "only a name beside its own");
        assert!(remove_leftover(&own, "modelo-b").is_none(), "never its own");
        let said = remove_leftover(&own, "modelo-a").expect("removed");
        assert!(said.contains("modelo-a") && said.contains("MB liberados"), "{said}");
        assert!(!old.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// Runs the agent of `config` until `stopping` is set.
pub fn run(config: Config, stopping: Arc<AtomicBool>) -> Result<(), String> {
    std::fs::create_dir_all(&config.dir).map_err(|e| format!("{}: {e}", config.dir.display()))?;
    let dir = std::path::absolute(&config.dir).unwrap_or_else(|_| config.dir.clone());
    let config = Config { dir, ..config };
    let session = uuid::Uuid::new_v4().simple().to_string();
    let link = Arc::new(Link::new(&config.server, session, stopping.clone()));
    let ci = Arc::new(
        Ci::new(CiOptions {
            data_dir: Some(config.dir.join("dados")),
            secrets: Arc::new(RemoteSecrets::new(link.clone())),
            ..CiOptions::default()
        })
        .map_err(|e| e.to_string())?,
    );
    for plugin in plugins::builtin() {
        ci.add_plugin(plugin);
    }
    let external = config.dir.join("plugins");
    if external.is_dir() {
        ci.discover_plugins(&external);
    }
    ci.apply_plugins().map_err(|e| e.to_string())?;
    let plugins = Plugins::new(ci.clone(), config.plugins.clone()).map_err(|e| e.to_string())?;
    let limits = config.limits.apply(&config.name);
    let ci_workspaces = config.dir.join("dados").join(raven_ci_core::WORKSPACES_DIR);
    let agent = Arc::new(Agent {
        config,
        link,
        ci,
        plugins,
        running: Mutex::default(),
        left: Mutex::new(left_behind(&ci_workspaces)),
        watching: Arc::default(),
        doorbell: Arc::new(Doorbell::default()),
        stopping: stopping.clone(),
    });
    for line in limits {
        say(&agent.config.name, &line);
    }
    agent.get_in()?;
    agent.say_hello()?;
    say(&agent.config.name, &format!("pronto, com {} executor(es)", agent.config.executors));

    let mut threads = Vec::new();
    let hello = agent.clone();
    threads.push(std::thread::spawn(move || {
        while !hello.link.rest(Duration::from_millis(HELLO_MS)) {
            if let Err(e) = hello.say_hello() {
                say(&hello.config.name, &e);
            }
            hello.doorbell.ring();
        }
    }));
    let listening = agent.clone();
    threads.push(std::thread::spawn(move || listen(&listening)));
    for _ in 0..agent.config.executors {
        let executor = agent.clone();
        threads.push(std::thread::spawn(move || {
            while !executor.stopping.load(Ordering::SeqCst) {
                executor.take_builds();
                executor.doorbell.wait(Duration::from_millis(HELLO_MS));
            }
        }));
    }
    agent.doorbell.ring();
    while !stopping.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    agent.doorbell.ring();
    for thread in threads {
        let _ = thread.join();
    }
    Ok(())
}

/// The builds whose folders a life before left in `root`, by run: the notes beside the folders
/// (`<number>.estado`) say the run.
fn left_behind(root: &Path) -> HashMap<String, PathBuf> {
    fn walk(folder: &Path, found: &mut HashMap<String, PathBuf>) {
        let Ok(entries) = std::fs::read_dir(folder) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "estado")
                && let Ok(text) = std::fs::read_to_string(&path)
                && let Ok(note) = serde_json::from_str::<serde_json::Value>(&text)
                && let Some(run) = note.get("run").and_then(serde_json::Value::as_str)
            {
                found.insert(run.to_owned(), path.with_extension(""));
            }
        }
    }
    let mut found = HashMap::new();
    walk(root, &mut found);
    found
}

/// Hears the doorbell of the server's live channel until the agent stops.
fn listen(agent: &Arc<Agent>) {
    while !agent.stopping.load(Ordering::SeqCst) {
        let Some(key) = agent.link.key() else {
            agent.link.rest(Duration::from_secs(1));
            continue;
        };
        let client = ravenrs_app_remote::RemoteClient::http(agent.link.base()).with_token(key);
        let doorbell = agent.doorbell.clone();
        let feed = client.listen_with(&topic_of(&agent.config.name), None, move || doorbell.ring());
        loop {
            if agent.link.rest(Duration::from_millis(250)) {
                return;
            }
            for event in feed.drain() {
                if event.kind == "cancelar"
                    && let (Some(run), Some(reason)) = (event.data.get("run"), event.data.get("reason"))
                {
                    let running = agent.running.lock().unwrap_or_else(PoisonError::into_inner);
                    if let Some(site) = run.as_str().and_then(|run| running.get(run)) {
                        site.cancel_steps(reason.as_str().unwrap_or_default());
                    }
                }
            }
            if matches!(feed.state(), ravenrs_app_remote::LiveState::Refused(_)) {
                // Refused (a topic not open yet, a key not good yet): try again after a while.
                agent.link.rest(Duration::from_secs(5));
                break;
            }
        }
    }
}
