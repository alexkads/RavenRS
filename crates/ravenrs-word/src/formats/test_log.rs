//! Test helper: the warnings this crate logs, captured per test thread.
//!
//! RavenRS: WordCraft's crates each had their own copy of this logger. Here they are modules of
//! one crate, whose test binary can install only one logger, so this one is doc's.

pub(crate) use crate::doc::test_log::warnings;
