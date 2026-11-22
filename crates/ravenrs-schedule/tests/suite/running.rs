use ravenrs_schedule::Running;

#[test]
fn the_same_key_cannot_start_twice() {
    let running = Running::new();
    let first = running.try_start("nightly");
    assert!(first.is_some());
    assert!(running.try_start("nightly").is_none(), "still running");
    assert!(running.try_start("hourly").is_some(), "another key is free");
    drop(first);
    assert!(!running.is_running("nightly"));
    assert!(running.try_start("nightly").is_some(), "free again once the first ended");
}

#[test]
fn clones_share_the_set() {
    let running = Running::new();
    let other = running.clone();
    let guard = running.try_start("nightly");
    assert_eq!(guard.as_ref().map(|g| g.key()), Some("nightly"));
    assert!(other.is_running("nightly"));
    assert!(other.try_start("nightly").is_none());
}

#[test]
fn a_run_that_panics_frees_its_key() {
    let running = Running::new();
    let inside = running.clone();
    let result = std::thread::spawn(move || {
        let _guard = inside.try_start("nightly");
        panic!("the run failed");
    })
    .join();
    assert!(result.is_err());
    assert!(!running.is_running("nightly"));
}
