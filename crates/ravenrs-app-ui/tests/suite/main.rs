//! Every integration test of this package in one binary.
//!
//! Each file under `tests/` used to become an executable of its own, linked against the whole
//! dependency graph; one binary links once. Add a new test file here as a module.

mod accessibility;
mod center;
mod counter;
mod dates;
mod e2e;
mod empty;
mod jobs;
mod kept;
mod list_editors;
mod narrow;
mod navigation;
mod relations;
mod security;
mod texts;
mod themes;
mod workflow;
