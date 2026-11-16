//! What can go wrong in a test, with a code that tools can rely on.

use std::fmt;

/// Why a step of a test failed. The codes are stable: tools, agents and reports match on them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// Nothing on the screen matches the query.
    NotFound,
    /// More than one thing matches and the query does not say which.
    Ambiguous,
    /// The target does not respond to input.
    Disabled,
    /// Something else is on top of the target, so input would land there.
    Covered,
    /// The target is outside the window or has no size.
    NotVisible,
    /// The target's value is secret and is never given out.
    ProtectedValue,
    /// A check did not hold.
    Expectation,
    /// What a step waited for did not happen in time.
    Timeout,
}

impl ErrorCode {
    /// The code as reports write it, such as `E2E_NOT_FOUND`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "E2E_NOT_FOUND",
            Self::Ambiguous => "E2E_AMBIGUOUS",
            Self::Disabled => "E2E_DISABLED",
            Self::Covered => "E2E_COVERED",
            Self::NotVisible => "E2E_NOT_VISIBLE",
            Self::ProtectedValue => "E2E_PROTECTED_VALUE",
            Self::Expectation => "E2E_EXPECTATION",
            Self::Timeout => "E2E_TIMEOUT",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A failed step: its code, what happened, how to fix it, what came close and the screen as it
/// was, so the failure explains itself without running the test again.
#[derive(Clone, Debug, PartialEq)]
pub struct E2eError {
    code: ErrorCode,
    message: String,
    hint: Option<String>,
    similar: Vec<String>,
    screen: String,
}

impl E2eError {
    /// Whether waiting could still make it pass: the part may not be there yet, may still be
    /// covered or disabled, or the check may not hold yet. An ambiguous query or a secret value
    /// stays so however long one waits.
    pub fn may_pass_later(&self) -> bool {
        matches!(
            self.code,
            ErrorCode::NotFound
                | ErrorCode::Covered
                | ErrorCode::Disabled
                | ErrorCode::NotVisible
                | ErrorCode::Expectation
        )
    }

    /// The same failure, as a wait that ran out after `waited`.
    pub(crate) fn timed_out(mut self, waited: std::time::Duration) -> Self {
        self.message = format!("waited {:.1} s and still: {}", waited.as_secs_f64(), self.message);
        self.code = ErrorCode::Timeout;
        if self.hint.is_none() {
            self.hint = Some("wait longer, or check what the screen is waiting for".to_owned());
        }
        self
    }

    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), hint: None, similar: Vec::new(), screen: String::new() }
    }

    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub(crate) fn similar(mut self, similar: Vec<String>) -> Self {
        self.similar = similar;
        self
    }

    pub(crate) fn screen(mut self, screen: String) -> Self {
        self.screen = screen;
        self
    }

    /// The stable code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// What happened.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// How to fix it, when there is an obvious way.
    pub fn fix(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// Up to three nodes that came close to what the query asked for, one line each.
    pub fn close_matches(&self) -> &[String] {
        &self.similar
    }

    /// The screen when it failed, as [`E2e::snapshot`](crate::E2e::snapshot) writes it.
    pub fn screen_text(&self) -> &str {
        &self.screen
    }
}

impl fmt::Display for E2eError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  fix: {hint}")?;
        }
        if !self.similar.is_empty() {
            f.write_str("\n  close matches:")?;
            for line in &self.similar {
                write!(f, "\n    {line}")?;
            }
        }
        if !self.screen.is_empty() {
            f.write_str("\n  screen:")?;
            for line in self.screen.lines() {
                write!(f, "\n    {line}")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for E2eError {}

/// The result of a test step.
pub type E2eResult<T = ()> = Result<T, E2eError>;
