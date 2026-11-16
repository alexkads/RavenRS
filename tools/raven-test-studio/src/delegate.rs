//! Handing a scenario to a coding agent: the Studio asks Codex (or Claude), with the cheapest
//! model and reading only, to run the scenario through the Studio's own command line, read the
//! report and, when it failed, find the cause in the Rust sources. Then the Studio runs the same
//! scenario itself and says whether the agent's verdict is the one it found, so a delegated
//! answer is never taken on trust.
//!
//! The agent is called through `ravenrs-agent-cli`, the same that Raven Flows uses: the CLI keeps
//! its own login, every limit is checked against what it says it can do, and secrets are taken
//! out of what is kept.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ravenrs_agent_cli::{Agent, Answer, Cli, Policy, cheapest_model};
use serde_json::{Value, json};

use crate::report::Report;

/// What a delegation gave.
#[derive(Clone, Debug)]
pub struct Delegation {
    /// Which agent ran it.
    pub agent: Agent,
    /// The model it ran with; the CLI's own when `None`.
    pub model: Option<String>,
    /// What the agent answered, read from its final message; `None` when it was not JSON.
    pub said: Option<Value>,
    /// Its final message, as written.
    pub message: String,
    /// The commands it ran.
    pub commands: Vec<String>,
    /// The session to resume it.
    pub session: Option<String>,
    /// The report of the same scenario, run by the Studio itself.
    pub checked: Report,
    /// How long the agent took.
    pub duration_ms: u64,
}

impl Delegation {
    /// The verdict the agent gave, `passed` or `failed`.
    pub fn verdict(&self) -> Option<&str> {
        self.said.as_ref()?.get("verdict")?.as_str()
    }

    /// Whether the agent's verdict is the one the Studio found running it itself.
    pub fn confirmed(&self) -> bool {
        self.verdict() == Some(if self.checked.passed() { "passed" } else { "failed" })
    }

    /// The agent's name, as people know it.
    pub fn agent_name(&self) -> &'static str {
        match self.agent {
            Agent::Codex => "Codex",
            Agent::Claude => "Claude",
        }
    }

    /// For a tool: what the agent said, with the Studio's own report beside it.
    pub fn json(&self) -> Value {
        let mut report = self.checked.json();
        report["delegation"] = json!({
            "agent": self.agent.program(),
            "model": self.model,
            "verdict": self.verdict(),
            "confirmed": self.confirmed(),
            "said": self.said,
            "message": self.message,
            "commands": self.commands,
            "session": self.session,
            "duration_ms": self.duration_ms,
        });
        report
    }

    /// For a person.
    pub fn text(&self) -> String {
        let model = self.model.as_deref().unwrap_or("modelo padrão");
        let field = |name: &str| self.said.as_ref().and_then(|s| s.get(name)).and_then(Value::as_str);
        let mut out = format!(
            "Delegado ao {} ({model}) em {:.1} s\n",
            self.agent_name(),
            self.duration_ms as f64 / 1000.0
        );
        match self.verdict() {
            Some(verdict) => {
                out.push_str(&format!("o {} disse: {verdict}", self.agent_name()));
                if let Some(summary) = field("summary") {
                    out.push_str(&format!(" — {summary}"));
                }
                out.push('\n');
            }
            None => out.push_str(&format!(
                "o {} não respondeu no formato pedido:\n{}\n",
                self.agent_name(),
                self.message
            )),
        }
        for (label, name) in [("causa", "cause"), ("como corrigir", "fix")] {
            if let Some(text) = field(name).filter(|t| !t.is_empty()) {
                out.push_str(&format!("{label}: {text}\n"));
            }
        }
        out.push_str(if self.confirmed() {
            "o Studio rodou o mesmo cenário e confirmou o veredito\n"
        } else {
            "ATENÇÃO: o Studio rodou o mesmo cenário e chegou a outro veredito\n"
        });
        out.push('\n');
        out.push_str(&self.checked.text());
        out
    }
}

/// The agent a name means: `codex` or `claude`.
pub fn agent_named(name: &str) -> Option<Agent> {
    match name.trim().to_lowercase().as_str() {
        "codex" => Some(Agent::Codex),
        "claude" => Some(Agent::Claude),
        _ => None,
    }
}

/// The folder the agent reads: the RavenRS checkout the Studio was built from, when it is
/// still there, or the current folder.
fn sources() -> PathBuf {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if checkout.join("Cargo.toml").is_file() {
        checkout.canonicalize().unwrap_or(checkout)
    } else {
        std::env::current_dir().unwrap_or_default()
    }
}

/// What the agent is asked: run the scenario with the Studio, read the report, and answer in
/// JSON. In Portuguese, as the Studio speaks.
pub fn mission(studio: &Path, scenario: &str, sources: &Path) -> String {
    format!(
        "Você é o engenheiro de testes e2e do Raven Test Studio, a ferramenta de testes do framework RavenRS (Rust).\n\
         \n\
         Missão: rode o cenário \"{scenario}\" e diga se passou.\n\
         1. Rode exatamente: \"{studio}\" --run {scenario} --format json --no-cache\n\
         2. Leia o relatório JSON: verdict, summary e, no bloco que falhou, code, meaning, detail, fix e screen.\n\
         3. Se falhou, procure a causa no código Rust em {sources} (tools/raven-test-studio e crates/ravenrs-e2e) e aponte arquivo e linha.\n\
         Não altere nenhum arquivo. Não rode cargo nem outro comando que compile.\n\
         \n\
         Responda só com um objeto JSON, sem texto antes ou depois:\n\
         {{\"verdict\": \"passed\" ou \"failed\", \"summary\": \"a frase do relatório\", \"failed_step\": número ou null, \
         \"code\": \"o código E2E_ ou null\", \"cause\": \"a causa, em uma frase\", \"fix\": \"como corrigir, com arquivo:linha\"}}",
        studio = studio.display(),
        sources = sources.display(),
    )
}

/// The JSON object in an agent's final message, which may come inside a code fence or with words
/// around it.
pub fn read_answer(message: &str) -> Option<Value> {
    let start = message.find('{')?;
    let end = message.rfind('}')?;
    let value: Value = serde_json::from_str(message.get(start..=end)?).ok()?;
    value.get("verdict").and_then(Value::as_str).filter(|v| *v == "passed" || *v == "failed")?;
    Some(value)
}

/// Hands `scenario` to `cli`'s agent with `model` (its cheapest when `None`), then runs it in
/// the Studio to check the answer. `Err` says why the agent could not run.
pub fn delegate(cli: &Cli, scenario: &str, model: Option<String>) -> Result<Delegation, String> {
    let checked = crate::run_named(scenario, None)?;
    let studio = std::env::current_exe().map_err(|e| format!("não achei o próprio Studio: {e}"))?;
    let model = model.or_else(|| cheapest_model(cli.agent()));
    let sources = sources();
    let policy = Policy {
        read_only: true,
        dir: Some(sources.clone()),
        timeout: Duration::from_secs(600),
        model: model.clone(),
        ..Policy::default()
    };
    let started = std::time::Instant::now();
    let answer: Answer = cli
        .run(&mission(&studio, scenario, &sources), &policy, None, &|| false)
        .map_err(|e| format!("o {} não rodou: {e}", cli.agent().program()))?;
    let message = answer.final_message.clone().unwrap_or_default();
    let commands = answer
        .events
        .iter()
        .filter_map(|event| match event {
            ravenrs_agent_cli::AgentEvent::Tool { detail, .. } if !detail.is_empty() => Some(detail.clone()),
            _ => None,
        })
        .collect();
    Ok(Delegation {
        agent: cli.agent(),
        model,
        said: read_answer(&message),
        message,
        commands,
        session: answer.session,
        checked,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}
