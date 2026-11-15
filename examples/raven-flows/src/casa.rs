//! Where the app keeps what it has, with no screen: the flows as JSON files, one per flow, and
//! the runs in the SQLite file of `ravenrs-flows-runtime`. The screens ask this module and
//! never touch the files or the database themselves.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use ravenrs::prelude::UiWaker;
use ravenrs_flows::{Catalog, FlowDefinition, Item, NodeDef, NodeRun, NodeStatus};
use ravenrs_flows_runtime::{Request, Retention, Revision, RunRecord, RunStatus, Runtime};
use serde_json::json;

/// Who asks for the runs started from the app.
const ACTOR: &str = "Raven Flows";

/// The folder of the flows inside the app's folder.
const FLOWS: &str = "fluxos";

/// The database of the runs inside the app's folder.
const RUNS: &str = "execucoes.db";

/// How the runs asked for get run.
#[derive(Debug)]
pub enum Worker {
    /// On a thread of its own, with its own connection to the database; the screen is woken
    /// when it has something new. The app's choice.
    Thread,
    /// At once, on the calling thread: what the tests use, so a run is over when the click is.
    Inline,
}

/// The app's folder and what is in it.
pub struct Casa {
    dir: PathBuf,
    runtime: Runtime,
    worker: Worker,
    /// The worker thread, once started: each message asks it to work until nothing is due.
    kick: RefCell<Option<Sender<()>>>,
}

impl std::fmt::Debug for Casa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Casa").field("dir", &self.dir).field("worker", &self.worker).finish_non_exhaustive()
    }
}

/// The milliseconds since 1970, on the computer's clock.
pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl Casa {
    /// Opens the app's folder `dir`, making it when it is not there. The first time, it gets
    /// the sample flow, so the list is not empty.
    pub fn open(dir: impl AsRef<Path>, worker: Worker) -> Result<Self, String> {
        let dir = dir.as_ref().to_path_buf();
        let flows = dir.join(FLOWS);
        let first = !flows.exists();
        fs::create_dir_all(&flows).map_err(|e| format!("Não consegui criar {}: {e}", flows.display()))?;
        let runtime = Runtime::open(dir.join(RUNS), Catalog::builtin())
            .map_err(|e| format!("Não consegui abrir as execuções: {e}"))?;
        let casa = Self { dir, runtime, worker, kick: RefCell::new(None) };
        if first {
            casa.save(&mut sample())?;
        }
        Ok(casa)
    }

    /// The app's folder, the default one: `~/.raven-flows`.
    pub fn default_dir() -> PathBuf {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
        PathBuf::from(home).join(".raven-flows")
    }

    /// Starts the worker thread, which wakes the screen through `waker` when a run moved.
    /// Called once, by the first build; does nothing with [`Worker::Inline`].
    pub fn start(&self, waker: UiWaker) {
        if !matches!(self.worker, Worker::Thread) || self.kick.borrow().is_some() {
            return;
        }
        let (send, receive) = channel::<()>();
        let path = self.dir.join(RUNS);
        let spawned = std::thread::Builder::new().name("raven-flows-worker".into()).spawn(move || {
            let runtime = match Runtime::open(&path, Catalog::builtin()) {
                Ok(runtime) => runtime,
                Err(e) => {
                    tracing::error!(error = %e, "the worker could not open the runs");
                    return;
                }
            };
            while receive.recv().is_ok() {
                // What piled up while it worked is one more round, not one per message.
                while receive.try_recv().is_ok() {}
                if let Err(e) = runtime.work_until_idle("worker") {
                    tracing::error!(error = %e, "a run could not be worked");
                }
                waker.wake();
            }
        });
        match spawned {
            Ok(_) => *self.kick.borrow_mut() = Some(send),
            Err(e) => tracing::error!(error = %e, "the worker thread did not start"),
        }
    }

    /// Asks for what is due to run now.
    fn work(&self) -> Result<(), String> {
        match self.worker {
            Worker::Inline => {
                self.runtime.work_until_idle("app").map_err(|e| format!("A execução falhou: {e}"))?;
            }
            Worker::Thread => {
                if let Some(kick) = &*self.kick.borrow() {
                    let _ = kick.send(());
                }
            }
        }
        Ok(())
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(FLOWS).join(format!("{id}.json"))
    }

    /// The flows, by name.
    pub fn flows(&self) -> Vec<FlowDefinition> {
        let Ok(entries) = fs::read_dir(self.dir.join(FLOWS)) else { return Vec::new() };
        let mut flows: Vec<FlowDefinition> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| {
                let text = fs::read_to_string(e.path()).ok()?;
                match FlowDefinition::from_json(&text) {
                    Ok(flow) => Some(flow),
                    Err(why) => {
                        tracing::warn!(file = %e.path().display(), error = %why, "a flow file could not be read");
                        None
                    }
                }
            })
            .collect();
        flows.sort_by_key(|f| f.name.to_lowercase());
        flows
    }

    /// The flow `id`, as saved.
    pub fn flow(&self, id: &str) -> Option<FlowDefinition> {
        let text = fs::read_to_string(self.file(id)).ok()?;
        FlowDefinition::from_json(&text).ok()
    }

    /// Saves `flow` as a new revision.
    pub fn save(&self, flow: &mut FlowDefinition) -> Result<(), String> {
        flow.revision += 1;
        fs::write(self.file(&flow.id), flow.to_json())
            .map_err(|e| format!("Não consegui salvar o fluxo: {e}"))
    }

    /// A new flow called `name`, with a manual start, saved.
    pub fn create(&self, name: &str) -> Result<FlowDefinition, String> {
        let taken: Vec<String> = self.flows().into_iter().map(|f| f.id).collect();
        let base = slug(name);
        let id = (1..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") })
            .find(|id| !taken.contains(id));
        let mut flow = FlowDefinition::new(id.unwrap_or(base), name)
            .node(NodeDef::new("inicio", "manual").title("Início").parameter("items", json!([{}])));
        self.save(&mut flow)?;
        Ok(flow)
    }

    /// Deletes the flow `id`; its runs stay until they are pruned.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        fs::remove_file(self.file(id)).map_err(|e| format!("Não consegui excluir o fluxo: {e}"))
    }

    /// Runs `flow` as it is now and says the run's id.
    pub fn run(&self, flow: &FlowDefinition) -> Result<String, String> {
        let id = self
            .runtime
            .enqueue(Request::new(flow.clone()).by(ACTOR))
            .map_err(|e| format!("Não consegui pedir a execução: {e}"))?;
        self.work()?;
        Ok(id)
    }

    /// Every run of every flow, the newest first.
    pub fn runs(&self) -> Vec<RunRecord> {
        let mut runs: Vec<RunRecord> =
            self.flows().iter().flat_map(|f| self.runtime.runs_of(&f.id).unwrap_or_default()).collect();
        runs.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then_with(|| b.id.cmp(&a.id)));
        runs
    }

    /// The newest run of the flow `id`.
    pub fn last_run(&self, flow_id: &str) -> Option<RunRecord> {
        self.runtime
            .runs_of(flow_id)
            .ok()?
            .into_iter()
            .max_by(|a, b| a.created_ms.cmp(&b.created_ms).then_with(|| a.id.cmp(&b.id)))
    }

    /// The run `id`.
    pub fn run_record(&self, id: &str) -> Option<RunRecord> {
        self.runtime.get(id).ok()
    }

    /// What each node did in the run `id`.
    pub fn parts(&self, id: &str) -> BTreeMap<String, NodeRun> {
        self.runtime.parts(id).unwrap_or_default()
    }

    /// The event the run `id` waits for, when it waits for one.
    pub fn waiting_for(&self, id: &str) -> Option<String> {
        self.parts(id)
            .into_values()
            .find(|part| part.status == NodeStatus::Waiting)
            .and_then(|part| part.wake.and_then(|w| w.event))
    }

    /// Says the event the run `id` waits for came, so it goes on.
    pub fn approve(&self, id: &str) -> Result<(), String> {
        let event = self.waiting_for(id).ok_or("Esta execução não espera nenhuma aprovação.")?;
        let item = Item::new(json!({ "aprovado_por": ACTOR, "em": now_ms() }));
        self.runtime.signal(id, &event, vec![item]).map_err(|e| format!("Não consegui aprovar: {e}"))?;
        self.work()
    }

    /// Runs `id` again with the same input, on the flow as it is now when there is one, and
    /// says the new run's id.
    pub fn repeat(&self, id: &str) -> Result<String, String> {
        let record = self.run_record(id).ok_or("Esta execução não existe mais.")?;
        let revision = match self.flow(&record.flow_id) {
            Some(flow) => Revision::Current(flow),
            None => Revision::Original,
        };
        let new = self.runtime.rerun(id, revision).map_err(|e| format!("Não consegui repetir: {e}"))?;
        self.work()?;
        Ok(new)
    }

    /// Forgets the ended runs older than `days`, and says how many went.
    pub fn prune(&self, days: u64) -> Result<usize, String> {
        let retention = Retention { older_than_ms: Some(days * 86_400_000), ..Retention::default() };
        self.runtime.prune(&retention).map(|p| p.runs).map_err(|e| format!("Não consegui limpar: {e}"))
    }
}

/// A file name for `name`: lowercase letters and digits joined by `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() { "fluxo".to_owned() } else { out }
}

/// What a run's status says, in words.
pub fn run_words(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "Na fila",
        RunStatus::Running => "Rodando",
        RunStatus::Waiting => "Aguardando",
        RunStatus::Finished => "Concluída",
        RunStatus::Stopped => "Parou com erro",
        RunStatus::Cancelled => "Cancelada",
        RunStatus::TimedOut => "Passou do tempo",
        _ => "Desconhecido",
    }
}

/// What a node's part in a run says, in words.
pub fn node_words(status: NodeStatus) -> &'static str {
    match status {
        NodeStatus::Done => "Concluído",
        NodeStatus::Pinned => "Dados fixados",
        NodeStatus::Disabled => "Desligado",
        NodeStatus::Skipped => "Não rodou",
        NodeStatus::Failed => "Falhou",
        NodeStatus::Reused => "Reaproveitado",
        NodeStatus::Cancelled => "Cancelado",
        NodeStatus::TimedOut => "Passou do tempo",
        NodeStatus::Uncertain => "Incerto",
        NodeStatus::Waiting => "Aguardando",
        _ => "Desconhecido",
    }
}

/// How long ago `ms` was, in words.
pub fn ago(ms: i64, now: i64) -> String {
    let minutes = (now - ms).max(0) / 60_000;
    match minutes {
        0 => "agora".to_owned(),
        1..=59 => format!("há {minutes} min"),
        60..=1439 => format!("há {} h", minutes / 60),
        _ => format!("há {} dias", minutes / 1440),
    }
}

/// The sample flow: orders that are paid and big wait for an approval, the unpaid ones are
/// marked to be charged.
pub fn sample() -> FlowDefinition {
    FlowDefinition::new("conferir-pedidos", "Conferir pedidos")
        .node(NodeDef::new("inicio", "manual").title("Pedidos do dia").parameter(
            "items",
            json!([
                { "pedido": 101, "cliente": "Ana", "valor": 80, "pago": true },
                { "pedido": 102, "cliente": "Bia", "valor": 240, "pago": true },
                { "pedido": 103, "cliente": "Caio", "valor": 510, "pago": false },
                { "pedido": 104, "cliente": "Davi", "valor": 130, "pago": true }
            ]),
        ))
        .node(NodeDef::new("pago", "if").title("Pago?").parameter("condition", json!("[pago] = true")))
        .node(
            NodeDef::new("grandes", "filter")
                .title("Acima de R$ 100")
                .parameter("condition", json!("[valor] > 100")),
        )
        .node(
            NodeDef::new("aprovar", "wait_event")
                .title("Aprovar pedidos")
                .parameter("event", json!("aprovacao"))
                .parameter("keep", json!(true))
                .parameter("as", json!("aprovacao")),
        )
        .node(
            NodeDef::new("aprovado", "set")
                .title("Marcar aprovado")
                .parameter("fields", json!({ "situacao": "'aprovado'" })),
        )
        .node(
            NodeDef::new("cobrar", "set")
                .title("Marcar para cobrar")
                .parameter("fields", json!({ "situacao": "'cobrar'" })),
        )
        .connect("inicio", "main", "pago", "main")
        .connect("pago", "true", "grandes", "main")
        .connect("grandes", "main", "aprovar", "main")
        .connect("aprovar", "main", "aprovado", "main")
        .connect("pago", "false", "cobrar", "main")
}
