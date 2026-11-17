//! What a run tells whoever asked for it: a person in a terminal, a build server, or the coding
//! agent that called the tool.
//!
//! The text is a few lines an agent can quote as is. The JSON carries every block with its
//! result, the stable code of a failure and the screen of the app at that moment, so the agent
//! can explain or fix the failure without running it again.

use serde_json::{Value, json};

use crate::session::{self as runner, Outcome};

/// The formats a report prints in, as `--format` names them.
pub const FORMATS: [&str; 5] = ["text", "json", "jsonl", "junit", "html"];

/// What a run of an app compiled into the tool could not check, said in its report.
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

/// What a run with the app's real window on screen could not check, said in its report; the
/// systems it did not run on come from [`crate::window::other_systems`].
pub const NOT_VERIFIED_HEADED: [&str; 3] = [
    "o app testado rodou à parte, com a janela real na tela, nos dados de teste do app: os dados de quem usa não foram tocados",
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
    /// The films kept of the apps the run drove (`.ravenmovie`), when it filmed.
    pub films: Vec<String>,
    /// Why an app was not filmed, when it was asked to be.
    pub film_note: Option<String>,
    /// Whether the apps showed their window (`--headed`).
    pub window: crate::window::WindowMode,
    /// How long the run waited after each step, for a person to follow it.
    pub slow_mo_ms: u64,
    /// Whether the person watching stopped the run before its end.
    pub stopped: bool,
}

impl Report {
    /// A run that could not start: its first block failed with `why`, before any app opened.
    pub fn refused(scenario: &str, block: &str, why: &str) -> Self {
        Self::refused_with(scenario, block, runner::Failure::plain(why))
    }

    /// A run refused before its scenarios could start, preserving the structured failure
    /// from opening the app or reading its embedded scenarios. Use [`Self::refused`] for a
    /// plain message; neither constructor infers a code from the message's text.
    pub fn refused_with(scenario: &str, block: &str, failure: runner::Failure) -> Self {
        let outcome = Outcome {
            id: "open".to_owned(),
            kind: "open".to_owned(),
            title: format!("Abrir o {block}"),
            line: block.to_owned(),
            error: Some(failure.message),
            code: failure.code,
            meaning: failure.meaning,
            detail: failure.detail,
            fix: failure.fix,
            close: failure.close,
            screen: failure.screen,
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
            films: Vec::new(),
            film_note: None,
            window: crate::app::window(),
            slow_mo_ms: 0,
            stopped: false,
        }
    }

    /// What this run could not check, with the systems it did not run on.
    pub fn not_verified(&self) -> Vec<&'static str> {
        let mut said = match (self.window, self.apart) {
            (crate::window::WindowMode::Headed, _) => NOT_VERIFIED_HEADED,
            (_, true) => NOT_VERIFIED_APART,
            (_, false) => NOT_VERIFIED,
        }
        .to_vec();
        said[1] = crate::window::other_systems();
        said
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
            films: value
                .get("films")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default(),
            film_note: text(value, "film_note"),
            window: value
                .get("window")
                .and_then(Value::as_str)
                .and_then(crate::window::WindowMode::read)
                .unwrap_or_default(),
            slow_mo_ms: value.get("slow_mo_ms").and_then(Value::as_u64).unwrap_or_default(),
            stopped: value.get("stopped").and_then(Value::as_bool).unwrap_or_default(),
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
        if self.window == crate::window::WindowMode::Headed {
            out.push_str(&format!("\ncom a janela do app na tela, em {}", crate::window::this_system()));
            if self.slow_mo_ms > 0 {
                out.push_str(&format!(", {} ms entre os passos", self.slow_mo_ms));
            }
        }
        if self.stopped {
            out.push_str("\nparado por quem acompanhava antes do fim");
        }
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
        for film in &self.films {
            out.push_str(&format!("\nfilme: {film}"));
        }
        if let Some(note) = &self.film_note {
            out.push_str(&format!("\nsem filme: {note}"));
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
            "films": self.films,
            "film_note": self.film_note,
            "window": self.window.word(),
            "slow_mo_ms": self.slow_mo_ms,
            "platform": crate::window::this_system(),
            "stopped": self.stopped,
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
            "window": self.window.word(),
            "slow_mo_ms": self.slow_mo_ms,
            "platform": crate::window::this_system(),
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
            "<testsuites name=\"ravenrs-e2e\" tests=\"{tests}\" failures=\"{failures}\" skipped=\"{skipped}\" time=\"{}\">\n",
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
            // A build server shows the film next to the suite, as JUnit's attachments say.
            if !report.films.is_empty() {
                out.push_str("    <system-out>");
                for film in &report.films {
                    out.push_str(&xml(&format!("[[ATTACHMENT|{film}]]\n")));
                }
                out.push_str("</system-out>\n");
            }
            out.push_str("  </testsuite>\n");
        }
        out.push_str("</testsuites>\n");
        out
    }

    /// Keeps the run in `folder`, under a folder named after the scenario: `relatorio.json`,
    /// `relatorio.html`, `trace.jsonl` and, when it failed and the app's screen came with the failure,
    /// `arvore.txt`. A run that passes removes an `arvore.txt` left by an earlier failure.
    /// Says where it kept them.
    pub fn save(&self, folder: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        let dir = folder.join(folder_name(&self.scenario));
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(&self.json()).unwrap_or_default();
        std::fs::write(dir.join("relatorio.json"), json + "\n")?;
        std::fs::write(dir.join("trace.jsonl"), self.trace())?;
        std::fs::write(dir.join("relatorio.html"), crate::html::page_in(std::slice::from_ref(self), &dir))?;
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

/// Keeps each report in `out`, when given, and gives them all in `format` (one of [`FORMATS`];
/// anything else is text). The JSON is a list when `list`, else the one report. Where they were
/// kept goes to stderr, so the JSON, JSON lines or XML on stdout stays only that.
pub fn deliver(reports: &[Report], list: bool, format: &str, out: Option<&std::path::Path>) {
    if let Some(out) = out {
        for report in reports {
            match report.save(out) {
                Ok(dir) => eprintln!("{}: gravado em {}", report.scenario, dir.display()),
                Err(e) => eprintln!("{}: não gravou em {}: {e}", report.scenario, out.display()),
            }
        }
    }
    print!("{}", render(reports, list, format));
}

/// `reports` in `format`, as [`deliver`] prints them.
pub fn render(reports: &[Report], list: bool, format: &str) -> String {
    match format {
        "json" => {
            let all: Vec<_> = reports.iter().map(Report::json).collect();
            let value = match all.as_slice() {
                [one] if !list => one.clone(),
                _ => Value::Array(all),
            };
            serde_json::to_string_pretty(&value).unwrap_or_default() + "\n"
        }
        "jsonl" => reports.iter().map(Report::trace).collect(),
        "junit" => Report::junit(reports),
        "html" => crate::html::page(reports),
        _ => reports.iter().map(|r| format!("{}\n\n", r.text())).collect(),
    }
}

/// The JSON Schema (2020-12) of [`Report::json`], for a tool or an AI that reads it.
pub fn schema() -> Value {
    let text = json!({ "type": "string" });
    let maybe = json!({ "type": ["string", "null"] });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://github.com/alexkads/RavenRS/schemas/e2e-report.json",
        "title": "RavenRS e2e report",
        "description": "How a run of .raventest scenarios went, as Raven Test Studio and cargo ravenrs e2e give it with --format json.",
        "type": "object",
        "required": ["scenario", "verdict", "summary", "total", "ran", "blocks", "duration_ms"],
        "properties": {
            "scenario": text, "verdict": { "enum": ["passed", "failed"] }, "summary": text,
            "total": { "type": "integer", "minimum": 0, "description": "blocks on the scenario's path" },
            "ran": { "type": "integer", "minimum": 0 },
            "duration_ms": { "type": "integer", "minimum": 0 },
            "from_cache": { "type": "boolean" }, "cached_at": { "type": ["integer", "null"] },
            "opened_apart": { "type": "boolean" },
            "not_verified": { "type": "array", "items": text },
            "not_run": { "type": "array", "items": text, "description": "titles of the blocks after a failure" },
            "films": { "type": "array", "items": text, "description": "the .ravenmovie files kept of the apps the run drove" },
            "film_note": { "type": ["string", "null"], "description": "why an app was not filmed, when it was asked to be" },
            "window": { "enum": ["headless", "headed"], "description": "whether the apps showed their real window (--headed)" },
            "slow_mo_ms": { "type": "integer", "minimum": 0, "description": "how long the run waited after each step, for a person to follow it" },
            "platform": { "type": "string", "description": "the system the run ran on: macos, linux or windows" },
            "stopped": { "type": "boolean", "description": "the person watching stopped the run before its end" },
            "blocks": { "type": "array", "items": {
                "type": "object",
                "required": ["step", "id", "kind", "title", "line", "result"],
                "properties": {
                    "step": { "type": "integer", "minimum": 1 }, "id": text, "kind": text, "title": text, "line": text,
                    "result": { "enum": ["passed", "failed"] },
                    "code": { "type": ["string", "null"], "pattern": "^E2E_[A-Z_]+$", "description": "stable code of a failure" },
                    "error": maybe, "meaning": maybe, "detail": maybe, "fix": maybe,
                    "close": { "type": "array", "items": text },
                    "screen": { "type": ["string", "null"], "description": "the app's screen as text when it failed" },
                    "duration_ms": { "type": "integer", "minimum": 0 }
                }
            }}
        }
    })
}
