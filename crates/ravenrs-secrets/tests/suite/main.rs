//! The tests of ravenrs-secrets, in one binary.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod keys;
mod redaction;
mod vault;

use ravenrs_secrets::{FixedKey, MemoryStorage, Vault};

/// A vault in memory with a key of its own.
pub fn vault(seed: u8) -> Vault<MemoryStorage> {
    Vault::open(&FixedKey([seed; 32]), MemoryStorage::new()).unwrap()
}

/// A value that looks like a real token, to look for in what must not hold it.
pub const TOKEN: &str = "ghp_umTokenDeTesteMuitoSecreto123";
