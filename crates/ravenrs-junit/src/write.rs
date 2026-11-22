//! The writer: [`Suites`] as JUnit XML.

use std::fmt::Write as _;

use crate::{Case, Outcome, Problem, Suite, Suites, Summary};

/// `suites` as JUnit XML, two spaces of indent, counts worked out from the cases.
pub(crate) fn suites(suites: &Suites) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let total = suites.summary();
    out.push_str("<testsuites");
    if let Some(name) = &suites.name {
        let _ = write!(out, " name=\"{}\"", escape(name));
    }
    counts(&mut out, &total);
    out.push_str(">\n");
    for suite in &suites.suites {
        write_suite(&mut out, suite);
    }
    out.push_str("</testsuites>\n");
    out
}

/// ` tests=… failures=… [errors=…] skipped=… time=…`; `errors` only when there are some, as most
/// tools leave it out then.
fn counts(out: &mut String, summary: &Summary) {
    let _ = write!(out, " tests=\"{}\" failures=\"{}\"", summary.tests, summary.failures);
    if summary.errors > 0 {
        let _ = write!(out, " errors=\"{}\"", summary.errors);
    }
    let _ = write!(out, " skipped=\"{}\" time=\"{}\"", summary.skipped, seconds(summary.time_ms));
}

fn write_suite(out: &mut String, suite: &Suite) {
    let _ = write!(out, "  <testsuite name=\"{}\"", escape(&suite.name));
    counts(out, &suite.summary());
    out.push_str(">\n");
    for case in &suite.cases {
        write_case(out, case);
    }
    if let Some(text) = &suite.system_out {
        let _ = writeln!(out, "    <system-out>{}</system-out>", escape(text));
    }
    out.push_str("  </testsuite>\n");
}

fn write_case(out: &mut String, case: &Case) {
    let time = case.time_ms.map_or_else(|| "0".to_owned(), seconds);
    let open = format!(
        "    <testcase classname=\"{}\" name=\"{}\" time=\"{time}\"",
        escape(&case.classname),
        escape(&case.name)
    );
    match &case.outcome {
        Outcome::Passed => {
            let _ = writeln!(out, "{open}/>");
        }
        Outcome::Failure(problem) => write_problem(out, &open, "failure", problem),
        Outcome::Error(problem) => write_problem(out, &open, "error", problem),
        Outcome::Skipped { message } => {
            let _ = writeln!(out, "{open}>");
            match message {
                Some(message) => {
                    let _ = writeln!(out, "      <skipped message=\"{}\"/>", escape(message));
                }
                None => out.push_str("      <skipped/>\n"),
            }
            out.push_str("    </testcase>\n");
        }
    }
}

fn write_problem(out: &mut String, open: &str, element: &str, problem: &Problem) {
    let _ = writeln!(out, "{open}>");
    let _ = write!(out, "      <{element}");
    if let Some(kind) = &problem.kind {
        let _ = write!(out, " type=\"{}\"", escape(kind));
    }
    if let Some(message) = &problem.message {
        let _ = write!(out, " message=\"{}\"", escape(message));
    }
    if problem.text.is_empty() {
        out.push_str("/>\n");
    } else {
        let _ = writeln!(out, ">{}</{element}>", escape(&problem.text));
    }
    out.push_str("    </testcase>\n");
}

/// Text that goes inside an XML attribute or element. Control characters other than tab and line
/// breaks are not allowed in XML 1.0, and are left out.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// Milliseconds as the seconds JUnit writes: `1.250`.
pub(crate) fn seconds(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
