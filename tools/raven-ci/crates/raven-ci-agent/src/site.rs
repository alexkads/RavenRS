//! A build as the agent runs it: the [`BuildSite`] whose steps run here and whose log, steps,
//! attachments and end go to the server.
//!
//! What has to reach the server in order (the steps, the lines of the log, the commit fetched)
//! goes through the build's [`Outbox`]: a thread of the build sends it, in batches, again and
//! again while the server does not answer, so a step never waits for the network and nothing is
//! lost while the agent lives. The server drops a line it already has by the agent's session and
//! the line's place, so sending again never repeats one.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use raven_ci_core::agents::{AgentLine, End, LogBatch, PART_BYTES};
use raven_ci_core::{BuildSite, Ci, Secrets};
use raven_ci_plugin_api::{Attachment, BuildLog, BuildStatus, CiError};
use ravenrs_flows::{CancelToken, Catalog, RunOutcome, SystemClock};
use ravenrs_flows_runtime::{Held, Lease, StepRecord};
use serde_json::{Value, json};

use crate::link::Link;

/// The most lines a build keeps waiting for the server; past it, new lines are counted and said
/// when the server answers again.
const MOST_WAITING: usize = 200_000;
/// The most lines sent at once.
const BATCH: usize = 500;

/// What waits to go to the server, in order.
#[derive(Debug)]
enum Msg {
    Step(StepRecord),
    Line(AgentLine),
    Code(Option<String>),
}

#[derive(Default)]
struct Queue {
    waiting: VecDeque<Msg>,
    /// A batch on its way: flushing waits for it too.
    sending: bool,
    closed: bool,
    dropped: u64,
}

/// What a build has waiting for the server, and the thread that sends it.
pub struct Outbox {
    queue: Mutex<Queue>,
    changed: Condvar,
    /// How many lines of this build the server has, in this session: where the next batch starts.
    lines_sent: AtomicU64,
}

impl std::fmt::Debug for Outbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outbox").field("lines_sent", &self.lines_sent).finish_non_exhaustive()
    }
}

impl Outbox {
    fn new() -> Self {
        Self { queue: Mutex::default(), changed: Condvar::new(), lines_sent: AtomicU64::new(0) }
    }

    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, msg: Msg) {
        let mut queue = self.lock();
        if queue.closed {
            return;
        }
        if matches!(msg, Msg::Line(_)) && queue.waiting.len() >= MOST_WAITING {
            queue.dropped += 1;
            return;
        }
        queue.waiting.push_back(msg);
        self.changed.notify_all();
    }

    /// Waits until everything pushed so far reached the server (or the build was lost).
    fn flush(&self) {
        let mut queue = self.lock();
        while !queue.closed && (queue.sending || !queue.waiting.is_empty()) {
            queue = self.changed.wait(queue).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn close(&self) {
        let mut queue = self.lock();
        queue.closed = true;
        queue.waiting.clear();
        self.changed.notify_all();
    }

    /// The next batch to send: steps together, lines together, or a commit.
    fn next(&self) -> Option<Vec<Msg>> {
        let mut queue = self.lock();
        loop {
            if queue.closed {
                return None;
            }
            if let Some(first) = queue.waiting.pop_front() {
                let mut batch = vec![first];
                let same = |a: &Msg, b: &Msg| {
                    matches!((a, b), (Msg::Step(_), Msg::Step(_)) | (Msg::Line(_), Msg::Line(_)))
                };
                while batch.len() < BATCH && queue.waiting.front().is_some_and(|next| same(&batch[0], next)) {
                    if let Some(next) = queue.waiting.pop_front() {
                        batch.push(next);
                    }
                }
                if queue.dropped > 0 && matches!(batch[0], Msg::Line(_)) {
                    let text = format!(
                        "{} linhas não chegaram ao servidor enquanto ele não respondia",
                        queue.dropped
                    );
                    batch.push(Msg::Line(AgentLine { step: String::new(), text }));
                    queue.dropped = 0;
                }
                queue.sending = true;
                return Some(batch);
            }
            queue = self.changed.wait(queue).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn sent(&self) {
        self.lock().sending = false;
        self.changed.notify_all();
    }
}

/// The log of a build the agent runs: each line, with what the agent knows to hide hidden, goes
/// through the outbox.
pub struct RemoteLog {
    ci: Arc<Ci>,
    outbox: Arc<Outbox>,
    hidden: Mutex<Vec<String>>,
}

impl std::fmt::Debug for RemoteLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteLog").finish_non_exhaustive()
    }
}

impl BuildLog for RemoteLog {
    fn line(&self, step: &str, text: &str) {
        let text = self.ci.log().hide(text);
        let hidden = self.hidden.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let text = hidden.iter().fold(text, |text, value| text.replace(value.as_str(), "****"));
        self.outbox.push(Msg::Line(AgentLine { step: step.to_owned(), text }));
    }

    fn hide(&self, value: &str) {
        if value.len() >= 3 {
            self.hidden.lock().unwrap_or_else(PoisonError::into_inner).push(value.to_owned());
        }
    }
}

/// A build the agent runs. See the [module](self).
pub struct AgentSite {
    link: Arc<Link>,
    run: String,
    lease: Lease,
    outbox: Arc<Outbox>,
    log: Arc<RemoteLog>,
    token: CancelToken,
    lost: Arc<AtomicBool>,
    commit: Mutex<Option<String>>,
    environment: BTreeMap<String, String>,
    sender: Mutex<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for AgentSite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentSite").field("run", &self.run).finish_non_exhaustive()
    }
}

impl AgentSite {
    /// The site of the build of `lease`, sending through `link`, its steps cancelled through
    /// `token`, giving its steps `environment` too.
    pub fn new(
        ci: Arc<Ci>,
        link: Arc<Link>,
        lease: Lease,
        token: CancelToken,
        environment: BTreeMap<String, String>,
    ) -> Self {
        let outbox = Arc::new(Outbox::new());
        let lost = Arc::new(AtomicBool::new(false));
        let run = lease.id.clone();
        let sender = {
            let (link, outbox, lost, token, run) =
                (link.clone(), outbox.clone(), lost.clone(), token.clone(), run.clone());
            std::thread::Builder::new()
                .name(format!("raven-ci-envio-{run}"))
                .spawn(move || send_all(&link, &outbox, &lost, &token, &run))
                .ok()
        };
        let log = Arc::new(RemoteLog { ci, outbox: outbox.clone(), hidden: Mutex::default() });
        Self {
            link,
            run,
            lease,
            outbox,
            log,
            token,
            lost,
            commit: Mutex::default(),
            environment,
            sender: Mutex::new(sender),
        }
    }

    /// Whether the server took the build back (it ended without this agent, or another holds it).
    pub fn lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }

    /// Marks the build as no longer this agent's and stops its steps.
    pub fn forget(&self, why: &str) {
        self.lost.store(true, Ordering::SeqCst);
        self.token.cancel(why);
        self.outbox.close();
    }

    /// Cancels its steps, saying `reason`: someone asked on the server, or a limit passed.
    pub fn cancel_steps(&self, reason: &str) {
        if !self.token.is_cancelled() {
            self.token.cancel(reason);
        }
    }

    /// Stops the thread that sends, once everything went.
    pub fn close(&self) {
        self.outbox.flush();
        self.outbox.close();
        if let Some(sender) = self.sender.lock().unwrap_or_else(PoisonError::into_inner).take() {
            let _ = sender.join();
        }
    }

    fn outcome(&self, outcome: &RunOutcome) -> BuildStatus {
        self.outbox.flush();
        if self.lost() {
            return BuildStatus::Running;
        }
        match self.link.call::<BuildStatus>("desfecho", &json!({ "run": self.run, "outcome": outcome })) {
            Some(Ok(status)) => status,
            Some(Err((_, said))) => {
                self.forget(&said);
                BuildStatus::Running
            }
            None => BuildStatus::Running,
        }
    }

    fn end(&self, status: BuildStatus) {
        let commit = self.commit.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let end = End { run: self.run.clone(), status, commit };
        if let Some(Err((_, said))) = self.link.deliver("fim", &end) {
            self.forget(&said);
        }
    }
}

/// The thread of a build that sends its outbox, in order, until it is closed.
fn send_all(link: &Link, outbox: &Outbox, lost: &AtomicBool, token: &CancelToken, run: &str) {
    while let Some(batch) = outbox.next() {
        let answered = match &batch[0] {
            Msg::Step(_) => {
                let records: Vec<&StepRecord> =
                    batch.iter().filter_map(|m| if let Msg::Step(r) = m { Some(r) } else { None }).collect();
                link.call::<Held>("passos", &json!({ "run": run, "records": records })).map(|answer| {
                    answer.map(|held| {
                        if let Some(reason) = held.cancel
                            && !token.is_cancelled()
                        {
                            token.cancel(reason);
                        }
                        held.held
                    })
                })
            }
            Msg::Line(_) => {
                let lines: Vec<AgentLine> = batch
                    .into_iter()
                    .filter_map(|m| if let Msg::Line(l) = m { Some(l) } else { None })
                    .collect();
                let count = lines.len() as u64;
                let from = outbox.lines_sent.load(Ordering::SeqCst);
                let batch = LogBatch { run: run.to_owned(), session: link.session.clone(), from, lines };
                link.deliver("log", &batch).map(|answer| {
                    answer.map(|_| {
                        outbox.lines_sent.fetch_add(count, Ordering::SeqCst);
                        true
                    })
                })
            }
            Msg::Code(commit) => {
                link.deliver("codigo", &json!({ "run": run, "commit": commit })).map(|a| a.map(|_| true))
            }
        };
        match answered {
            Some(Ok(true)) => outbox.sent(),
            Some(Ok(false)) | Some(Err(_)) => {
                lost.store(true, Ordering::SeqCst);
                token.cancel("o build não é mais deste agente");
                outbox.close();
                return;
            }
            None => {
                outbox.close();
                return;
            }
        }
    }
}

/// `text` as it goes in a query string.
fn encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

impl BuildSite for AgentSite {
    fn log(&self) -> Arc<dyn BuildLog> {
        self.log.clone()
    }

    fn environment(&self) -> BTreeMap<String, String> {
        self.environment.clone()
    }

    fn fail(&self, reason: &str) -> Result<(), CiError> {
        let status = self.outcome(&RunOutcome::Stopped { node: String::new(), reason: reason.to_owned() });
        if status.ended() {
            self.end(status);
        }
        Ok(())
    }

    fn started(&self, commit: Option<&str>) {
        *self.commit.lock().unwrap_or_else(PoisonError::into_inner) = commit.map(str::to_owned);
        self.outbox.push(Msg::Code(commit.map(str::to_owned)));
    }

    fn steps(&self, catalog: &Catalog) -> Result<BuildStatus, CiError> {
        let run = self.lease.run(catalog, Arc::new(SystemClock), self.token.clone(), |record| {
            self.outbox.push(Msg::Step(record));
        });
        let outcome = match run {
            Ok(run) => run.outcome,
            Err(reason) => RunOutcome::Stopped { node: String::new(), reason },
        };
        Ok(self.outcome(&outcome))
    }

    fn cancel(&self, reason: &str) {
        self.token.cancel(reason);
    }

    fn attach(&self, attachment: &Attachment) -> Result<(), CiError> {
        self.outbox.flush();
        let parts: Vec<&[u8]> = if attachment.bytes.is_empty() {
            vec![&[]]
        } else {
            attachment.bytes.chunks(PART_BYTES).collect()
        };
        let last = parts.len() - 1;
        for (n, part) in parts.into_iter().enumerate() {
            let path = format!(
                "/api/ci/agentes/anexo?run={}&nome={}&tipo={}&mime={}&resumo={}&ultima={}",
                encoded(&self.run),
                encoded(&attachment.name),
                encoded(&attachment.kind),
                encoded(&attachment.mime),
                encoded(&attachment.summary.to_string()),
                u8::from(n == last)
            );
            match self.link.deliver_bytes(&path, "application/octet-stream", part) {
                Some(Ok(_)) => {}
                Some(Err((status, said))) => {
                    return Err(CiError::failed(format!("o servidor recusou o anexo ({status}): {said}")));
                }
                None => return Err(CiError::failed("o agente parou antes de mandar o anexo")),
            }
        }
        Ok(())
    }

    fn ended(&self, status: BuildStatus) {
        self.outbox.flush();
        if !self.lost() {
            self.end(status);
        }
    }
}

/// The secrets the plugins of an agent ask for: the server's, asked while a build runs.
#[derive(Debug)]
pub struct RemoteSecrets {
    link: Arc<Link>,
    hidden: Mutex<BTreeMap<String, Vec<String>>>,
}

impl RemoteSecrets {
    /// Asking through `link`.
    pub fn new(link: Arc<Link>) -> Self {
        Self { link, hidden: Mutex::default() }
    }
}

impl Secrets for RemoteSecrets {
    fn get(&self, name: &str) -> Option<String> {
        let answer = self.link.deliver("segredo", &json!({ "name": name }))?.ok()?;
        let value = answer.get("value").and_then(Value::as_str)?.to_owned();
        let hidden: Vec<String> = answer
            .get("hidden")
            .and_then(Value::as_array)
            .map(|all| all.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        self.hidden.lock().unwrap_or_else(PoisonError::into_inner).insert(name.to_owned(), hidden);
        Some(value)
    }

    fn hidden(&self, name: &str) -> Vec<String> {
        self.hidden.lock().unwrap_or_else(PoisonError::into_inner).get(name).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_in_a_query_is_encoded() {
        assert_eq!(encoded("relatório final.html"), "relat%C3%B3rio%20final.html");
        assert_eq!(encoded("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn the_outbox_sends_steps_and_lines_in_batches_of_their_own() {
        let outbox = Outbox::new();
        outbox.push(Msg::Line(AgentLine { step: "a".into(), text: "1".into() }));
        outbox.push(Msg::Line(AgentLine { step: "a".into(), text: "2".into() }));
        outbox.push(Msg::Step(StepRecord::Started { node: "b".into() }));
        outbox.push(Msg::Code(None));
        let first = outbox.next().expect("a batch");
        assert_eq!(first.len(), 2);
        outbox.sent();
        assert!(matches!(outbox.next().expect("a batch")[0], Msg::Step(_)));
        outbox.sent();
        assert!(matches!(outbox.next().expect("a batch")[0], Msg::Code(None)));
        outbox.sent();
        outbox.flush();
        outbox.close();
        assert!(outbox.next().is_none());
    }
}
