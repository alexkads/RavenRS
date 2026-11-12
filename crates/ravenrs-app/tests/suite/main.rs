//! Every integration test of this package in one binary.
//!
//! Each file under `tests/` used to become an executable of its own, linked against the whole
//! dependency graph; one binary links once. Add a new test file here as a module.

mod behavior;
mod center;
mod derive;
mod external;
mod jobs;
mod kept;
mod layers_properties;
mod live;
mod manifest;
mod outbox;
mod record_actions;
mod rules;
mod security;
mod security_paths;
mod workflow;
