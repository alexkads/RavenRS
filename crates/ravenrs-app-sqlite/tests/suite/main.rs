//! Every integration test of this package in one binary.
//!
//! Each file under `tests/` used to become an executable of its own, linked against the whole
//! dependency graph; one binary links once. Add a new test file here as a module.

mod backup;
mod contract;
mod decimal;
mod schema;
mod space;
