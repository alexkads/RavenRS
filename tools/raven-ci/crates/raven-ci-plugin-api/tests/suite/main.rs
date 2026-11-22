//! The API of the plugins of Raven CI (#834): each extension point used through its fake plugin,
//! and the host saying yes or no by permission.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

mod host;
mod points;
