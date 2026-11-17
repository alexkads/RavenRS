//! The test channel of RavenRS apps.
//!
//! A tool, such as Raven Test Studio, opens the app under test in a child process with no window
//! and drives it through this channel. The app answers with an [`ravenrs_e2e::E2e`] session over
//! its own view, so a step runs exactly as it runs in process:
//!
//! - **The app** turns on the `serve` feature and, when it is opened with `--e2e-serve`, calls
//!   [`serve`] instead of opening its window. It decides what data the session sees: Raven
//!   Finanças opens its sample family in memory, never the database of the person using it.
//! - **The tool** opens it with [`Client::spawn`] and sends [`Command`]s; each answer is a
//!   [`Reply`], and a failed step comes back as a [`Failure`] with the stable code, the hint,
//!   what came close and the screen of the app as it was.
//!
//! The channel is the child's standard input and output, one JSON object per line: no port, no
//! network, and it ends with the child. The app first says [`Hello`], then answers each command
//! in order.
//!
//! The hello may also carry scenarios the app keeps in its own `tests/e2e/` folder, as
//! `.raventest` text ([`Scenario`]), so a tool can run the tests an app brings with it, installed
//! or not, without the app's source. Scenarios kept apart, in a folder of the QA team, use the
//! same format and are run by path.
//!
//! A step is written the way a person writes a block of a scenario: the target is `#id` for a
//! test id or the text shown, as [`target`] reads it. [`Command::apply`] runs a command on a
//! session in this process too, so a tool runs a scenario the same way whether the app is
//! compiled in or opened apart.

mod client;
mod scenario;
#[cfg(feature = "serve")]
mod serve;

use ravenrs_e2e::{E2e, E2eError, Query};
use serde::{Deserialize, Serialize};

pub use client::Client;
pub use scenario::{Block, EXTENSION, FOLDER, Kind, Scenario, ScenarioError, check_folder, scenario_schema};
#[cfg(feature = "serve")]
pub use serve::{Filming, serve, serve_filmed, serve_on, serve_with};

/// The name the app gives in its [`Hello`], so a tool knows it is talking to this channel.
pub const PROTOCOL: &str = "ravenrs-e2e-wire";
/// The version of the protocol, raised when a command or a field changes meaning.
pub const VERSION: u32 = 1;
/// The option that opens an app on this channel instead of its window.
pub const SERVE_FLAG: &str = "--e2e-serve";

/// Codes of this channel, next to the `E2E_*` codes of `ravenrs-e2e`.
pub mod codes {
    /// The app did not start, or ended before saying hello.
    pub const APP_START: &str = "E2E_APP_START";
    /// The app did not answer in time; the tool ended it.
    pub const TIMEOUT: &str = "E2E_TIMEOUT";
    /// A line that is not a command or an answer of this protocol.
    pub const WIRE: &str = "E2E_WIRE";
    /// The app was asked to film, and it is not served with filming, or it could not keep the film.
    pub const NO_FILM: &str = "E2E_NO_FILM";
}

/// What the app says first: that it speaks this channel, which version, and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// Always [`PROTOCOL`].
    pub protocol: String,
    /// The [`VERSION`] the app speaks.
    pub version: u32,
    /// The app's name, as its window shows it.
    pub app: String,
    /// The scenarios the app brings, each the text of a `.raventest` file of its `tests/e2e/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenarios: Vec<String>,
}

impl Hello {
    /// The hello of `app` on this version of the channel.
    pub fn of(app: &str) -> Self {
        Self { protocol: PROTOCOL.to_owned(), version: VERSION, app: app.to_owned(), scenarios: Vec::new() }
    }

    /// With the scenarios the app brings, as the text of its `.raventest` files.
    pub fn scenarios<S: Into<String>>(mut self, scenarios: impl IntoIterator<Item = S>) -> Self {
        self.scenarios = scenarios.into_iter().map(Into::into).collect();
        self
    }
}

/// A step a tool asks of the app. The target is `#id` or the text shown, as [`target`] reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Command {
    /// Clicks the target.
    Click {
        /// `#id` or the text shown.
        target: String,
    },
    /// Types the text into the target, clicking it first when it does not have the focus.
    Type {
        /// `#id` or the text shown.
        target: String,
        /// What to type.
        text: String,
    },
    /// The target's value, or its name when it has no value, is exactly the text.
    Check {
        /// `#id` or the text shown.
        target: String,
        /// What it must be.
        text: String,
    },
    /// The target's name or description contains the text.
    Mention {
        /// `#id` or the text shown.
        target: String,
        /// What it must contain.
        text: String,
    },
    /// Puts the text in the field in place of what it has, as [`ravenrs_e2e::Locator::fill`].
    Fill {
        /// `#id` or the text shown.
        target: String,
        /// What the field must have.
        text: String,
    },
    /// Picks the option in a list or a combo box, as [`ravenrs_e2e::Locator::select`].
    Select {
        /// `#id` or the text shown.
        target: String,
        /// The option, as it is shown.
        text: String,
    },
    /// Opens the item, if it is closed.
    Expand {
        /// `#id` or the text shown.
        target: String,
    },
    /// Closes the item, if it is open.
    Collapse {
        /// `#id` or the text shown.
        target: String,
    },
    /// Scrolls the target into sight, even a row a long list has not built yet.
    Scroll {
        /// `#id` or the text shown.
        target: String,
    },
    /// Waits for the target to show (and to mention the text, when there is one), up to
    /// [`WAIT`]; it fails with `E2E_TIMEOUT` when it does not.
    Wait {
        /// `#id` or the text shown.
        target: String,
        /// What it must mention, if anything.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Drags the target to the part `to` finds, as [`ravenrs_e2e::Locator::drag_to`]: a node of
    /// a diagram to a place, or a port to another port to join them.
    Drag {
        /// `#id` or the text shown: what is dragged.
        target: String,
        /// `#id` or the text shown: where it is dropped.
        to: String,
    },
    /// Starts filming the session, to keep the film at `path` (a `.ravenmovie`) when
    /// [`Command::FilmStop`] comes. Only an app served with [`serve_filmed`] films; one served
    /// with [`serve`] answers `E2E_NO_FILM`.
    Film {
        /// Where the film goes, on the computer the app runs on.
        path: String,
    },
    /// Stops filming and keeps the film; answers with its path.
    #[serde(rename = "film_stop")]
    FilmStop,
    /// The screen as text, as [`E2e::snapshot`] writes it.
    Snapshot,
    /// Ends the session; the app exits.
    Bye,
}

/// What a step needs to find: `#id` is a test id, anything else is the text shown.
pub fn target(text: &str) -> Query {
    match text.trim().strip_prefix('#') {
        Some(id) => Query::test_id(id.trim()),
        None => Query::text(text.trim()),
    }
}

/// How long a step tries again while its target is not ready yet (not there, covered,
/// disabled, out of sight, or a check that does not hold yet) before it fails as it is.
pub const PATIENCE: std::time::Duration = std::time::Duration::from_secs(2);
/// How long a [`Command::Wait`] waits.
pub const WAIT: std::time::Duration = std::time::Duration::from_secs(5);

impl Command {
    /// Runs the command on `e2e`: `Ok` with the screen for [`Command::Snapshot`], `Ok(None)`
    /// for a step that passed, the failure otherwise. [`Command::Bye`] does nothing here.
    ///
    /// A step does not fail for being early: it tries again for up to [`PATIENCE`], with the
    /// session's clock moving, and an action on a part out of sight scrolls to it first.
    pub fn apply(&self, e2e: &mut E2e) -> Result<Option<String>, Failure> {
        let done = match self {
            Self::Snapshot => return Ok(Some(e2e.snapshot())),
            Self::Film { .. } | Self::FilmStop => {
                return Err(Failure::of(codes::NO_FILM, "this app is served without filming"));
            }
            Self::Bye => Ok(()),
            Self::Wait { target: t, text } => {
                let q = target(t);
                match text {
                    Some(text) => e2e.wait_until(WAIT, |e| e.locate(q.clone()).expect().to_mention(text)),
                    None => e2e.wait_until(WAIT, |e| e.find(&q).map(drop)),
                }
            }
            _ => {
                if let Some(t) = self.acts_on() {
                    let q = target(t);
                    if e2e.locate(q.clone()).expect().to_be_visible().is_err() {
                        // Out of sight, or a row a long list has not built: scroll to it once.
                        let _ = e2e.locate(q).scroll_into_view();
                    }
                }
                e2e.patiently(PATIENCE, |e| self.once(e))
            }
        };
        done.map(|()| None).map_err(Failure::from)
    }

    /// The target of a command that acts on a part, which must be in sight.
    fn acts_on(&self) -> Option<&str> {
        match self {
            Self::Click { target }
            | Self::Type { target, .. }
            | Self::Fill { target, .. }
            | Self::Select { target, .. }
            | Self::Expand { target }
            | Self::Collapse { target }
            | Self::Drag { target, .. } => Some(target),
            _ => None,
        }
    }

    /// One try of the step.
    fn once(&self, e2e: &mut E2e) -> ravenrs_e2e::E2eResult {
        match self {
            Self::Click { target: t } => e2e.locate(target(t)).click(),
            Self::Type { target: t, text } => e2e.locate(target(t)).type_text(text),
            Self::Fill { target: t, text } => e2e.locate(target(t)).fill(text),
            Self::Select { target: t, text } => e2e.locate(target(t)).select(text),
            Self::Expand { target: t } => e2e.locate(target(t)).expand(),
            Self::Collapse { target: t } => e2e.locate(target(t)).collapse(),
            Self::Scroll { target: t } => e2e.locate(target(t)).scroll_into_view(),
            Self::Drag { target: t, to } => e2e.locate(target(t)).drag_to(&target(to)),
            Self::Check { target: t, text } => {
                let q = target(t);
                match e2e.find(&q) {
                    Ok(node) if node.value.is_some() => e2e.locate(q).expect().to_have_value(text),
                    Ok(_) => e2e.locate(q).expect().to_have_name(text),
                    Err(e) => Err(e),
                }
            }
            Self::Mention { target: t, text } => e2e.locate(target(t)).expect().to_mention(text),
            Self::Wait { .. } | Self::Snapshot | Self::Bye | Self::Film { .. } | Self::FilmStop => Ok(()),
        }
    }
}

/// Why a step failed, as it crosses the channel: the stable code, what happened, how to fix it,
/// what came close and the screen of the app as it was.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// `E2E_NOT_FOUND`, `E2E_TIMEOUT` and the like; see [`codes`] and `ravenrs_e2e::ErrorCode`.
    pub code: String,
    /// What happened: which query, what was found.
    pub message: String,
    /// How to fix it, when there is an obvious way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// Up to three parts of the screen that came close.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub close: Vec<String>,
    /// The screen of the app when the step failed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub screen: String,
}

impl Failure {
    /// A failure of the channel itself, with no screen.
    pub fn of(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_owned(), message: message.into(), ..Self::default() }
    }
}

impl From<E2eError> for Failure {
    fn from(e: E2eError) -> Self {
        Self {
            code: e.code().as_str().to_owned(),
            message: e.message().to_owned(),
            fix: e.fix().map(str::to_owned),
            close: e.close_matches().to_vec(),
            screen: e.screen_text().to_owned(),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for Failure {}

/// The app's answer to one command.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    /// Whether the step passed.
    pub ok: bool,
    /// The screen, for [`Command::Snapshot`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    /// Why it failed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

impl Reply {
    /// The reply to what [`Command::apply`] returned.
    pub fn of(result: Result<Option<String>, Failure>) -> Self {
        match result {
            Ok(snapshot) => Self { ok: true, snapshot, failure: None },
            Err(failure) => Self { ok: false, snapshot: None, failure: Some(failure) },
        }
    }

    /// Back to what [`Command::apply`] returned.
    pub fn into_result(self) -> Result<Option<String>, Failure> {
        if self.ok {
            Ok(self.snapshot)
        } else {
            Err(self
                .failure
                .unwrap_or_else(|| Failure::of(codes::WIRE, "the app said the step failed, not why")))
        }
    }
}
