//! Nodes of a flow that call the coding agents installed on the computer, the Codex CLI and the
//! Claude CLI, through `ravenrs-process`.
//!
//! The app never holds the login: the CLI keeps it. A node reads only by default, and every
//! limit it is given (only reading, the tools it may use, a budget) is checked against what the
//! installed CLI says it can do (its `--help`), so a limit the CLI does not keep is refused
//! before anything runs, never ignored. The events the CLI writes (JSON lines) are read into
//! [`AgentEvent`]s, with the values of secret variables taken out, and kept with the node's
//! answer.
//!
//! | Piece | What it does |
//! |---|---|
//! | [`Cli`] | Finds the CLI ([`Cli::probe`]), builds its arguments from a [`Policy`] and runs it. |
//! | [`Policy`] | Reading only (the default), the folder, the tools allowed and denied, a time limit, turns and a budget. |
//! | [`Answer`] | The final message, the events, the session to resume and the cost, when the CLI says it. |
//! | [`with_agents`] | Adds the nodes `codex`, `claude` and `code_review` to a catalog. |

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ravenrs_flows::{Catalog, Item, NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::{End, Event, ProcessError, Spec};
use serde::Serialize;
use serde_json::{Value, json};

/// Which coding agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    /// The Codex CLI (`codex exec`).
    Codex,
    /// The Claude CLI (`claude -p`).
    Claude,
}

impl Agent {
    /// The program's name in `PATH`.
    pub fn program(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    fn help(self) -> &'static [&'static str] {
        match self {
            Self::Codex => &["exec", "--help"],
            Self::Claude => &["--help"],
        }
    }
}

/// What the installed CLI can keep, as its `--help` says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caps {
    /// It runs inside a sandbox that can be read only (`--sandbox`).
    pub sandbox: bool,
    /// It writes its events as JSON lines.
    pub events: bool,
    /// It takes a permission mode (`--permission-mode`).
    pub permission_mode: bool,
    /// It takes the tools allowed and denied.
    pub tools: bool,
    /// It takes a budget in dollars.
    pub budget: bool,
    /// It takes a number of turns.
    pub turns: bool,
}

/// Whether the CLI is there and what it can do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    /// It is not installed, or not in `PATH`.
    Missing,
    /// It is there, but cannot write its events as this crate reads them.
    Incompatible {
        /// What `--version` said.
        version: String,
    },
    /// Ready to run.
    Ready {
        /// What `--version` said.
        version: String,
        /// What it can keep.
        caps: Caps,
    },
}

/// The limits of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    /// Only reading: it changes no file. The default.
    pub read_only: bool,
    /// The folder it works in.
    pub dir: Option<PathBuf>,
    /// The tools it may use; all when empty.
    pub allowed_tools: Vec<String>,
    /// The tools it may not use.
    pub denied_tools: Vec<String>,
    /// How long it may run.
    pub timeout: Duration,
    /// How many turns it may take.
    pub max_turns: Option<u32>,
    /// How many dollars it may spend.
    pub budget_usd: Option<f64>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            read_only: true,
            dir: None,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            timeout: Duration::from_secs(600),
            max_turns: None,
            budget_usd: None,
        }
    }
}

/// What a CLI said along the way.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// The session began, with its id to resume.
    Started {
        /// Its id.
        session: String,
    },
    /// A message of the agent.
    Message {
        /// What it said.
        text: String,
    },
    /// A tool it used, as running a command or changing a file.
    Tool {
        /// Which.
        name: String,
        /// On what.
        detail: String,
    },
    /// Something went wrong.
    Error {
        /// What.
        text: String,
    },
    /// It finished, with what it cost when it says.
    Finished {
        /// In dollars; unknown when the CLI does not say.
        cost_usd: Option<f64>,
    },
    /// An event this crate does not read, by its type.
    Other {
        /// Its type.
        kind: String,
    },
}

/// What a run gave.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Answer {
    /// The last message, the answer.
    pub final_message: Option<String>,
    /// Everything it said, secrets taken out.
    pub events: Vec<AgentEvent>,
    /// The session to resume.
    pub session: Option<String>,
    /// What it cost, when it says.
    pub cost_usd: Option<f64>,
    /// Whether it ended well.
    pub success: bool,
}

/// Why a run could not happen or failed.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentError {
    /// The CLI is not installed.
    Missing(Agent),
    /// The CLI is there but too old to be read.
    Incompatible(String),
    /// A limit asked for is one the installed CLI does not keep, so it did not run.
    Refused(String),
    /// It ran and failed.
    Failed(String),
    /// It passed its time limit or was stopped.
    Stopped,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(agent) => {
                write!(f, "{} is not installed: install it and log in on it", agent.program())
            }
            Self::Incompatible(version) => write!(f, "this version ({version}) cannot be read: update it"),
            Self::Refused(why) => write!(f, "refused before running: {why}"),
            Self::Failed(why) => write!(f, "it failed: {why}"),
            Self::Stopped => write!(f, "it was stopped, or passed its time limit"),
        }
    }
}

impl std::error::Error for AgentError {}

/// An installed coding agent.
#[derive(Clone, Debug)]
pub struct Cli {
    agent: Agent,
    program: OsString,
    vars: Vec<(String, String)>,
}

/// Whether a variable's name says it holds a secret.
fn secret_name(name: &str) -> bool {
    let upper = name.to_uppercase();
    ["TOKEN", "KEY", "SECRET", "PASSWORD", "PASSWD", "AUTH", "CREDENTIAL"].iter().any(|w| upper.contains(w))
}

impl Cli {
    /// The agent, found by its name in `PATH`.
    pub fn new(agent: Agent) -> Self {
        Self { agent, program: agent.program().into(), vars: Vec::new() }
    }

    /// Runs this program instead, as a fake one in tests.
    pub fn program(mut self, program: impl Into<OsString>) -> Self {
        self.program = program.into();
        self
    }

    /// Gives the CLI this variable. Its value is taken out of the events when its name says
    /// it is a secret.
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.push((name.into(), value.into()));
        self
    }

    /// Which agent.
    pub fn agent(&self) -> Agent {
        self.agent
    }

    /// Whether it is installed and what it can keep.
    pub fn probe(&self) -> Probe {
        if ravenrs_process::resolve(&self.program).is_none() {
            return Probe::Missing;
        }
        // The CLI's own variables reach every call to it, the questions too.
        let ask = |args: &[&str]| {
            let mut spec =
                Spec::new(self.program.clone()).args(args.iter().copied()).timeout(Duration::from_secs(15));
            for (name, value) in &self.vars {
                spec = spec.env(name.clone(), value.clone());
            }
            spec.output()
        };
        let version = match ask(&["--version"]) {
            Ok(out) if out.end == End::TimedOut => return Probe::Incompatible { version: String::new() },
            Ok(out) => out.stdout.into_iter().find(|l| !l.trim().is_empty()).unwrap_or_default(),
            Err(ProcessError::NotFound(_)) => return Probe::Missing,
            Err(_) => return Probe::Incompatible { version: String::new() },
        };
        let help =
            ask(self.agent.help()).map(|o| [o.stdout, o.stderr].concat().join("\n")).unwrap_or_default();
        let has = |flag: &str| help.contains(flag);
        let caps = Caps {
            sandbox: has("--sandbox"),
            events: has("--json") || has("stream-json"),
            permission_mode: has("--permission-mode"),
            tools: has("--allowedTools") && has("--disallowedTools"),
            budget: has("--max-budget-usd"),
            turns: has("--max-turns"),
        };
        if caps.events { Probe::Ready { version, caps } } else { Probe::Incompatible { version } }
    }

    /// The arguments of a run under `policy`, resuming `session`; a limit `caps` cannot keep
    /// is refused.
    pub fn arguments(
        &self,
        caps: Caps,
        policy: &Policy,
        session: Option<&str>,
    ) -> Result<Vec<String>, AgentError> {
        let refuse =
            |what: &str| Err(AgentError::Refused(format!("{} does not keep {what}", self.agent.program())));
        let mut args: Vec<String> = Vec::new();
        match self.agent {
            Agent::Codex => {
                if !caps.sandbox {
                    return refuse("a sandbox");
                }
                if !policy.allowed_tools.is_empty() || !policy.denied_tools.is_empty() {
                    return refuse("a list of tools");
                }
                if policy.budget_usd.is_some() {
                    return refuse("a budget");
                }
                if policy.max_turns.is_some() {
                    return refuse("a number of turns");
                }
                args.extend(["exec", "--json", "--skip-git-repo-check", "--sandbox"].map(String::from));
                args.push(if policy.read_only { "read-only" } else { "workspace-write" }.into());
                if let Some(dir) = &policy.dir {
                    args.extend(["--cd".to_owned(), dir.to_string_lossy().into_owned()]);
                }
                if let Some(session) = session {
                    args.extend(["resume".to_owned(), session.to_owned()]);
                }
                // The prompt comes on the input.
                args.push("-".into());
            }
            Agent::Claude => {
                if !caps.permission_mode {
                    return refuse("a permission mode, so not reading only");
                }
                args.extend(
                    ["-p", "--output-format", "stream-json", "--verbose", "--permission-mode"]
                        .map(String::from),
                );
                args.push(if policy.read_only { "plan" } else { "acceptEdits" }.into());
                if !policy.allowed_tools.is_empty() || !policy.denied_tools.is_empty() {
                    if !caps.tools {
                        return refuse("a list of tools");
                    }
                    if !policy.allowed_tools.is_empty() {
                        args.extend(["--allowedTools".to_owned(), policy.allowed_tools.join(",")]);
                    }
                    if !policy.denied_tools.is_empty() {
                        args.extend(["--disallowedTools".to_owned(), policy.denied_tools.join(",")]);
                    }
                }
                if let Some(budget) = policy.budget_usd {
                    if !caps.budget {
                        return refuse("a budget");
                    }
                    args.extend(["--max-budget-usd".to_owned(), budget.to_string()]);
                }
                if let Some(turns) = policy.max_turns {
                    if !caps.turns {
                        return refuse("a number of turns");
                    }
                    args.extend(["--max-turns".to_owned(), turns.to_string()]);
                }
                if let Some(session) = session {
                    args.extend(["--resume".to_owned(), session.to_owned()]);
                }
            }
        }
        Ok(args)
    }

    /// Runs `prompt` under `policy`, resuming `session`; `stop` is asked between events, and
    /// the run stops when it says so.
    pub fn run(
        &self,
        prompt: &str,
        policy: &Policy,
        session: Option<&str>,
        stop: &dyn Fn() -> bool,
    ) -> Result<Answer, AgentError> {
        let caps = match self.probe() {
            Probe::Missing => return Err(AgentError::Missing(self.agent)),
            Probe::Incompatible { version } => return Err(AgentError::Incompatible(version)),
            Probe::Ready { caps, .. } => caps,
        };
        let args = self.arguments(caps, policy, session)?;
        let mut spec = Spec::new(self.program.clone()).args(args).stdin().timeout(policy.timeout);
        if let Some(dir) = &policy.dir {
            spec = spec.dir(dir);
        }
        for (name, value) in &self.vars {
            spec = spec.env(name.clone(), value.clone());
        }
        let running = spec.spawn().map_err(|e| match e {
            ProcessError::NotFound(_) => AgentError::Missing(self.agent),
            other => AgentError::Failed(other.to_string()),
        })?;
        let control = running.control();
        control.send(prompt);
        control.close();
        let secrets = self.secrets();
        let mut answer =
            Answer { final_message: None, events: Vec::new(), session: None, cost_usd: None, success: false };
        let mut errors = Vec::new();
        let end = loop {
            if stop() {
                control.stop();
            }
            match running.next_within(Duration::from_millis(100)) {
                None => continue,
                Some(Event::Out(line)) => {
                    for event in read(self.agent, &redact(&line, &secrets), &mut answer) {
                        answer.events.push(event);
                    }
                }
                Some(Event::Err(line)) => errors.push(redact(&line, &secrets)),
                Some(Event::Ended(end)) => break end,
            }
        };
        match end {
            End::Stopped | End::TimedOut => Err(AgentError::Stopped),
            End::Code(0) => {
                answer.success = !answer.events.iter().any(|e| matches!(e, AgentEvent::Error { .. }));
                if answer.final_message.is_none() {
                    answer.final_message = answer.events.iter().rev().find_map(|e| match e {
                        AgentEvent::Message { text } => Some(text.clone()),
                        _ => None,
                    });
                }
                Ok(answer)
            }
            _ => Err(AgentError::Failed(errors.last().cloned().unwrap_or_else(|| format!("{end:?}")))),
        }
    }

    /// The values to take out of the events: of the variables given, and of this process,
    /// whose names say they are secrets.
    fn secrets(&self) -> Vec<String> {
        self.vars
            .iter()
            .map(|(n, v)| (n.clone(), v.clone()))
            .chain(std::env::vars())
            .filter(|(name, value)| secret_name(name) && value.len() >= 6)
            .map(|(_, value)| value)
            .collect()
    }
}

/// `text` with every secret value replaced.
fn redact(text: &str, secrets: &[String]) -> String {
    secrets.iter().fold(text.to_owned(), |text, secret| text.replace(secret.as_str(), "•••"))
}

/// The events of one line the CLI wrote, filling what the answer learns from them.
fn read(agent: Agent, line: &str, answer: &mut Answer) -> Vec<AgentEvent> {
    let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
        return if line.trim().is_empty() {
            Vec::new()
        } else {
            vec![AgentEvent::Other { kind: "text".into() }]
        };
    };
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let kind = text(&value, "type");
    match agent {
        Agent::Codex => match kind.as_str() {
            "thread.started" => {
                let session = text(&value, "thread_id");
                answer.session = Some(session.clone());
                vec![AgentEvent::Started { session }]
            }
            "item.completed" | "item.started" => {
                let item = value.get("item").cloned().unwrap_or(Value::Null);
                match text(&item, "type").as_str() {
                    "agent_message" if kind == "item.completed" => {
                        vec![AgentEvent::Message { text: text(&item, "text") }]
                    }
                    "command_execution" if kind == "item.started" => {
                        vec![AgentEvent::Tool { name: "command".into(), detail: text(&item, "command") }]
                    }
                    "file_change" => vec![AgentEvent::Tool {
                        name: "file".into(),
                        detail: item.get("changes").map(Value::to_string).unwrap_or_default(),
                    }],
                    "error" => vec![AgentEvent::Error { text: text(&item, "message") }],
                    _ => Vec::new(),
                }
            }
            "turn.completed" => vec![AgentEvent::Finished { cost_usd: None }],
            "turn.failed" | "error" => {
                let why = value.get("error").map_or_else(|| text(&value, "message"), |e| text(e, "message"));
                vec![AgentEvent::Error { text: why }]
            }
            other => vec![AgentEvent::Other { kind: other.to_owned() }],
        },
        Agent::Claude => match kind.as_str() {
            "system" => {
                let session = text(&value, "session_id");
                answer.session = Some(session.clone());
                vec![AgentEvent::Started { session }]
            }
            "assistant" => value
                .pointer("/message/content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|part| match text(part, "type").as_str() {
                    "text" => Some(AgentEvent::Message { text: text(part, "text") }),
                    "tool_use" => Some(AgentEvent::Tool {
                        name: text(part, "name"),
                        detail: part.get("input").map(Value::to_string).unwrap_or_default(),
                    }),
                    _ => None,
                })
                .collect(),
            "result" => {
                let cost = value.get("total_cost_usd").and_then(Value::as_f64);
                answer.cost_usd = cost;
                if let Some(session) = value.get("session_id").and_then(Value::as_str) {
                    answer.session = Some(session.to_owned());
                }
                let result = text(&value, "result");
                let mut events = Vec::new();
                if value.get("is_error").and_then(Value::as_bool) == Some(true) {
                    events.push(AgentEvent::Error { text: result });
                } else if !result.is_empty() {
                    answer.final_message = Some(result);
                }
                events.push(AgentEvent::Finished { cost_usd: cost });
                events
            }
            "user" => Vec::new(),
            other => vec![AgentEvent::Other { kind: other.to_owned() }],
        },
    }
}

/// A node that calls a coding agent with the prompt of its parameter `prompt` (text with
/// expressions inside it), once per item, under the policy of its parameters.
#[derive(Clone, Debug)]
pub struct AgentNode {
    kind: &'static str,
    title: &'static str,
    cli: Arc<Cli>,
    /// Asks for a review: reading only always, and the prompt starts with how to review.
    review: bool,
}

/// How a review asks, before what the node's prompt says.
const REVIEW: &str =
    "Revise o que vem abaixo e aponte cada problema com arquivo e linha. Não altere nenhum arquivo.\n\n";

impl AgentNode {
    /// The node `codex`.
    pub fn codex(cli: Cli) -> Self {
        Self { kind: "codex", title: "Codex CLI", cli: Arc::new(cli), review: false }
    }

    /// The node `claude`.
    pub fn claude(cli: Cli) -> Self {
        Self { kind: "claude", title: "Claude CLI", cli: Arc::new(cli), review: false }
    }

    /// The node `code_review`, with this agent.
    pub fn review(cli: Cli) -> Self {
        Self { kind: "code_review", title: "Code review", cli: Arc::new(cli), review: true }
    }

    fn policy(&self, cx: &NodeContext<'_>) -> Policy {
        let strings = |name: &str| -> Vec<String> {
            cx.parameter(name)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default()
        };
        let mut policy = Policy {
            read_only: self.review || cx.parameter("read_only").and_then(Value::as_bool).unwrap_or(true),
            dir: cx.parameter("dir").and_then(Value::as_str).filter(|d| !d.is_empty()).map(PathBuf::from),
            allowed_tools: strings("allowed_tools"),
            denied_tools: strings("denied_tools"),
            max_turns: cx.parameter("max_turns").and_then(Value::as_u64).map(|n| n as u32),
            budget_usd: cx.parameter("budget_usd").and_then(Value::as_f64),
            ..Policy::default()
        };
        if let Some(ms) = cx.parameter("timeout_ms").and_then(Value::as_u64) {
            policy.timeout = Duration::from_millis(ms);
        }
        if let Some(left) = cx.time_left_ms() {
            policy.timeout = policy.timeout.min(Duration::from_millis(left.max(0) as u64));
        }
        policy
    }
}

impl NodeKind for AgentNode {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(self.kind, 1).title(self.title).input("main").output("main").requires("prompt")
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let policy = self.policy(cx);
        let session = cx.parameter("session").and_then(Value::as_str).filter(|s| !s.is_empty());
        let items: Vec<Item> = match cx.input("main") {
            [] => vec![Item::new(json!({}))],
            some => some.to_vec(),
        };
        let mut out = Vec::with_capacity(items.len());
        for item in &items {
            let prompt = cx.text("prompt", item)?.unwrap_or_default();
            let prompt = if self.review { format!("{REVIEW}{prompt}") } else { prompt };
            let answer =
                self.cli.run(&prompt, &policy, session, &|| cx.cancelled()).map_err(|e| match e {
                    AgentError::Missing(_) | AgentError::Incompatible(_) | AgentError::Refused(_) => {
                        NodeError::new(e.to_string())
                    }
                    AgentError::Stopped => NodeError::cancelled(e.to_string()),
                    // Reading only, nothing changed: it can be tried again. Otherwise whether files
                    // changed is not known.
                    AgentError::Failed(_) if policy.read_only => NodeError::retryable(e.to_string()),
                    AgentError::Failed(_) => NodeError::unknown(e.to_string()),
                })?;
            out.push(item.derive(serde_json::to_value(&answer).unwrap_or(Value::Null)));
        }
        Ok(Ports::from([("main".into(), out)]))
    }
}

/// `catalog` with the nodes `codex` and `claude`, and `code_review` with `review`'s agent.
pub fn with_agents(catalog: Catalog, codex: Cli, claude: Cli, review: Cli) -> Catalog {
    catalog.with(AgentNode::codex(codex)).with(AgentNode::claude(claude)).with(AgentNode::review(review))
}
