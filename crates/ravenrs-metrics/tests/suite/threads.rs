use ravenrs_metrics::Registry;

#[test]
fn a_gauge_used_from_eight_threads_adds_up() {
    let registry = Registry::new();
    let waiting = registry.gauge("builds_waiting", "Builds in the queue.", &[]).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let series = waiting.with(&[]).unwrap();
            scope.spawn(move || {
                for _ in 0..1000 {
                    series.add(2.0);
                    series.sub(1.0);
                }
            });
        }
    });
    assert_eq!(waiting.with(&[]).unwrap().get(), 8000.0);
    assert!(registry.render().contains("builds_waiting 8000\n"));
}
