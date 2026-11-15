//! The executor through the fake program: lines by stream, a grandchild stopped with its parent
//! at the time limit, a variable left out, input lines, long lines cut, codes and the version.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::time::{Duration, Instant};

use ravenrs_process::{End, Event, ProcessError, Spec, resolve, version};

const FAKE: &str = env!("CARGO_BIN_EXE_ravenrs-process-fake");

#[test]
fn lines_come_in_order_on_each_stream_and_a_full_stream_does_not_hang() {
    // Far more than a pipe holds, on both streams at once.
    let output =
        Spec::new(FAKE).args(["both", "20000"]).timeout(Duration::from_secs(30)).output().expect("runs");
    assert_eq!(output.end, End::Code(0));
    assert_eq!(output.stdout.len(), 20000);
    assert_eq!(output.stderr.len(), 20000);
    assert!(output.stdout.iter().enumerate().all(|(i, l)| *l == format!("out {i}")), "in order on stdout");
    assert!(output.stderr.iter().enumerate().all(|(i, l)| *l == format!("err {i}")), "and on stderr");
}

#[test]
fn a_child_that_starts_a_grandchild_and_hangs_ends_whole_at_the_time_limit() {
    let started = Instant::now();
    let running = Spec::new(FAKE)
        .args(["grandchild", "60000"])
        .timeout(Duration::from_millis(500))
        .spawn()
        .expect("runs");
    let mut grandchild = None;
    let mut end = None;
    for event in running.events() {
        match event {
            Event::Out(line) => {
                grandchild = line.strip_prefix("grandchild ").and_then(|p| p.parse::<u32>().ok())
            }
            Event::Ended(e) => end = Some(e),
            Event::Err(_) => {}
        }
    }
    assert_eq!(end, Some(End::TimedOut));
    assert!(started.elapsed() < Duration::from_secs(10), "it did not wait for the grandchild's minute");
    let pid = grandchild.expect("the grandchild said who it is");
    if cfg!(unix) {
        let alive =
            std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().expect("kill -0");
        assert!(!alive.success(), "the grandchild was stopped too");
    }
}

#[test]
fn a_variable_not_allowed_does_not_reach_the_child() {
    // SAFETY-free: the variable is set for this test process only through the child spec.
    let secret =
        Spec::new(FAKE).args(["env", "RAVENRS_TEST_SECRET"]).env("RAVENRS_TEST_SECRET", "x").output();
    assert_eq!(secret.expect("runs").stdout, ["x"], "set for it, it sees it");
    let path = Spec::new(FAKE).args(["env", "PATH"]).output().expect("runs");
    assert_ne!(path.stdout, ["<none>"], "PATH is among the basic ones");
    let home_like = Spec::new(FAKE).args(["env", "CARGO_PKG_NAME"]).output().expect("runs");
    assert_eq!(home_like.stdout, ["<none>"], "a variable of this process not allowed stays here");
    let allowed =
        Spec::new(FAKE).args(["env", "CARGO_PKG_NAME"]).allow_env("CARGO_PKG_NAME").output().expect("runs");
    assert_eq!(allowed.stdout, ["ravenrs-process"], "unless allowed");
}

#[test]
fn input_lines_reach_the_child_and_stop_ends_it() {
    let running = Spec::new(FAKE).arg("echo").stdin().spawn().expect("runs");
    let control = running.control();
    assert!(control.send("olá"));
    assert_eq!(running.next_within(Duration::from_secs(10)), Some(Event::Out("echo olá".into())));
    control.stop();
    let output = running.wait();
    assert_eq!(output.end, End::Stopped);
    assert!(!control.send("depois"), "its input is closed");
}

#[test]
fn a_long_line_is_cut_and_codes_and_versions_come_back() {
    let output = Spec::new(FAKE).args(["long", "2500"]).line_limit(1000).output().expect("runs");
    assert_eq!(output.stdout.iter().map(String::len).collect::<Vec<_>>(), [1000, 1000, 500]);
    assert_eq!(Spec::new(FAKE).args(["fail", "3"]).output().expect("runs").end, End::Code(3));
    assert!(!End::Code(3).success());
    assert_eq!(version(FAKE, &["out", "1"], Duration::from_secs(10)).as_deref(), Ok("out 0"));
    assert_eq!(version(FAKE, &["sleep", "5000"], Duration::from_millis(200)), Err(ProcessError::TimedOut));
    assert!(matches!(Spec::new("ravenrs-no-such-program").spawn(), Err(ProcessError::NotFound(_))));
    assert!(resolve(FAKE).is_some());
}
