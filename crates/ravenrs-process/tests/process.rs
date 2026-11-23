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

#[test]
fn a_child_for_a_user_that_cannot_be_reached_fails_at_once_saying_why() {
    let started = Instant::now();
    let asked = Spec::new(FAKE)
        .args(["out", "1"])
        .user("raven-ci-ninguem-assim")
        .timeout(Duration::from_secs(20))
        .output();
    match asked {
        // Windows: refused before it starts.
        Err(ProcessError::Unsupported(what)) => assert!(what.contains("raven-ci-ninguem-assim"), "{what}"),
        // Root: there is no such user. Anyone else with no sudo at all: no sudo.
        Err(ProcessError::Start(_) | ProcessError::NotFound(_)) => {}
        // Through sudo: it says why, and ends with a failure, without asking for a password.
        Ok(output) => {
            assert!(!output.end.success(), "{output:?}");
            assert!(!output.stderr.is_empty(), "sudo says why");
        }
        Err(other) => panic!("{other}"),
    }
    assert!(started.elapsed() < Duration::from_secs(15), "it did not wait for a password");
}

#[cfg(unix)]
#[test]
fn as_root_a_child_runs_as_the_user_asked() {
    let me = std::process::Command::new("id").arg("-u").output().expect("id");
    if String::from_utf8_lossy(&me.stdout).trim() != "0" {
        eprintln!("pulado: só root troca o usuário de um filho sem sudo");
        return;
    }
    let output = Spec::new("id").arg("-un").user("nobody").output().expect("runs");
    assert_eq!(output.stdout, ["nobody"]);
}

#[test]
fn confining_a_process_needs_a_delegated_cgroup_on_linux_and_is_refused_elsewhere() {
    // As a build agent does: it confines itself, and so leaves the cgroup it was in, which can then
    // hand the controllers down (a cgroup with processes cannot).
    let limits = ravenrs_process::Limits { memory_bytes: Some(1024 * 1024 * 1024), cores: Some(1.5) };
    let confined = ravenrs_process::confine(std::process::id(), "ravenrs-process-teste", &limits);
    if cfg!(target_os = "linux") {
        match confined {
            Ok(confined) => {
                let max = std::fs::read_to_string(confined.cgroup.join("memory.max")).expect("memory.max");
                assert_eq!(max.trim(), (1024 * 1024 * 1024).to_string());
                let cpu = std::fs::read_to_string(confined.cgroup.join("cpu.max")).expect("cpu.max");
                assert!(cpu.starts_with("150000 100000"), "{cpu}");
                let mine = std::fs::read_to_string("/proc/self/cgroup").expect("cgroup");
                assert!(mine.contains("ravenrs-process-teste"), "{mine}");
                // What it starts from then on is held too.
                let child = Spec::new(FAKE).args(["sleep", "3000"]).spawn().expect("runs");
                let theirs = std::fs::read_to_string(format!("/proc/{}/cgroup", child.control().pid()))
                    .expect("cgroup");
                child.control().stop();
                assert!(theirs.contains("ravenrs-process-teste"), "{theirs}");
            }
            Err(e) => eprintln!("pulado: este Linux não delega o cgroup a quem roda o teste ({e})"),
        }
    } else {
        assert!(matches!(confined, Err(ProcessError::Unsupported(_))), "{confined:?}");
    }
}

#[cfg(unix)]
#[test]
fn stop_and_wait_returns_only_once_a_child_that_ignores_the_polite_stop_is_gone() {
    // `trap '' TERM` makes the shell and the `sleep` it starts ignore the polite stop.
    let running = Spec::new("sh").args(["-c", "trap '' TERM; echo pronto; sleep 30"]).spawn().expect("runs");
    assert_eq!(running.next_within(Duration::from_secs(10)), Some(Event::Out("pronto".into())));
    let group = format!("-{}", running.control().pid());
    let began = Instant::now();
    assert_eq!(running.stop_and_wait(Duration::from_secs(10)), Some(End::Stopped));
    let took = began.elapsed();
    assert!(took >= ravenrs_process::GRACE / 2, "it waited for the forced stop: {took:?}");
    assert!(took < Duration::from_secs(10), "and not for the whole time: {took:?}");
    let alive = std::process::Command::new("kill").args(["-0", "--", &group]).status().expect("kill");
    assert!(!alive.success(), "nothing of the group is left");
}

#[test]
fn stop_and_wait_comes_back_at_once_for_a_child_that_stops_when_asked() {
    let running = Spec::new(FAKE).args(["sleep", "30000"]).spawn().expect("runs");
    let began = Instant::now();
    assert_eq!(running.stop_and_wait(Duration::from_secs(10)), Some(End::Stopped));
    assert!(began.elapsed() < Duration::from_secs(5), "{:?}", began.elapsed());
}

/// What a child left in the background when it ended stays in its group, holding its output
/// open; with `stop_leftovers` it is stopped the moment the child ends, and said (#931). And
/// `stop_group` stops what is left of a group on its own.
#[cfg(unix)]
#[test]
fn what_a_child_left_in_the_background_is_stopped_with_its_group() {
    let alive = |pid: u32| {
        std::process::Command::new("kill")
            .args(["-0", "--", &format!("-{pid}")])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    let started = Instant::now();
    let running = Spec::new("sh").args(["-c", "sleep 30 & echo pronto"]).stop_leftovers().spawn().expect("starts");
    let control = running.control();
    let output = running.wait();
    assert_eq!(output.end, End::Code(0));
    assert_eq!(output.stdout, vec!["pronto".to_owned()]);
    assert!(started.elapsed() < Duration::from_secs(10), "it did not wait for the sleep: {:?}", started.elapsed());
    assert_eq!(control.leftovers(), vec!["sleep".to_owned()]);
    assert!(!alive(control.pid()), "nothing of the group is left");

    // Without it, the group stays, and stop_group stops it apart. The sleep writes nowhere, so
    // the child is seen to end.
    let running = Spec::new("sh").args(["-c", "sleep 30 >/dev/null 2>&1 & echo pronto"]).spawn().expect("starts");
    let control = running.control();
    assert_eq!(running.wait().end, End::Code(0));
    assert!(control.leftovers().is_empty());
    assert!(alive(control.pid()), "the sleep stays after its parent ended");
    assert_eq!(ravenrs_process::stop_group(control.pid()), vec!["sleep".to_owned()]);
    assert!(!alive(control.pid()));
    assert!(ravenrs_process::stop_group(control.pid()).is_empty(), "a group that ended gives nothing");
}
