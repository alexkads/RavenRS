//! Raven Code: a small editor for Rust projects, built on the `DockArea`. It opens a project from
//! disk, edits and saves its files, and builds and runs it with cargo, with the compiler's errors
//! marked in the code and listed under Problems. It is the start of an IDE for RavenRS.
//!
//! `cargo run -p raven-code -- <folder>` opens a project; with no folder it opens a copy of a small
//! sample project in a temporary folder, so the repository is never edited.
//!
//! * [`project`]: the project on disk.
//! * [`cargo`]: building and running it, and reading what the compiler says.
//! * [`app`]: the window, wiring both to the components.
//!
//! Keys: Ctrl+S (Cmd+S on a Mac) saves the file at the front, Ctrl+B builds, F5 runs, and Ctrl+Tab
//! goes back to the panel used before.

pub mod app;
pub mod cargo;
pub mod project;

pub use app::RavenCode;

use std::io;
use std::path::PathBuf;

use project::Project;

/// The sample project, copied into a temporary folder the first time and opened from there.
pub fn sample() -> io::Result<Project> {
    let folder = std::env::temp_dir().join("raven-code-sample");
    project::write_sample(&folder)?;
    Project::open(folder)
}

/// The project a command line names, or the sample with none.
pub fn from_args(mut args: impl Iterator<Item = String>) -> io::Result<Project> {
    match args.next() {
        Some(folder) => Project::open(PathBuf::from(folder)),
        None => sample(),
    }
}
