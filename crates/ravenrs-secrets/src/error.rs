//! What a vault refuses, and the rights it gives: the base the other modules stand on.

use std::fmt;

use serde::{Deserialize, Serialize};

/// What someone may do with a secret. Each is given on its own: managing a secret does not let
/// anyone see it, and using it does not either.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Access {
    /// Have it handed to code that uses it, with [`Vault::use_with`], without seeing it.
    Use,
    /// See it, with [`Vault::reveal`].
    Reveal,
    /// Rotate it, revoke it and say who may do what.
    Administer,
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Use => "use",
            Self::Reveal => "reveal",
            Self::Administer => "administer",
        })
    }
}

/// Why a vault said no.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecretError {
    /// There is no such secret.
    NotFound,
    /// It was revoked.
    Revoked,
    /// This person may not do this with it.
    Denied(Access),
    /// The record does not open with this key: another key, or a record someone changed.
    Tampered,
    /// The key could not be had, in its source's words.
    Key(String),
    /// Where records are kept failed, in its own words.
    Storage(String),
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("there is no such secret"),
            Self::Revoked => f.write_str("the secret was revoked"),
            Self::Denied(access) => write!(f, "not allowed to {access} this secret"),
            Self::Tampered => f.write_str("the secret does not open with this key, or was changed"),
            Self::Key(why) => write!(f, "the key: {why}"),
            Self::Storage(why) => write!(f, "where secrets are kept: {why}"),
        }
    }
}

impl std::error::Error for SecretError {}
