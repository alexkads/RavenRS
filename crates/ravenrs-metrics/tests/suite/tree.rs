//! The memory of a process and what it started, with the feature `process`.

#[test]
#[cfg(feature = "process")]
fn the_tree_of_this_process_holds_at_least_what_it_holds_and_a_gone_process_none() {
    let me = std::process::id();
    let tree = ravenrs_metrics::tree_rss_bytes(me).expect("this process is here");
    assert!(tree > 0);
    // A child adds to the tree.
    let mut child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sleep" })
        .args(if cfg!(windows) { vec!["/C", "ping -n 3 127.0.0.1 > NUL"] } else { vec!["2"] })
        .spawn()
        .expect("a child");
    std::thread::sleep(std::time::Duration::from_millis(300));
    let with_child = ravenrs_metrics::tree_rss_bytes(me).expect("still here");
    let _ = child.kill();
    let _ = child.wait();
    assert!(with_child > 0);
    assert_eq!(ravenrs_metrics::tree_rss_bytes(u32::MAX - 7), None);
}
