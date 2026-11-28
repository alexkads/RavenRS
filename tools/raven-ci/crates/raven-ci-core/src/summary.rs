//! The summary of a build for programs and sessions of AI (#941): what a build is and, when it
//! failed, why, in a few kilobytes, so whoever reads it does not read the whole log.
//!
//! # Its role
//!
//! A session of AI that asks for a build and reads its whole log spends thousands of tokens on
//! `Compiling …` lines to find the three that matter. The summary is those three: where the build
//! is, which stage and step failed with the last lines that step wrote, the tests that broke with
//! their messages, the diagnostics of `cargo` with file and line, and what the build kept.
//!
//! # How it works
//!
//! [`Ci::summary`] reads what the core already keeps, with no plugin:
//!
//! - **The build** ([`Ci::build`]): status, cause, who asked, commit, reason, duration.
//! - **The stages** ([`Ci::stages`]): the first step that failed, and the last [`TAIL_LINES`] lines
//!   of the log it wrote, every secret already hidden by the log. A build that failed before any
//!   step (fetching the code, an agent that never came) gives the last lines of the core instead.
//! - **The attachments**, by kind: `junit` (the `testes.json` of the plugin `junit`, suites of
//!   `ravenrs-junit`) gives the count and the tests that failed with their message; `cargo` (the
//!   `cargo-diagnostics.json` of the plugin `cargo`) gives the errors and warnings with file and
//!   line, errors first; every other kind is listed by name and size, with its id to download.
//! - **The size**: a line is cut at [`LINE_CHARS`] characters and the lists at a few items, and
//!   the whole JSON stays under [`MOST_BYTES`]: when it would pass, the lists are cut further and
//!   `truncated` says so.
//!
//! [`to_markdown`] writes the same as a `resumo.md` for a pull request, a person or a session
//! that prefers text.

use raven_ci_plugin_api::{BuildStatus, CiError};
use serde::Serialize;
use serde_json::Value;

use crate::Ci;

/// How many of the last lines of the step that failed the summary keeps.
pub const TAIL_LINES: usize = 30;
/// How many characters of a line the summary keeps.
pub const LINE_CHARS: usize = 300;
/// The most the summary's JSON holds, in bytes.
pub const MOST_BYTES: usize = 8 * 1024;
/// How many tests that failed, and diagnostics, it names at first.
const MOST_ITEMS: usize = 20;

/// The summary of a build. See the [module](self).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BuildSummary {
    /// The job.
    pub job: String,
    /// The build's number.
    pub number: u64,
    /// Where it is: `succeeded`, `failed`, `running`…
    pub status: BuildStatus,
    /// The same in words: `passou`, `falhou`.
    pub status_words: String,
    /// Why it was asked for.
    pub cause: String,
    /// Who asked.
    pub actor: Option<String>,
    /// The commit it fetched.
    pub commit: Option<String>,
    /// How long it took, or has taken so far, in milliseconds.
    pub duration_ms: Option<i64>,
    /// Why it did not pass, as the core says it.
    pub reason: Option<String>,
    /// Each stage, with its status.
    pub stages: Vec<StageLine>,
    /// The step that failed, with its last lines.
    pub failed: Option<FailedStep>,
    /// The tests, when the build kept a report of them.
    pub tests: Option<Tests>,
    /// The diagnostics of `cargo`, when the build kept them.
    pub diagnostics: Option<Diagnostics>,
    /// What else the build kept, to download by id.
    pub attachments: Vec<AttachmentLine>,
    /// Some list was cut to keep the summary small.
    pub truncated: bool,
}

/// A stage and where it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StageLine {
    /// Its name.
    pub name: String,
    /// `done`, `failed`, `running`, `pending`.
    pub status: String,
}

/// The step that failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FailedStep {
    /// Its stage.
    pub stage: String,
    /// Its node in the flow: what the log calls it.
    pub step: String,
    /// What it is called.
    pub title: String,
    /// Why, as the step said.
    pub error: Option<String>,
    /// The last lines it wrote, oldest first, with no noise.
    pub tail: Vec<String>,
    /// How many lines of noise it wrote that the tail leaves out: `Compiling …`, `test … ok`.
    pub noise_left_out: usize,
}

/// The count of the tests and the ones that failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Tests {
    /// How many ran.
    pub tests: u64,
    /// How many failed an assertion.
    pub failures: u64,
    /// How many could not run to the end.
    pub errors: u64,
    /// How many were skipped.
    pub skipped: u64,
    /// The ones that failed or errored, with their message.
    pub failed: Vec<FailedTest>,
}

/// A test that failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FailedTest {
    /// `classe::nome`.
    pub name: String,
    /// What the failure says, in one line.
    pub message: String,
}

/// The diagnostics of `cargo`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostics {
    /// How many errors.
    pub errors: u64,
    /// How many warnings.
    pub warnings: u64,
    /// The first of them, errors first.
    pub first: Vec<Diagnostic>,
}

/// One diagnostic of `cargo`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// `error` or `warning`.
    pub level: String,
    /// `E0308`, `clippy::needless_return`.
    pub code: Option<String>,
    /// `src/lib.rs:12:5`.
    pub place: Option<String>,
    /// What it says.
    pub message: String,
}

/// Something the build kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AttachmentLine {
    /// Its id: `GET /api/ci/attachment?id=`.
    pub id: i64,
    /// Its kind: `artifact`, `junit`.
    pub kind: String,
    /// Its name.
    pub name: String,
    /// Its size in bytes.
    pub size: u64,
}

/// `text` without the codes of color of a terminal: `ESC[1m`, and the `[1m` left of one whose
/// `ESC` was taken out. They are many bytes and say nothing to whoever reads the summary.
fn without_colors(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut n = 0;
    while n < chars.len() {
        let start = if chars[n] == '\u{1b}' && chars.get(n + 1) == Some(&'[') { n + 1 } else { n };
        if chars[start] == '[' {
            let mut end = start + 1;
            while end < chars.len() && end - start <= 12 && (chars[end].is_ascii_digit() || chars[end] == ';')
            {
                end += 1;
            }
            if end < chars.len() && chars[end] == 'm' && (end > start + 1 || start > n) {
                n = end + 1;
                continue;
            }
        }
        if chars[n] != '\u{1b}' {
            out.push(chars[n]);
        }
        n += 1;
    }
    out
}

/// Whether `line` says nothing about why a step failed: progress of a build, a test that passed,
/// a blank line. The tail leaves them out, so the lines that matter fit.
fn noise(line: &str) -> bool {
    let line = without_colors(line);
    let line = line.trim();
    const PROGRESS: [&str; 9] = [
        "Compiling ",
        "Checking ",
        "Downloaded ",
        "Downloading ",
        "Fresh ",
        "Locking ",
        "Adding ",
        "Updating ",
        "Blocking waiting",
    ];
    line.is_empty()
        || PROGRESS.iter().any(|start| line.starts_with(start))
        || (line.starts_with("test ") && (line.ends_with("... ok") || line.ends_with("... ignored")))
}

/// `text` with no colors, cut at [`LINE_CHARS`] characters, with `…` when cut.
fn cut(text: &str) -> String {
    let text = without_colors(text);
    let text = text.trim_end();
    if text.chars().count() <= LINE_CHARS {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(LINE_CHARS).collect();
    out.push('…');
    out
}

fn text_of(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}

/// The tests of a `testes.json` (suites of `ravenrs-junit`).
fn tests_of(value: &Value) -> Tests {
    let mut tests = Tests { tests: 0, failures: 0, errors: 0, skipped: 0, failed: Vec::new() };
    let suites = value.get("suites").and_then(Value::as_array).cloned().unwrap_or_default();
    for case in suites
        .iter()
        .flat_map(|suite| suite.get("cases").and_then(Value::as_array).cloned().unwrap_or_default())
    {
        tests.tests += 1;
        let name = format!(
            "{}::{}",
            text_of(&case, "classname").unwrap_or_default(),
            text_of(&case, "name").unwrap_or_default()
        );
        let outcome = case.get("outcome").cloned().unwrap_or(Value::Null);
        let problem = match &outcome {
            Value::Object(object) if object.contains_key("failure") => {
                tests.failures += 1;
                object.get("failure")
            }
            Value::Object(object) if object.contains_key("error") => {
                tests.errors += 1;
                object.get("error")
            }
            Value::Object(object) if object.contains_key("skipped") => {
                tests.skipped += 1;
                None
            }
            _ => None,
        };
        if let Some(problem) = problem {
            let message = text_of(problem, "message")
                .filter(|m| !m.trim().is_empty())
                .or_else(|| {
                    text_of(problem, "text")
                        .and_then(|t| t.lines().find(|l| !l.trim().is_empty()).map(str::to_owned))
                })
                .unwrap_or_default();
            tests.failed.push(FailedTest { name: cut(&name), message: cut(&message) });
        }
    }
    tests
}

/// The diagnostics of a `cargo-diagnostics.json`, errors first.
fn diagnostics_of(value: &Value) -> Diagnostics {
    let all = value.as_array().cloned().unwrap_or_default();
    let level = |d: &Value| text_of(d, "level").unwrap_or_default().to_lowercase();
    let errors = all.iter().filter(|d| level(d) == "error").count() as u64;
    let warnings = all.iter().filter(|d| level(d) == "warning").count() as u64;
    let mut sorted: Vec<&Value> = all.iter().collect();
    sorted.sort_by_key(|d| u8::from(level(d) != "error"));
    // `cargo test` builds the library and its tests: the same diagnostic comes twice.
    let mut seen = std::collections::BTreeSet::new();
    sorted.retain(|d| {
        seen.insert((
            level(d),
            text_of(d, "code"),
            text_of(d, "file"),
            d.get("line").and_then(Value::as_u64),
            text_of(d, "message"),
        ))
    });
    let first = sorted
        .into_iter()
        .map(|d| {
            let place = text_of(d, "file").map(|file| {
                let mut place = file;
                if let Some(line) = d.get("line").and_then(Value::as_u64) {
                    place.push_str(&format!(":{line}"));
                    if let Some(column) = d.get("column").and_then(Value::as_u64) {
                        place.push_str(&format!(":{column}"));
                    }
                }
                place
            });
            Diagnostic {
                level: level(d),
                code: text_of(d, "code"),
                place,
                message: cut(&text_of(d, "message").unwrap_or_default()),
            }
        })
        .collect();
    Diagnostics { errors, warnings, first }
}

impl BuildSummary {
    fn size(&self) -> usize {
        serde_json::to_vec(self).map_or(0, |bytes| bytes.len())
    }

    /// Cuts the lists, a little at a time, until the JSON is under [`MOST_BYTES`].
    fn fit(&mut self) {
        let mut most = MOST_ITEMS;
        loop {
            let mut cut_any = false;
            if let Some(tests) = &mut self.tests
                && tests.failed.len() > most
            {
                tests.failed.truncate(most);
                cut_any = true;
            }
            if let Some(diagnostics) = &mut self.diagnostics
                && diagnostics.first.len() > most
            {
                diagnostics.first.truncate(most);
                cut_any = true;
            }
            if self.attachments.len() > most {
                self.attachments.truncate(most);
                cut_any = true;
            }
            self.truncated |= cut_any;
            if self.size() <= MOST_BYTES {
                return;
            }
            if most == 0 {
                // Only the tail is left to cut: from the oldest line.
                if let Some(failed) = &mut self.failed
                    && failed.tail.len() > 3
                {
                    failed.tail.remove(0);
                    self.truncated = true;
                    continue;
                }
                return;
            }
            most /= 2;
        }
    }
}

impl Ci {
    /// The summary of the build `number` of `job`: see the [module](crate::summary).
    pub fn summary(&self, job: &str, number: u64) -> Result<BuildSummary, CiError> {
        let build = self.build(job, number)?;
        let stages = self.stages(job, number)?;
        let log = self.build_log(job, number, 0)?;
        let started = log.first().map(|line| line.at_ms);
        let duration_ms = started.map(|at| build.ended_ms.unwrap_or_else(|| self.store.now_ms()) - at);
        let failed = stages.iter().find_map(|stage| {
            stage
                .steps
                .iter()
                .find(|step| step.status == "failed")
                .map(|step| (stage.name.clone(), step.clone()))
        });
        let tail_of = |step: &str| -> (Vec<String>, usize) {
            let all: Vec<&str> =
                log.iter().filter(|line| line.step == step).map(|line| line.text.as_str()).collect();
            let lines: Vec<&str> = all.iter().copied().filter(|line| !noise(line)).collect();
            let tail = lines[lines.len().saturating_sub(TAIL_LINES)..].iter().map(|line| cut(line)).collect();
            (tail, all.len() - lines.len())
        };
        let failed = match failed {
            Some((stage, step)) => {
                let (tail, noise_left_out) = tail_of(&step.node);
                Some(FailedStep {
                    stage,
                    tail,
                    noise_left_out,
                    step: step.node,
                    title: step.title,
                    error: step.error.as_deref().map(cut),
                })
            }
            // Failed before any step: what the core said.
            None if build.status == BuildStatus::Failed => {
                let (tail, noise_left_out) = tail_of("");
                Some(FailedStep {
                    stage: String::new(),
                    step: String::new(),
                    title: "antes dos passos".to_owned(),
                    error: build.reason.as_deref().map(cut),
                    tail,
                    noise_left_out,
                })
            }
            None => None,
        };
        let mut summary = BuildSummary {
            job: build.job.clone(),
            number: build.number,
            status: build.status,
            status_words: build.status.words().to_owned(),
            cause: build.cause.clone(),
            actor: build.actor.clone(),
            commit: build.commit.clone(),
            duration_ms,
            reason: build.reason.as_deref().map(cut),
            stages: stages
                .iter()
                .map(|s| StageLine { name: s.name.clone(), status: s.status.clone() })
                .collect(),
            failed,
            tests: None,
            diagnostics: None,
            attachments: Vec::new(),
            truncated: false,
        };
        for attachment in self.attachments(job, number)? {
            let read = || {
                self.attachment_bytes(attachment.id)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            };
            match attachment.kind.as_str() {
                "junit" => summary.tests = read().map(|value| tests_of(&value)),
                "cargo" => summary.diagnostics = read().map(|value| diagnostics_of(&value)),
                _ => {}
            }
            summary.attachments.push(AttachmentLine {
                id: attachment.id,
                kind: attachment.kind,
                name: attachment.name,
                size: attachment.size,
            });
        }
        summary.fit();
        Ok(summary)
    }
}

/// The summary as Markdown: the `resumo.md` that goes with a pull request.
pub fn to_markdown(summary: &BuildSummary) -> String {
    let mut out = format!("# {} #{}: {}\n\n", summary.job, summary.number, summary.status_words);
    let mut facts = vec![format!("- **Causa:** {}", summary.cause)];
    if let Some(actor) = &summary.actor {
        facts.push(format!("- **Quem pediu:** {actor}"));
    }
    if let Some(commit) = &summary.commit {
        facts.push(format!("- **Commit:** `{commit}`"));
    }
    if let Some(ms) = summary.duration_ms {
        facts.push(format!("- **Duração:** {} s", ms / 1000));
    }
    if let Some(reason) = &summary.reason {
        facts.push(format!("- **Motivo:** {reason}"));
    }
    out.push_str(&facts.join("\n"));
    out.push_str("\n\n## Estágios\n\n");
    for stage in &summary.stages {
        let mark = match stage.status.as_str() {
            "done" => "✅",
            "failed" => "❌",
            "running" => "⏳",
            _ => "·",
        };
        out.push_str(&format!("- {mark} {} ({})\n", stage.name, stage.status));
    }
    if let Some(failed) = &summary.failed {
        let at = if failed.stage.is_empty() {
            failed.title.clone()
        } else {
            format!("{} › {}", failed.stage, failed.title)
        };
        out.push_str(&format!("\n## O que falhou: {at}\n\n"));
        if let Some(error) = &failed.error {
            out.push_str(&format!("{error}\n\n"));
        }
        if failed.noise_left_out > 0 {
            out.push_str(&format!(
                "_As últimas linhas, sem {} de progresso e testes que passaram:_\n\n",
                failed.noise_left_out
            ));
        }
        if !failed.tail.is_empty() {
            out.push_str("```text\n");
            for line in &failed.tail {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("```\n");
        }
    }
    if let Some(tests) = &summary.tests {
        out.push_str(&format!(
            "\n## Testes\n\n{} rodaram: {} falharam, {} com erro, {} pulados.\n",
            tests.tests, tests.failures, tests.errors, tests.skipped
        ));
        for test in &tests.failed {
            out.push_str(&format!("\n- `{}`: {}", test.name, test.message));
        }
        if !tests.failed.is_empty() {
            out.push('\n');
        }
    }
    if let Some(diagnostics) = &summary.diagnostics {
        out.push_str(&format!(
            "\n## Diagnósticos do cargo\n\n{} erros, {} avisos.\n",
            diagnostics.errors, diagnostics.warnings
        ));
        for d in &diagnostics.first {
            let code = d.code.as_deref().map(|c| format!("[{c}] ")).unwrap_or_default();
            let place = d.place.as_deref().map(|p| format!("`{p}` ")).unwrap_or_default();
            out.push_str(&format!("\n- {}: {place}{code}{}", d.level, d.message));
        }
        if !diagnostics.first.is_empty() {
            out.push('\n');
        }
    }
    if !summary.attachments.is_empty() {
        out.push_str("\n## O que o build guardou\n\n");
        for attachment in &summary.attachments {
            out.push_str(&format!(
                "- {} ({}, {} bytes, id {})\n",
                attachment.name, attachment.kind, attachment.size, attachment.id
            ));
        }
    }
    if summary.truncated {
        out.push_str("\n_O resumo foi cortado para caber; o log inteiro e os anexos têm o resto._\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_tests_of_a_report_give_the_count_and_the_message_of_each_failure() {
        let report = json!({ "suites": [{ "name": "s", "cases": [
            { "classname": "pedidos", "name": "cria", "outcome": "passed" },
            { "classname": "pedidos", "name": "paga", "outcome": { "failure": { "kind": null, "message": "esperava 2, veio 3", "text": "" } } },
            { "classname": "pedidos", "name": "cai", "outcome": { "error": { "kind": null, "message": null, "text": "\npanicked at x" } } },
            { "classname": "pedidos", "name": "pula", "outcome": { "skipped": { "message": null } } }
        ]}]});
        let tests = tests_of(&report);
        assert_eq!((tests.tests, tests.failures, tests.errors, tests.skipped), (4, 1, 1, 1));
        assert_eq!(
            tests.failed[0],
            FailedTest { name: "pedidos::paga".into(), message: "esperava 2, veio 3".into() }
        );
        assert_eq!(tests.failed[1].message, "panicked at x");
    }

    #[test]
    fn the_diagnostics_come_errors_first_with_their_place() {
        let found = json!([
            { "step": "s", "level": "warning", "code": "unused", "message": "aviso", "file": "a.rs", "line": 1, "column": 2 },
            { "step": "s", "level": "error", "code": "E0308", "message": "tipos", "file": "b.rs", "line": 9, "column": 5 }
        ]);
        let diagnostics = diagnostics_of(&found);
        assert_eq!((diagnostics.errors, diagnostics.warnings), (1, 1));
        assert_eq!(diagnostics.first[0].place.as_deref(), Some("b.rs:9:5"));
        assert_eq!(diagnostics.first[0].code.as_deref(), Some("E0308"));
    }

    #[test]
    fn the_codes_of_color_go_and_the_text_stays() {
        assert_eq!(without_colors("\u{1b}[1m\u{1b}[94m|\u{1b}[0m texto"), "| texto");
        assert_eq!(without_colors("[1m[33m^^^^^[0m ajuda"), "^^^^^ ajuda");
        assert_eq!(without_colors("vetor[3] e [m] ficam"), "vetor[3] e [m] ficam");
    }

    #[test]
    fn progress_and_tests_that_passed_are_noise_and_failures_are_not() {
        for line in [
            "   Compiling serde v1.0.228",
            "test testes::soma_1 ... ok",
            "",
            "\u{1b}[1m\u{1b}[32m   Compiling\u{1b}[0m x",
        ] {
            assert!(noise(line), "{line:?}");
        }
        for line in [
            "test testes::soma_errada ... FAILED",
            "thread 'x' panicked at src/lib.rs:6:32:",
            "error[E0308]: tipos",
        ] {
            assert!(!noise(line), "{line:?}");
        }
    }

    #[test]
    fn the_same_diagnostic_twice_is_named_once() {
        let twice = json!([
            { "level": "warning", "code": "unused", "message": "aviso", "file": "a.rs", "line": 1 },
            { "level": "warning", "code": "unused", "message": "aviso", "file": "a.rs", "line": 1 }
        ]);
        assert_eq!(diagnostics_of(&twice).first.len(), 1);
    }

    #[test]
    fn a_long_line_is_cut_and_says_so() {
        let long = "x".repeat(LINE_CHARS + 10);
        let short = cut(&long);
        assert_eq!(short.chars().count(), LINE_CHARS + 1);
        assert!(short.ends_with('…'));
        assert_eq!(cut("curta  "), "curta");
    }
}
