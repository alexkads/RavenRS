//! Raven CI as a product (#833, #837): the version the installer checks, the self-test, and the
//! manager of plugins on the command line, run on the binary as a person and an agent run it.
#![allow(clippy::expect_used, clippy::print_stderr, reason = "tests fail loudly and say what they skip")]

mod plugins;
mod product;
