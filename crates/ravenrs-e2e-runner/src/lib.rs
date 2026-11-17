//! Runs end-to-end scenarios of RavenRS apps and reports them: the engine shared by Raven Test
//! Studio (RTS) and `cargo ravenrs e2e`.
//!
//! - [`project`] reads a Rust project without building it: its RavenRS packages, which are apps,
//!   whether each speaks the test channel (`ravenrs-e2e-wire`), its `.raventest` scenarios and
//!   the screens it builds.
//! - [`app`] opens an app apart, in a child process with no window: its code compiled by [`build`], a
//!   copy the installer put on the computer, or a binary a variable names.
//! - [`session`] is what a run drives and how each step goes: the step a block stands for, and
//!   why it failed, by its stable code, with the meaning and the fix in Portuguese.
//! - [`run`] runs `.raventest` files straight from their blocks, and [`report`] says how it went
//!   as text, JSON, JSON lines or JUnit, and keeps it in a folder (`relatorio.json`,
//!   `trace.jsonl`, `arvore.txt`).
//!
//! ```no_run
//! use ravenrs_e2e_runner::{app, project, run};
//!
//! let project = project::read(std::path::Path::new(".")).unwrap();
//! project.learn_apps();
//! for folder in project.scenario_folders() {
//!     let report = run::run_files(&folder, app::open).unwrap();
//!     println!("{}", report.text());
//! }
//! ```

// The JSON Schema of the report is one `json!` literal, deeper than the default allows.
#![recursion_limit = "256"]

pub mod app;
pub mod build;
pub mod config;
pub mod driver;
pub mod html;
pub mod project;
pub mod report;
pub mod run;
pub mod rust;
pub mod session;
