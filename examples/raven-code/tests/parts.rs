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
        raven_code::cargo::start(root.clone(), task, None, report.clone(), Default::default(), move || {
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

#[test]
fn files_and_folders_are_made_renamed_and_deleted_inside_the_project_only() {
    let root = folder("ops");
    write_sample(&root).unwrap();
    let project = Project::open(&root).unwrap();
    project.create_file("src/util.rs").unwrap();
    assert_eq!(fs::read_to_string(root.join("src/util.rs")).unwrap(), "");
    assert!(project.create_file("src/util.rs").is_err(), "a file that is there is not made again");
    project.create_folder("src/parts").unwrap();
    assert!(project.create_folder("src/parts").is_err());
    project.rename("src/greeting.rs", "src/hello.rs").unwrap();
    assert!(root.join("src/hello.rs").exists() && !root.join("src/greeting.rs").exists());
    assert!(project.rename("src/hello.rs", "src/main.rs").is_err(), "nothing is overwritten");
    assert!(project.rename("src/hello.rs", "../out.rs").is_err(), "nothing leaves the project");
    project.remove("src/parts").unwrap();
    project.remove("src/util.rs").unwrap();
    assert!(!root.join("src/parts").exists() && !root.join("src/util.rs").exists());
    assert!(project.remove("..").is_err());
    use raven_code::project::bad_name;
    assert!(bad_name("ok.rs").is_none());
    for bad in ["", " ", ".", "..", "a/b", "a\\b"] {
        assert!(bad_name(bad).is_some(), "{bad:?}");
    }
}

#[test]
fn a_search_finds_lines_in_every_text_file() {
    let root = folder("search");
    write_sample(&root).unwrap();
    let project = Project::open(&root).unwrap();
    let hits = project.search("hello", false);
    let places: Vec<(String, usize)> = hits.iter().map(|h| (h.file.clone(), h.line)).collect();
    // The function, the greeting it makes, the call, and the package's name.
    assert_eq!(
        places,
        [
            ("src/greeting.rs".to_owned(), 2),
            ("src/greeting.rs".to_owned(), 3),
            ("src/main.rs".to_owned(), 4),
            ("Cargo.toml".to_owned(), 2)
        ],
        "folders first, as the tree lists them"
    );
    let call = hits.iter().find(|h| h.file == "src/main.rs").unwrap();
    assert_eq!(call.column, 30, "counted in characters, from 1");
    let capitals: Vec<usize> = project.search("Hello", true).iter().map(|h| h.line).collect();
    assert_eq!(capitals, [3], "with capitals, only the greeting");
    assert!(project.search("", false).is_empty());
}

#[test]
fn the_stamp_changes_when_the_disk_does() {
    let root = folder("stamp");
    write_sample(&root).unwrap();
    let project = Project::open(&root).unwrap();
    let before = project.stamp();
    assert_eq!(project.stamp(), before, "the same when nothing changed");
    fs::write(root.join("src/new.rs"), "// new\n").unwrap();
    assert_ne!(project.stamp(), before);
}

#[test]
fn recent_projects_keep_ten_the_latest_first_and_sessions_come_back() {
    use raven_code::config::{Config, Session};
    let config = Config::at(folder("config"));
    assert!(config.recent().is_empty());
    for n in 0..12 {
        config.remember(&PathBuf::from(format!("/work/p{n}"))).unwrap();
    }
    config.remember(&PathBuf::from("/work/p5")).unwrap();
    let recent = config.recent();
    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0], PathBuf::from("/work/p5"), "the latest first, once");
    assert_eq!(recent.iter().filter(|p| **p == PathBuf::from("/work/p5")).count(), 1);
    config.forget(&PathBuf::from("/work/p5")).unwrap();
    assert!(!config.recent().contains(&PathBuf::from("/work/p5")));
    let session = Session {
        layout: "dock 4\ncenter src/main.rs".to_owned(),
        open: vec![("src/main.rs".to_owned(), 12), ("src/greeting.rs".to_owned(), 0)],
        active: Some("src/main.rs".to_owned()),
    };
    let project = PathBuf::from("/work/p1");
    config.save_session(&project, &session).unwrap();
    assert_eq!(config.session(&project), Some(session));
    assert_eq!(config.session(&PathBuf::from("/work/other/p1")), None, "projects of one name apart");
    assert_eq!(Session::from_text("something else"), None);
}

#[test]
fn test_results_and_binaries_are_read_from_what_cargo_says() {
    use raven_code::cargo::{binaries_in, test_result};
    assert_eq!(test_result("test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"), Some((3, 0)));
    assert_eq!(test_result("test result: FAILED. 1 passed; 2 failed; 0 ignored"), Some((1, 2)));
    assert_eq!(test_result("running 3 tests"), None);
    let mut report = Report::default();
    report.take("test result: ok. 2 passed; 0 failed; 0 ignored");
    report.take("test result: FAILED. 1 passed; 1 failed; 0 ignored");
    assert_eq!(report.tests, Some((3, 1)), "every test binary adds up");
    let metadata = r#"{"packages":[{"name":"two","targets":[
        {"name":"two","kind":["lib"]},{"name":"server","kind":["bin"]},{"name":"tool","kind":["bin"]},
        {"name":"bench","kind":["bench"]}]}]}"#;
    assert_eq!(binaries_in(metadata), ["server", "tool"]);
    assert!(binaries_in("not json").is_empty());
}

/// With cargo on the machine: a program that waits is stopped, and one that reads gets what
/// is sent. `cargo test -p raven-code -- --ignored`.
#[test]
#[ignore = "runs cargo, which takes a while"]
fn a_running_program_is_stopped_and_reads_what_is_sent() {
    use std::sync::{Arc, Mutex, mpsc};
    use raven_code::cargo::{Control, Task, start};
    let root = folder("stdin");
    write_sample(&root).unwrap();
    // A name of its own: the other test builds `hello`, maybe at the same time and in the same
    // build folder.
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"reader\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n").unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    let mut line = String::new();\n    std::io::stdin().read_line(&mut line).unwrap();\n    println!(\"got {}\", line.trim());\n    loop { std::thread::sleep(std::time::Duration::from_millis(50)); }\n}\n",
    )
    .unwrap();
    let report = Arc::new(Mutex::new(Report::default()));
    let control = Control::default();
    let (done, wait) = mpsc::channel();
    let watch = report.clone();
    start(root.clone(), Task::Run, None, report.clone(), control.clone(), move || {
        let now = watch.lock().unwrap();
        if now.output.contains("Running") || !now.running {
            let _ = done.send(());
        }
    });
    wait.recv_timeout(std::time::Duration::from_secs(300)).expect("it runs");
    // Sends until the program has read it: cargo may still be starting it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !report.lock().unwrap().output.contains("got Raven") {
        control.send("Raven");
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(std::time::Instant::now() < deadline, "{}", report.lock().unwrap().output);
    }
    let asked = std::time::Instant::now();
    assert!(control.stop());
    while report.lock().unwrap().running {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(asked.elapsed() < std::time::Duration::from_secs(1), "stopped in under a second");
    let report = report.lock().unwrap().clone();
    assert!(report.stopped && report.success == Some(false), "{}", report.output);
    assert!(!control.stop(), "nothing runs now");
}
