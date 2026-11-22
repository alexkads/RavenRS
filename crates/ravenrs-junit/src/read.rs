//! The reader: JUnit XML as [`Suites`].

use std::fmt;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{Case, Outcome, Problem, Suite, Suites};

/// An XML that could not be read as JUnit, with where it broke.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// The byte where reading stopped.
    pub position: u64,
    /// What was wrong, in words.
    pub reason: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JUnit ilegível no byte {}: {}", self.position, self.reason)
    }
}

impl std::error::Error for Error {}

/// The value of the attribute `name` of `tag`, unescaped.
fn attribute(tag: &BytesStart<'_>, name: &str) -> Option<String> {
    let found = tag.attributes().flatten().find(|a| a.key.local_name().as_ref() == name.as_bytes())?;
    let raw = String::from_utf8_lossy(&found.value).into_owned();
    Some(quick_xml::escape::unescape(&raw).map_or(raw.clone(), |text| text.into_owned()))
}

/// Seconds with a fraction (`1.5`, `0.003`, `1,5` as some locales write it) as milliseconds.
fn milliseconds(text: &str) -> Option<u64> {
    let seconds: f64 = text.trim().replace(',', ".").parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then(|| (seconds * 1000.0).round() as u64)
}

/// Where a text being read goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Nothing,
    Problem,
    SuiteOut,
}

/// Reads JUnit XML: a `<testsuites>` or a lone `<testsuite>`. A suite inside another is kept in
/// the same list, after its parent; cases outside any suite go into one named after the run.
pub fn read(xml: &str) -> Result<Suites, Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut suites = Suites::default();
    // The suites open, innermost last; each is pushed to `suites` when it closes, in the order
    // they opened.
    let mut open: Vec<(usize, Suite)> = Vec::new();
    let mut order = 0;
    let mut closed: Vec<(usize, Suite)> = Vec::new();
    let mut case: Option<Case> = None;
    let mut into = Target::Nothing;
    let mut text = String::new();
    let mut seen_root = false;
    let fail = |reader: &Reader<&[u8]>, reason: String| Error { position: reader.buffer_position(), reason };
    loop {
        let event = reader.read_event().map_err(|e| fail(&reader, e.to_string()))?;
        match &event {
            Event::Start(tag) | Event::Empty(tag) => {
                let empty = matches!(event, Event::Empty(_));
                let local = tag.local_name();
                match local.as_ref() {
                    b"testsuites" => {
                        seen_root = true;
                        suites.name = attribute(tag, "name");
                    }
                    b"testsuite" => {
                        seen_root = true;
                        let suite = Suite {
                            name: attribute(tag, "name").unwrap_or_default(),
                            time_ms: attribute(tag, "time").and_then(|t| milliseconds(&t)),
                            ..Suite::default()
                        };
                        if empty {
                            closed.push((order, suite));
                        } else {
                            open.push((order, suite));
                        }
                        order += 1;
                    }
                    b"testcase" => {
                        let classname = attribute(tag, "classname")
                            .or_else(|| open.last().map(|(_, s)| s.name.clone()))
                            .unwrap_or_default();
                        let made = Case {
                            classname,
                            name: attribute(tag, "name").unwrap_or_default(),
                            time_ms: attribute(tag, "time").and_then(|t| milliseconds(&t)),
                            outcome: Outcome::Passed,
                        };
                        if empty {
                            push_case(&mut open, &mut suites, made);
                        } else {
                            case = Some(made);
                        }
                    }
                    which @ (b"failure" | b"error") if case.is_some() => {
                        let problem = Problem {
                            kind: attribute(tag, "type"),
                            message: attribute(tag, "message"),
                            text: String::new(),
                        };
                        if let Some(case) = &mut case {
                            case.outcome = if which == b"failure" {
                                Outcome::Failure(problem)
                            } else {
                                Outcome::Error(problem)
                            };
                        }
                        if !empty {
                            into = Target::Problem;
                            text.clear();
                        }
                    }
                    b"skipped" if case.is_some() => {
                        if let Some(case) = &mut case {
                            case.outcome = Outcome::Skipped { message: attribute(tag, "message") };
                        }
                    }
                    b"system-out" if case.is_none() && !open.is_empty() && !empty => {
                        into = Target::SuiteOut;
                        text.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(t) if into != Target::Nothing => {
                text.push_str(&t.decode().map_err(|e| fail(&reader, e.to_string()))?);
            }
            Event::CData(t) if into != Target::Nothing => {
                text.push_str(&t.decode().map_err(|e| fail(&reader, e.to_string()))?);
            }
            Event::GeneralRef(r) if into != Target::Nothing => {
                let name = r.decode().map_err(|e| fail(&reader, e.to_string()))?;
                let entity = format!("&{name};");
                let resolved = quick_xml::escape::unescape(&entity)
                    .map_err(|e| fail(&reader, format!("{entity}: {e}")))?
                    .into_owned();
                text.push_str(&resolved);
            }
            Event::End(tag) => match tag.local_name().as_ref() {
                b"failure" | b"error" if into == Target::Problem => {
                    if let Some(Case {
                        outcome: Outcome::Failure(problem) | Outcome::Error(problem), ..
                    }) = &mut case
                    {
                        problem.text = std::mem::take(&mut text);
                    }
                    into = Target::Nothing;
                }
                b"system-out" if into == Target::SuiteOut => {
                    if let Some((_, suite)) = open.last_mut() {
                        suite.system_out = Some(std::mem::take(&mut text));
                    }
                    into = Target::Nothing;
                }
                b"testcase" => {
                    if let Some(made) = case.take() {
                        push_case(&mut open, &mut suites, made);
                    }
                }
                b"testsuite" => {
                    if let Some(suite) = open.pop() {
                        closed.push(suite);
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    if !open.is_empty() || case.is_some() {
        return Err(fail(&reader, "o XML terminou com um testsuite ou testcase aberto".to_owned()));
    }
    if !seen_root && suites.suites.is_empty() {
        return Err(fail(&reader, "não há testsuites nem testsuite".to_owned()));
    }
    closed.sort_by_key(|(order, _)| *order);
    // Cases outside any suite were put in a suite of their own, first.
    let loose = std::mem::take(&mut suites.suites);
    suites.suites = loose.into_iter().chain(closed.into_iter().map(|(_, suite)| suite)).collect();
    Ok(suites)
}

/// Puts `case` in the suite open innermost, or in a suite of cases outside any suite.
fn push_case(open: &mut [(usize, Suite)], suites: &mut Suites, case: Case) {
    match open.last_mut() {
        Some((_, suite)) => suite.cases.push(case),
        None => {
            if suites.suites.is_empty() {
                suites.suites.push(Suite::new(suites.name.clone().unwrap_or_default()));
            }
            if let Some(suite) = suites.suites.first_mut() {
                suite.cases.push(case);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_become_milliseconds() {
        assert_eq!(milliseconds("1.5"), Some(1500));
        assert_eq!(milliseconds("0,003"), Some(3));
        assert_eq!(milliseconds(" 2 "), Some(2000));
        assert_eq!(milliseconds("-1"), None);
        assert_eq!(milliseconds("x"), None);
    }
}
