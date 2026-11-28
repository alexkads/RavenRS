use ravenrs_metrics::{CONTENT_TYPE, MetricError, Registry, process_rss_bytes, rss_bytes_of};

#[test]
fn a_counter_with_a_label_writes_its_type_and_value() {
    let registry = Registry::new();
    let builds = registry.counter("builds_total", "Builds that ended.", &["result"]).unwrap();
    let passed = builds.with(&["passed"]).unwrap();
    for _ in 0..3 {
        passed.inc();
    }
    passed.add(-5.0);
    let text = registry.render();
    assert!(text.contains("# HELP builds_total Builds that ended.\n"), "{text}");
    assert!(text.contains("# TYPE builds_total counter\n"), "{text}");
    assert!(text.contains("builds_total{result=\"passed\"} 3\n"), "a counter does not go down: {text}");
}

#[test]
fn a_histogram_counts_into_growing_ranges() {
    let registry = Registry::new();
    let took = registry.histogram("build_seconds", "How long builds take.", &[], &[1.0, 5.0, 10.0]).unwrap();
    let series = took.with(&[]).unwrap();
    for value in [0.5, 3.0, 20.0] {
        series.observe(value);
    }
    let text = registry.render();
    for line in [
        "# TYPE build_seconds histogram",
        "build_seconds_bucket{le=\"1\"} 1",
        "build_seconds_bucket{le=\"5\"} 2",
        "build_seconds_bucket{le=\"10\"} 2",
        "build_seconds_bucket{le=\"+Inf\"} 3",
        "build_seconds_sum 23.5",
        "build_seconds_count 3",
    ] {
        assert!(text.lines().any(|found| found == line), "{line} in:\n{text}");
    }
}

#[test]
fn label_values_and_help_are_escaped() {
    let registry = Registry::new();
    let gauge =
        registry.gauge("plugin_memory_bytes", "Memory of each plugin.\nIn bytes.", &["plugin"]).unwrap();
    gauge.with(&["a \"b\" c\\d\ne"]).unwrap().set(42.0);
    let text = registry.render();
    assert!(text.contains("# HELP plugin_memory_bytes Memory of each plugin.\\nIn bytes.\n"), "{text}");
    assert!(text.contains("plugin_memory_bytes{plugin=\"a \\\"b\\\" c\\\\d\\ne\"} 42\n"), "{text}");
}

#[test]
fn mistakes_are_errors_not_panics() {
    let registry = Registry::new();
    assert_eq!(
        registry.counter("builds-total", "", &[]).unwrap_err(),
        MetricError::BadName("builds-total".into())
    );
    assert_eq!(
        registry.gauge("queue", "", &["bad-label"]).unwrap_err(),
        MetricError::BadName("bad-label".into())
    );
    registry.gauge("queue", "", &[]).unwrap();
    assert_eq!(registry.gauge("queue", "", &[]).unwrap_err(), MetricError::Taken("queue".into()));
    let builds = registry.counter("builds_total", "", &["result", "job"]).unwrap();
    assert_eq!(builds.with(&["passed"]).unwrap_err(), MetricError::Labels { expected: 2, got: 1 });
    assert_eq!(registry.histogram("h", "", &[], &[5.0, 1.0]).unwrap_err(), MetricError::BadBuckets);
    assert_eq!(registry.histogram("h2", "", &[], &[]).unwrap_err(), MetricError::BadBuckets);
    assert_eq!(registry.histogram("h3", "", &["le"], &[1.0]).unwrap_err(), MetricError::BadName("le".into()));
}

#[test]
fn the_order_is_the_order_of_registration() {
    let registry = Registry::new();
    registry.gauge("zeta", "Last letter.", &[]).unwrap().with(&[]).unwrap().set(1.0);
    registry.gauge("alpha", "First letter.", &[]).unwrap().with(&[]).unwrap().set(2.0);
    let text = registry.render();
    assert!(text.find("zeta").unwrap() < text.find("alpha").unwrap(), "{text}");
    assert!(CONTENT_TYPE.starts_with("text/plain; version=0.0.4"));
}

#[test]
fn the_memory_of_the_process_is_read_where_there_is_a_place_to_read_it() {
    let rss = process_rss_bytes();
    if cfg!(target_os = "linux") {
        assert!(rss.is_some_and(|bytes| bytes > 0), "{rss:?}");
    } else {
        assert_eq!(rss, None);
    }
}

#[test]
fn the_memory_of_a_process_by_its_id_is_read_where_the_system_says_it() {
    let own = rss_bytes_of(std::process::id());
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(
            own.is_some_and(|bytes| bytes > 1024 * 1024),
            "this test process holds more than 1 MB: {own:?}"
        );
    } else {
        assert_eq!(own, None);
    }
    assert_eq!(rss_bytes_of(u32::MAX), None, "no process has this id");
}
