//! JUnit XML, both ways: what test tools write and build servers read.
//!
//! # Its role
//!
//! A test run says what passed and what failed as JUnit XML: Maven Surefire, pytest, `cargo
//! nextest`, Jest and the end-to-end runner of RavenRS (`ravenrs-e2e-runner`) all write it, and a
//! CI (Raven CI's plugin `junit`) reads it to show the count and the tests that broke. This crate
//! is the one place that knows the format, for writing and for reading, so the two never drift.
//!
//! # How it works
//!
//! - **The model** is [`Suites`] → [`Suite`] → [`Case`], each case with its [`Outcome`]: passed,
//!   a failure (an assertion), an error (it could not run) or skipped. Times are milliseconds.
//! - **[`read`]** takes a `<testsuites>` or a lone `<testsuite>`, suites inside suites (kept as one
//!   list, in the order they come), the text of a failure in elements or in CDATA, and `time` in
//!   seconds with a fraction. Unknown elements and attributes are passed over; an XML that is
//!   not well formed is an [`Error`] with where it broke.
//! - **[`Suites::to_xml`]** writes it back, escaped for XML 1.0 (control characters, which XML 1.0
//!   does not allow, are left out). The counts of each suite are worked out from its cases.
//! - **[`Summary`]** is what a screen shows at a glance: the counts, the time and the name of each
//!   test that failed or errored.
//!
//! # How to use it
//!
//! ```
//! use ravenrs_junit::{Case, Outcome, Suite, Suites, read};
//!
//! let suites = Suites::named("loja").suite(
//!     Suite::new("carrinho")
//!         .case(Case::new("carrinho", "soma os itens").time_ms(12))
//!         .case(Case::new("carrinho", "aplica o cupom").outcome(Outcome::failure("esperava 90, veio 100"))),
//! );
//! let xml = suites.to_xml();
//! let summary = read(&xml).expect("reads what it writes").summary();
//! assert_eq!((summary.tests, summary.failures), (2, 1));
//! assert_eq!(summary.failed, ["carrinho.aplica o cupom"]);
//! ```

mod read;
mod write;

pub use read::{Error, read};
use serde::{Deserialize, Serialize};

/// A run of tests: every suite of one or more files.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suites {
    /// The name of the run, written on `<testsuites>` when there is one.
    pub name: Option<String>,
    /// Its suites, in order.
    pub suites: Vec<Suite>,
}

/// A suite: a class, a file, a scenario.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suite {
    /// Its name.
    pub name: String,
    /// How long it took, when said. Read, it is the suite's `time`; written, the sum of its cases
    /// when not said.
    pub time_ms: Option<u64>,
    /// Its cases, in order.
    pub cases: Vec<Case>,
    /// What it wrote to its output (`<system-out>`), where build servers find attachments.
    pub system_out: Option<String>,
}

/// One test.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    /// Its class, or the suite it belongs to.
    pub classname: String,
    /// Its name.
    pub name: String,
    /// How long it took, when measured; a case not measured (one skipped) is written with time 0.
    pub time_ms: Option<u64>,
    /// How it ended.
    pub outcome: Outcome,
}

/// How a test ended.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// It passed.
    #[default]
    Passed,
    /// An assertion did not hold.
    Failure(Problem),
    /// It could not run to the end: an exception, a crash.
    Error(Problem),
    /// It was not run.
    Skipped {
        /// Why, when said.
        message: Option<String>,
    },
}

/// What went wrong in a failure or an error.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    /// Its type, as the tool names it: `AssertionError`, a stable code.
    pub kind: Option<String>,
    /// The line that says it.
    pub message: Option<String>,
    /// The rest: the stack, the details.
    pub text: String,
}

impl Outcome {
    /// A failure saying `message`.
    pub fn failure(message: impl Into<String>) -> Self {
        Self::Failure(Problem { kind: None, message: Some(message.into()), text: String::new() })
    }

    /// An error saying `message`.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(Problem { kind: None, message: Some(message.into()), text: String::new() })
    }

    /// Skipped, because of `message`.
    pub fn skipped(message: impl Into<String>) -> Self {
        Self::Skipped { message: Some(message.into()) }
    }
}

impl Suites {
    /// A run named `name`.
    pub fn named(name: impl Into<String>) -> Self {
        Self { name: Some(name.into()), suites: Vec::new() }
    }

    /// With `suite` at the end.
    pub fn suite(mut self, suite: Suite) -> Self {
        self.suites.push(suite);
        self
    }

    /// Every case of every suite, in order.
    pub fn cases(&self) -> impl Iterator<Item = &Case> {
        self.suites.iter().flat_map(|suite| suite.cases.iter())
    }

    /// The counts and the names of what failed.
    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        for suite in &self.suites {
            summary.add(&suite.summary());
        }
        summary
    }

    /// As JUnit XML.
    pub fn to_xml(&self) -> String {
        write::suites(self)
    }
}

impl Suite {
    /// The suite `name`, empty.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }

    /// With `case` at the end.
    pub fn case(mut self, case: Case) -> Self {
        self.cases.push(case);
        self
    }

    /// Having taken `ms`.
    pub fn time_ms(mut self, ms: u64) -> Self {
        self.time_ms = Some(ms);
        self
    }

    /// With `text` as its output.
    pub fn system_out(mut self, text: impl Into<String>) -> Self {
        self.system_out = Some(text.into());
        self
    }

    /// Its counts and the names of what failed in it.
    pub fn summary(&self) -> Summary {
        let mut summary = Summary { time_ms: self.time_ms.unwrap_or(0), ..Summary::default() };
        let mut cases_ms = 0;
        for case in &self.cases {
            summary.tests += 1;
            cases_ms += case.time_ms.unwrap_or(0);
            match &case.outcome {
                Outcome::Passed => {}
                Outcome::Failure(_) => {
                    summary.failures += 1;
                    summary.failed.push(case.full_name());
                }
                Outcome::Error(_) => {
                    summary.errors += 1;
                    summary.failed.push(case.full_name());
                }
                Outcome::Skipped { .. } => summary.skipped += 1,
            }
        }
        if self.time_ms.is_none() {
            summary.time_ms = cases_ms;
        }
        summary
    }
}

impl Case {
    /// The case `name` of `classname`, passed.
    pub fn new(classname: impl Into<String>, name: impl Into<String>) -> Self {
        Self { classname: classname.into(), name: name.into(), ..Self::default() }
    }

    /// Ended as `outcome`.
    pub fn outcome(mut self, outcome: Outcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Having taken `ms`.
    pub fn time_ms(mut self, ms: u64) -> Self {
        self.time_ms = Some(ms);
        self
    }

    /// `classname.name`, or the name alone when there is no class or the name already starts
    /// with it: what a list of what failed shows.
    pub fn full_name(&self) -> String {
        if self.classname.is_empty() || self.name.starts_with(&self.classname) {
            self.name.clone()
        } else {
            format!("{}.{}", self.classname, self.name)
        }
    }
}

/// What a run says at a glance.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// How many tests.
    pub tests: u64,
    /// How many failed an assertion.
    pub failures: u64,
    /// How many could not run to the end.
    pub errors: u64,
    /// How many were not run.
    pub skipped: u64,
    /// How long, in milliseconds.
    pub time_ms: u64,
    /// The full name ([`Case::full_name`]) of each that failed or errored, in order.
    pub failed: Vec<String>,
}

impl Summary {
    /// Adds the counts and the names of `other`.
    pub fn add(&mut self, other: &Self) {
        self.tests += other.tests;
        self.failures += other.failures;
        self.errors += other.errors;
        self.skipped += other.skipped;
        self.time_ms += other.time_ms;
        self.failed.extend(other.failed.iter().cloned());
    }

    /// How many passed.
    pub fn passed(&self) -> u64 {
        self.tests.saturating_sub(self.failures + self.errors + self.skipped)
    }

    /// Whether nothing failed nor errored.
    pub fn ok(&self) -> bool {
        self.failures == 0 && self.errors == 0
    }
}
