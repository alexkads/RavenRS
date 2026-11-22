# ravenrs-metrics

Metrics for RavenRS services in the Prometheus text format (0.0.4): what a `/metrics` route serves for Prometheus to read and Grafana to show. It has no dependencies.

## What it does

| Piece | What it does |
|---|---|
| `Registry` | Holds the metrics of a service; clones share them. `render()` writes them all, in the order they were registered. |
| `Registry::counter` | What only goes up, as builds run. `with(&["passed"])` gives a series with `inc` and `add`; a negative amount is ignored. |
| `Registry::gauge` | What goes up and down, as builds waiting or memory held: `set`, `add`, `sub`. |
| `Registry::histogram` | Counts observations into growing ranges, with `+Inf`, `_sum` and `_count`. `DEFAULT_BUCKETS` suits durations in seconds. |
| `CONTENT_TYPE` | `text/plain; version=0.0.4; charset=utf-8`, for the route. |
| `process_rss_bytes` | The memory the process holds, on Linux, from `/proc/self/status`. `None` on other systems, which have no such file. |

Names follow `[a-zA-Z_:][a-zA-Z0-9_:]*` and labels `[a-zA-Z_][a-zA-Z0-9_]*`. A bad name, a name taken, the wrong number of label values or ranges that do not grow are errors, never panics. Label values and help are escaped as the format asks (`\"`, `\\`, `\n`). Everything is safe from any thread.

## Example

```rust
use ravenrs_metrics::{DEFAULT_BUCKETS, Registry, process_rss_bytes};

let registry = Registry::new();
let builds = registry.counter("builds_total", "Builds that ended.", &["result"])?;
let took = registry.histogram("build_seconds", "How long builds take.", &[], DEFAULT_BUCKETS)?;
let memory = registry.gauge("process_resident_memory_bytes", "Memory the server holds.", &[])?;

builds.with(&["passed"])?.inc();
took.with(&[])?.observe(42.0);
if let Some(bytes) = process_rss_bytes() {
    memory.with(&[])?.set(bytes as f64);
}
let body = registry.render();
assert!(body.contains("builds_total{result=\"passed\"} 1"));
# Ok::<(), ravenrs_metrics::MetricError>(())
```
