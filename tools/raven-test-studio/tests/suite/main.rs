//! Guards that the Studio keeps testing itself: if any of these fails, the Studio's own
//! scenario no longer proves it works.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod apart;
mod atualizacao;
mod delegate;
mod delegate_window;
mod embedded;
mod film;
mod manifest;
mod outputs;
mod project;
mod self_test;
mod tray;
