//! The parts with no screen: the project on disk, and what the compiler says.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use std::fs;
use std::path::PathBuf;

use raven_code::cargo::{Diagnostic, Level, Line, Report, read_line};
use raven_code::project::{Project, write_sample};

/// A fresh folder of its own for a test.
fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("raven-code-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).unwrap();
    folder
}

const ERROR: &str = r#"{"reason":"compiler-message","message":{"rendered":"error: cannot find macro `printn` in this scope\n --> src/main.rs:4:5\n","level":"error","message":"cannot find macro `printn` in this scope","spans":[{"file_name":"src/main.rs","line_start":4,"line_end":4,"column_start":5,"column_end":11,"is_primary":true}]}}"#;
const WARNING: &str = r#"{"reason":"compiler-message","message":{"rendered":"warning: unused variable: `x`\n","level":"warning","message":"unused variable: `x`","spans":[{"file_name":"src/greeting.rs","line_start":2,"line_end":2,"column_start":9,"column_end":10,"is_primary":true}]}}"#;
const ABORT: &str = r#"{"reason":"compiler-message","message":{"rendered":"error: aborting due to 1 previous error\n","level":"error","message":"aborting due to 1 previous error","spans":[]}}"#;

#[test]
fn the_tree_leaves_out_target_and_hidden_folders_and_puts_folders_first() {
    let root = folder("tree");
    write_sample(&root).unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    let project = Project::open(&root).unwrap();
    let tree = project.tree().unwrap();
    let names: Vec<&str> = tree.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["src", "Cargo.toml"]);
    let src: Vec<&str> = tree[0].children.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(src, ["src/greeting.rs", "src/main.rs"]);
}

#[test]
fn files_read_and_write_and_nothing_outside_the_project() {
    let root = folder("files");
    write_sample(&root).unwrap();
    let project = Project::open(&root).unwrap();
    assert!(project.read("src/main.rs").unwrap().contains("greeting::hello"));
    project.write("src/main.rs", "fn main() {}\n").unwrap();
    assert_eq!(fs::read_to_string(root.join("src/main.rs")).unwrap(), "fn main() {}\n");
    assert!(project.read("../secret").is_err());
    assert!(project.write("src/../../escape.rs", "").is_err());
    write_sample(&root).unwrap();
    assert_eq!(project.read("src/main.rs").unwrap(), "fn main() {}\n", "the sample does not overwrite");
}

#[test]
fn a_compiler_error_becomes_a_diagnostic_at_its_place() {
    let Line::Compiler { rendered, diagnostic } = read_line(ERROR) else { panic!("a compiler message") };
    assert!(rendered.contains("--> src/main.rs:4:5"));
    assert_eq!(
        diagnostic,
        Some(Diagnostic {
            file: "src/main.rs".into(),
            line: 4,
            column: 5,
            length: 6,
            level: Level::Error,
            message: "cannot find macro `printn` in this scope".into(),
        })
    );
    let Line::Compiler { diagnostic, .. } = read_line(ABORT) else { panic!("a compiler message") };
    assert_eq!(diagnostic, None, "no place, no diagnostic");
    assert_eq!(read_line(r#"{"reason":"build-finished","success":false}"#), Line::Finished(false));
    assert_eq!(read_line(r#"{"reason":"compiler-artifact"}"#), Line::Quiet);
    assert_eq!(read_line("Hello from Raven Code!"), Line::Text("Hello from Raven Code!".into()));
}

#[test]
fn a_report_gathers_the_output_and_counts_what_was_found() {
    let mut report = Report::default();
    for line in [ERROR, WARNING, ABORT, r#"{"reason":"build-finished","success":false}"#] {
        report.take(line);
    }
    assert_eq!(report.diagnostics.len(), 2);
    assert_eq!(report.summary(), "1 error, 1 warning");
    assert_eq!(report.success, Some(false));
    assert!(report.output.contains("aborting due to 1 previous error"));
}

/// With cargo on the machine: the sample builds and runs. `cargo test -p raven-code -- --ignored`.
#[test]
#[ignore = "runs cargo, which takes a while"]
fn the_sample_builds_and_runs_with_cargo() {
    use std::sync::{Arc, Mutex, mpsc};
    let root = folder("cargo");
    write_sample(&root).unwrap();
    for task in [raven_code::cargo::Task::Build, raven_code::cargo::Task::Run] {
        let report = Arc::new(Mutex::new(Report::default()));
        let (done, wait) = mpsc::channel();
        let watch = report.clone();
        raven_code::cargo::start(root.clone(), task, report.clone(), move || {
            if !watch.lock().unwrap().running {
                let _ = done.send(());
            }
        });
        wait.recv_timeout(std::time::Duration::from_secs(300)).expect("cargo ends");
        let report = report.lock().unwrap().clone();
        assert_eq!(report.success, Some(true), "{}", report.output);
        if task == raven_code::cargo::Task::Run {
            assert!(report.output.contains("Hello from Raven Code!"), "{}", report.output);
        }
    }
}
