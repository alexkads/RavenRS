//! What goes wrong between a plugin and the core, said in words for the person who runs the CI.

use std::fmt;

use crate::Permission;

/// What went wrong, in words for the person who runs the CI.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CiError {
    /// The host said no: the plugin did not declare the permission, or it was taken away.
    Denied {
        /// The plugin that asked.
        plugin: String,
        /// What it would need.
        permission: Permission,
        /// What it wanted to do.
        what: String,
    },
    /// A setting is wrong: which, and why.
    Settings {
        /// The field, by name.
        field: String,
        /// Why it is wrong.
        reason: String,
    },
    /// Something the plugin needs is not there now: a service down, a plugin switched off.
    Unavailable(String),
    /// It failed, saying why.
    Failed(String),
}

impl CiError {
    /// It failed, saying `why`.
    pub fn failed(why: impl Into<String>) -> Self {
        Self::Failed(why.into())
    }

    /// `what` is not there now.
    pub fn unavailable(what: impl Into<String>) -> Self {
        Self::Unavailable(what.into())
    }

    /// The setting `field` is wrong because of `reason`.
    pub fn settings(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Settings { field: field.into(), reason: reason.into() }
    }
}

impl fmt::Display for CiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied { plugin, permission, what } => write!(
                f,
                "o plugin {plugin} não tem a permissão {} ({}), e por isso não pôde {what}",
                permission.capability(),
                permission.words()
            ),
            Self::Settings { field, reason } => write!(f, "a configuração {field}: {reason}"),
            Self::Unavailable(what) | Self::Failed(what) => f.write_str(what),
        }
    }
}

impl std::error::Error for CiError {}
