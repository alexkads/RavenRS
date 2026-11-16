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
//! A step is written the way a person writes a block of a scenario: the target is `#id` for a
//! test id or the text shown, as [`target`] reads it. [`Command::apply`] runs a command on a
//! session in this process too, so a tool runs a scenario the same way whether the app is
//! compiled in or opened apart.

mod client;
#[cfg(feature = "serve")]
mod serve;

use ravenrs_e2e::{E2e, E2eError, Query};
use serde::{Deserialize, Serialize};

pub use client::Client;
#[cfg(feature = "serve")]
pub use serve::{serve, serve_on};

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
}

impl Hello {
    /// The hello of `app` on this version of the channel.
    pub fn of(app: &str) -> Self {
        Self { protocol: PROTOCOL.to_owned(), version: VERSION, app: app.to_owned() }
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

impl Command {
    /// Runs the command on `e2e`: `Ok` with the screen for [`Command::Snapshot`], `Ok(None)`
    /// for a step that passed, the failure otherwise. [`Command::Bye`] does nothing here.
    pub fn apply(&self, e2e: &mut E2e) -> Result<Option<String>, Failure> {
        let done = match self {
            Self::Click { target: t } => e2e.locate(target(t)).click(),
            Self::Type { target: t, text } => e2e.locate(target(t)).type_text(text),
            Self::Check { target: t, text } => {
                let q = target(t);
                match e2e.find(&q) {
                    Ok(node) if node.value.is_some() => e2e.locate(q).expect().to_have_value(text),
                    Ok(_) => e2e.locate(q).expect().to_have_name(text),
                    Err(e) => Err(e),
                }
            }
            Self::Mention { target: t, text } => e2e.locate(target(t)).expect().to_mention(text),
            Self::Snapshot => return Ok(Some(e2e.snapshot())),
            Self::Bye => Ok(()),
        };
        done.map(|()| None).map_err(Failure::from)
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
