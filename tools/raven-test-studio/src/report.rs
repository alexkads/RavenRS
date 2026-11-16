//! What a run tells whoever asked for it: a person in a terminal, or the coding agent that
//! called the Studio.
//!
//! The text is a few lines an agent can quote as is. The JSON carries every block with its
//! result, the stable code of a failure and the screen of the app at that moment, so the agent
//! can explain or fix the failure without running it again.

use ravenrs::prelude::FlowGraph;
use serde_json::{Value, json};

use crate::runner::{self, Outcome, Run};

/// What a run of an app compiled into the Studio could not check, said in its report.
pub const NOT_VERIFIED: [&str; 3] = [
    "o app testado roda no mesmo processo do Studio, compilado junto com ele",
    "Linux e Windows não rodaram aqui",
    "a comparação com as pranchetas do design vem na F14 da #627",
];

/// What a run of an app opened apart could not check, said in its report.
pub const NOT_VERIFIED_APART: [&str; 3] = [
    "o app testado rodou à parte, sem janela e sem GPU, nos dados de exemplo em memória: o desenho na tela e os dados de quem usa não foram tocados",
    "Linux e Windows não rodaram aqui",
    "a comparação com as pranchetas do design vem na F14 da #627",
];

/// A finished run of a scenario.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The scenario's name.
    pub scenario: String,
    /// How each block went, in order.
    pub outcomes: Vec<Outcome>,
    /// How many blocks the scenario's path has.
    pub total: usize,
    /// When the run this report came from was kept in the cache, in seconds since 1970;
    /// `None` when it ran just now.
    pub cached_at: Option<u64>,
    /// How long the run took, in milliseconds, measured on the clock of the machine that ran
    /// it (a report from the cache keeps the time of the run it came from).
    pub duration_ms: u64,
    /// Whether the app ran apart, in a child process over the test channel.
    pub apart: bool,
}

impl Report {
    /// Runs `graph` to the end with `run` and reports it under `scenario`.
    pub fn of(scenario: &str, mut graph: FlowGraph, mut run: Run) -> Self {
        let total = runner::path(&graph).len();
        let apart = crate::sample::opens_apart(&graph);
        let started = std::time::Instant::now();
        let outcomes = run.to_end(&mut graph);
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        Self { scenario: scenario.to_owned(), outcomes, total, cached_at: None, duration_ms, apart }
    }

    /// A run that could not start: its first block failed with `why`, before any app opened.
    pub fn refused(scenario: &str, block: &str, why: &str) -> Self {
        let outcome = Outcome {
            id: "open".to_owned(),
            kind: "open".to_owned(),
            title: format!("Abrir o {block}"),
            line: block.to_owned(),
            error: Some(why.to_owned()),
            code: None,
            meaning: None,
            detail: None,
            fix: None,
            close: Vec::new(),
            screen: None,
        };
        Self {
            scenario: scenario.to_owned(),
            outcomes: vec![outcome],
            total: 1,
            cached_at: None,
            duration_ms: 0,
            apart: true,
        }
    }

    /// What this run could not check.
    pub fn not_verified(&self) -> [&'static str; 3] {
        if self.apart { NOT_VERIFIED_APART } else { NOT_VERIFIED }
    }

    /// A report read back from what [`Report::json`] wrote.
    pub fn from_json(value: &Value) -> Option<Self> {
        let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
        let outcomes = value
            .get("blocks")?
            .as_array()?
            .iter()
            .map(|b| {
                Some(Outcome {
                    id: text(b, "id")?,
                    kind: text(b, "kind")?,
                    title: text(b, "title")?,
                    line: text(b, "line").unwrap_or_default(),
                    error: text(b, "error"),
                    code: text(b, "code"),
                    meaning: text(b, "meaning"),
                    detail: text(b, "detail"),
                    fix: text(b, "fix"),
                    close: b
                        .get("close")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                        .unwrap_or_default(),
                    screen: text(b, "screen"),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            scenario: text(value, "scenario")?,
            outcomes,
            total: usize::try_from(value.get("total")?.as_u64()?).ok()?,
            cached_at: None,
            duration_ms: value.get("duration_ms").and_then(Value::as_u64).unwrap_or_default(),
            apart: value.get("opened_apart").and_then(Value::as_bool).unwrap_or_default(),
        })
    }

    /// Whether every block of the path ran and passed.
    pub fn passed(&self) -> bool {
        self.outcomes.len() == self.total && self.outcomes.iter().all(|o| o.error.is_none())
    }

    /// The first block that failed, if any.
    pub fn failure(&self) -> Option<&Outcome> {
        self.outcomes.iter().find(|o| o.error.is_some())
    }

    /// The process exit code: 0 when it passed, 1 when it did not.
    pub fn exit_code(&self) -> i32 {
        if self.passed() { 0 } else { 1 }
    }

    /// A few lines to read or quote: the verdict, the failure and its fix, what was not checked.
    pub fn text(&self) -> String {
        let mut out = format!(
            "{}: {}\n{}",
            self.scenario,
            if self.passed() { "passou" } else { "falhou" },
            runner::summary(&self.outcomes, self.total)
        );
        if self.cached_at.is_some() {
            out.push_str(
                "\ndo cache: nem o cenário nem o build mudaram desde a última vez (--no-cache roda de novo)",
            );
        }
        if let Some(f) = self.failure() {
            out.push_str(&format!(
                "\nfalhou em {} ({}: {}): {}",
                f.title,
                f.kind,
                f.line,
                f.error.as_deref().unwrap_or_default()
            ));
            if let Some(fix) = &f.fix {
                out.push_str(&format!("\ncomo corrigir: {fix}"));
            }
            if !f.close.is_empty() {
                out.push_str("\nparecidos na tela:");
                for line in &f.close {
                    out.push_str(&format!("\n  {line}"));
                }
            }
            if let Some(screen) = &f.screen {
                out.push_str("\ntela do app testado:\n");
                out.push_str(screen);
            }
        }
        out.push_str("\nnão verificado: ");
        out.push_str(&self.not_verified().join("; "));
        out
    }

    /// Everything, for a tool: the verdict, each block with its result, code and screen.
    pub fn json(&self) -> Value {
        let blocks: Vec<Value> = self
            .outcomes
            .iter()
            .enumerate()
            .map(|(i, o)| {
                json!({
                    "step": i + 1,
                    "id": o.id,
                    "kind": o.kind,
                    "title": o.title,
                    "line": o.line,
                    "result": if o.error.is_none() { "passed" } else { "failed" },
                    "code": o.code,
                    "error": o.error,
                    "meaning": o.meaning,
                    "detail": o.detail,
                    "fix": o.fix,
                    "close": o.close,
                    "screen": o.screen,
                })
            })
            .collect();
        json!({
            "scenario": self.scenario,
            "verdict": if self.passed() { "passed" } else { "failed" },
            "summary": runner::summary(&self.outcomes, self.total),
            "total": self.total,
            "ran": self.outcomes.len(),
            "blocks": blocks,
            "duration_ms": self.duration_ms,
            "from_cache": self.cached_at.is_some(),
            "cached_at": self.cached_at,
            "not_verified": self.not_verified(),
            "opened_apart": self.apart,
        })
    }
}
