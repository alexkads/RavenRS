//! Raven CI as a product (#833, #837, #873, #882, #898): the version the installer checks, the self-test, the
//! manager of plugins on the command line and the server, run on the binary as a person, an agent and
//! a service manager run it.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests fail loudly and say what they skip"
)]

mod compilado;
mod config;
mod plugins;
mod product;
mod remote;
mod server;
