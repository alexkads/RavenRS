//! What a run tells whoever asked for it: a person in a terminal, or the coding agent that
//! called the Studio.
//!
//! The text is a few lines an agent can quote as is. The JSON carries every block with its
//! result, the stable code of a failure and the screen of the app at that moment, so the agent
//! can explain or fix the failure without running it again.

use ravenrs::prelude::FlowGraph;
use serde_json::{Value, json};

use crate::runner::{self, Outcome, Run};

/// The formats a report prints in, as `--format` names them.
pub const FORMATS: [&str; 4] = ["text", "json", "jsonl", "junit"];

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
    /// The titles of the blocks of the path that did not run, because one before them failed.
    pub not_run: Vec<String>,
}

impl Report {
    /// Runs `graph` to the end with `run` and reports it under `scenario`.
    pub fn of(scenario: &str, mut graph: FlowGraph, mut run: Run) -> Self {
        let total = runner::path(&graph).len();
        let apart = crate::sample::opens_apart(&graph);
        let started = std::time::Instant::now();
        let outcomes = run.to_end(&mut graph);
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let not_run = runner::path(&graph)
            .iter()
            .skip(outcomes.len())
            .filter_map(|id| graph.nodes.iter().find(|n| n.id == *id))
            .map(|n| n.title.clone())
            .collect();
        Self { scenario: scenario.to_owned(), outcomes, total, cached_at: None, duration_ms, apart, not_run }
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
            ms: 0,
        };
        Self {
            scenario: scenario.to_owned(),
            outcomes: vec![outcome],
            total: 1,
            cached_at: None,
            duration_ms: 0,
            apart: true,
            not_run: Vec::new(),
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
                    ms: b.get("duration_ms").and_then(Value::as_u64).unwrap_or_default(),
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
            not_run: value
                .get("not_run")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default(),
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
                    "duration_ms": o.ms,
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
            "not_run": self.not_run,
        })
    }
}

impl Report {
    /// The run as lines of JSON, one per event: `start`, a `step` for each block that ran, with
    /// how long it took and how it went, and `end` with the verdict. It is what `trace.jsonl`
    /// keeps, and what `--format jsonl` prints.
    pub fn trace(&self) -> String {
        let mut lines = vec![json!({
            "event": "start",
            "scenario": self.scenario,
            "total": self.total,
            "opened_apart": self.apart,
            "from_cache": self.cached_at.is_some(),
        })];
        for (i, o) in self.outcomes.iter().enumerate() {
            lines.push(json!({
                "event": "step",
                "step": i + 1,
                "id": o.id,
                "kind": o.kind,
                "title": o.title,
                "line": o.line,
                "result": if o.error.is_none() { "passed" } else { "failed" },
                "code": o.code,
                "error": o.error,
                "duration_ms": o.ms,
            }));
        }
        for (i, title) in self.not_run.iter().enumerate() {
            lines.push(json!({
                "event": "skipped",
                "step": self.outcomes.len() + i + 1,
                "title": title,
            }));
        }
        lines.push(json!({
            "event": "end",
            "verdict": if self.passed() { "passed" } else { "failed" },
            "summary": runner::summary(&self.outcomes, self.total),
            "duration_ms": self.duration_ms,
        }));
        lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n"
    }

    /// `reports` in the JUnit XML that build servers and test tools read: a `testsuite` for each
    /// report and a `testcase` for each block of its path. A failure carries the stable code in
    /// `type`, and the fix and the screen of the app in its text; the blocks after it are
    /// `skipped`.
    pub fn junit(reports: &[Report]) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        let tests: usize = reports.iter().map(|r| r.outcomes.len() + r.not_run.len()).sum();
        let failures = reports.iter().filter(|r| r.failure().is_some()).count();
        let skipped: usize = reports.iter().map(|r| r.not_run.len()).sum();
        let seconds: u64 = reports.iter().map(|r| r.duration_ms).sum();
        out.push_str(&format!(
            "<testsuites name=\"Raven Test Studio\" tests=\"{tests}\" failures=\"{failures}\" skipped=\"{skipped}\" time=\"{}\">\n",
            secs(seconds)
        ));
        for report in reports {
            let suite = xml(&report.scenario);
            out.push_str(&format!(
                "  <testsuite name=\"{suite}\" tests=\"{}\" failures=\"{}\" skipped=\"{}\" time=\"{}\">\n",
                report.outcomes.len() + report.not_run.len(),
                usize::from(report.failure().is_some()),
                report.not_run.len(),
                secs(report.duration_ms)
            ));
            for (i, o) in report.outcomes.iter().enumerate() {
                let name = xml(&format!("{} {}: {}", i + 1, o.title, o.line));
                let open =
                    format!("    <testcase classname=\"{suite}\" name=\"{name}\" time=\"{}\"", secs(o.ms));
                match &o.error {
                    None => out.push_str(&format!("{open}/>\n")),
                    Some(error) => {
                        let mut body = error.clone();
                        if let Some(fix) = &o.fix {
                            body.push_str(&format!("\ncomo corrigir: {fix}"));
                        }
                        if let Some(screen) = &o.screen {
                            body.push_str(&format!("\ntela do app testado:\n{screen}"));
                        }
                        out.push_str(&format!(
                            "{open}>\n      <failure type=\"{}\" message=\"{}\">{}</failure>\n    </testcase>\n",
                            xml(o.code.as_deref().unwrap_or("E2E_FAILED")),
                            xml(error),
                            xml(&body)
                        ));
                    }
                }
            }
            for (i, title) in report.not_run.iter().enumerate() {
                let name = xml(&format!("{} {title}", report.outcomes.len() + i + 1));
                out.push_str(&format!(
                    "    <testcase classname=\"{suite}\" name=\"{name}\" time=\"0\">\n      <skipped message=\"um passo antes falhou\"/>\n    </testcase>\n"
                ));
            }
            out.push_str("  </testsuite>\n");
        }
        out.push_str("</testsuites>\n");
        out
    }

    /// Keeps the run in `folder`, under a folder named after the scenario: `relatorio.json`,
    /// `trace.jsonl` and, when it failed and the app's screen came with the failure,
    /// `arvore.txt`. A run that passes removes an `arvore.txt` left by an earlier failure.
    /// Says where it kept them.
    pub fn save(&self, folder: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        let dir = folder.join(folder_name(&self.scenario));
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(&self.json()).unwrap_or_default();
        std::fs::write(dir.join("relatorio.json"), json + "\n")?;
        std::fs::write(dir.join("trace.jsonl"), self.trace())?;
        let tree = dir.join("arvore.txt");
        match self.failure().and_then(|f| f.screen.as_ref()) {
            Some(screen) => std::fs::write(&tree, format!("{screen}\n"))?,
            None if tree.exists() => std::fs::remove_file(&tree)?,
            None => {}
        }
        Ok(dir)
    }
}

/// A scenario's name as one folder: letters, digits, `-` and `_` stay, a run of anything else
/// becomes one `-`, so a `/` or `..` in a name never leaves the folder given.
pub fn folder_name(scenario: &str) -> String {
    let mut name = String::new();
    for c in scenario.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            name.push(c);
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_matches('-');
    if name.is_empty() { "cenario".to_owned() } else { name.to_owned() }
}

/// Text that goes inside an XML attribute or element.
fn xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters other than tab and line breaks are not allowed in XML 1.0.
            c if c.is_control() && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// Milliseconds as the seconds JUnit writes.
fn secs(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
