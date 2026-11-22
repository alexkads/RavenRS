//! Metrics for RavenRS services in the Prometheus text format (0.0.4): what a `/metrics` route
//! serves for Prometheus to read and Grafana to show.
//!
//! * [`Registry`] holds the metrics of a service; [`Registry::render`] writes them all.
//! * [`Counter`] only goes up (builds run), [`Gauge`] goes both ways (builds waiting, memory),
//!   [`Histogram`] counts observations into ranges (how long builds take).
//! * Each takes labels: [`Counter::with`] and the others give the series of those label values.
//! * [`process_rss_bytes`] is the memory the process holds, on Linux, and [`rss_bytes_of`] the
//!   memory of another process, such as a child, on Linux and macOS.
//!
//! Everything is safe to use from any thread. Names and labels are checked when a metric is made,
//! and a mistake is an error, never a panic.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The `Content-Type` of what [`Registry::render`] writes.
pub const CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// The ranges of a histogram nobody gave ranges to, in seconds: from 5 ms to 10 s.
pub const DEFAULT_BUCKETS: &[f64] = &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

/// Why a metric was not made, or a series not given.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetricError {
    /// A metric or label name Prometheus does not take.
    BadName(String),
    /// A metric of this name is already registered.
    Taken(String),
    /// The label values given are not as many as the label names.
    Labels {
        /// How many the metric has.
        expected: usize,
        /// How many were given.
        got: usize,
    },
    /// A histogram's ranges are empty, not growing, or not numbers.
    BadBuckets,
}

impl fmt::Display for MetricError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadName(name) => write!(f, "'{name}' is not a name Prometheus takes"),
            Self::Taken(name) => write!(f, "a metric called '{name}' is already registered"),
            Self::Labels { expected, got } => write!(f, "{got} label values given, for {expected} labels"),
            Self::BadBuckets => f.write_str("the ranges of a histogram have to be numbers that grow"),
        }
    }
}

impl std::error::Error for MetricError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Counter,
    Gauge,
    Histogram,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gauge => "gauge",
            Self::Histogram => "histogram",
        }
    }
}

#[derive(Clone, Debug)]
enum Series {
    Value(f64),
    Histogram { counts: Vec<u64>, sum: f64, count: u64 },
}

#[derive(Debug)]
struct Family {
    name: String,
    help: String,
    kind: Kind,
    labels: Vec<String>,
    buckets: Vec<f64>,
    series: Mutex<BTreeMap<Vec<String>, Series>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic between two whole changes leaves the values as they were.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Family {
    fn key(&self, values: &[&str]) -> Result<Vec<String>, MetricError> {
        if values.len() != self.labels.len() {
            return Err(MetricError::Labels { expected: self.labels.len(), got: values.len() });
        }
        Ok(values.iter().map(|value| (*value).to_owned()).collect())
    }

    fn update(&self, key: &[String], change: impl FnOnce(&mut Series)) {
        let mut series = lock(&self.series);
        let entry = series.entry(key.to_vec()).or_insert_with(|| match self.kind {
            Kind::Histogram => Series::Histogram { counts: vec![0; self.buckets.len()], sum: 0.0, count: 0 },
            Kind::Counter | Kind::Gauge => Series::Value(0.0),
        });
        change(entry);
    }

    fn value(&self, key: &[String]) -> f64 {
        match lock(&self.series).get(key) {
            Some(Series::Value(value)) => *value,
            _ => 0.0,
        }
    }
}

fn valid_name(name: &str, colons: bool) -> bool {
    let mut chars = name.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || (colons && c == ':'));
    first && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || (colons && c == ':'))
}

/// The metrics of a service. Clones share them.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    families: Arc<Mutex<Vec<Arc<Family>>>>,
}

impl Registry {
    /// No metrics yet.
    pub fn new() -> Self {
        Self::default()
    }

    fn register(
        &self,
        name: &str,
        help: &str,
        kind: Kind,
        labels: &[&str],
        buckets: Vec<f64>,
    ) -> Result<Arc<Family>, MetricError> {
        if !valid_name(name, true) {
            return Err(MetricError::BadName(name.to_owned()));
        }
        if let Some(label) = labels.iter().find(|label| !valid_name(label, false) || label.starts_with("__"))
        {
            return Err(MetricError::BadName((*label).to_owned()));
        }
        if kind == Kind::Histogram && labels.contains(&"le") {
            return Err(MetricError::BadName("le".to_owned()));
        }
        let mut families = lock(&self.families);
        if families.iter().any(|family| family.name == name) {
            return Err(MetricError::Taken(name.to_owned()));
        }
        let family = Arc::new(Family {
            name: name.to_owned(),
            help: help.to_owned(),
            kind,
            labels: labels.iter().map(|label| (*label).to_owned()).collect(),
            buckets,
            series: Mutex::new(BTreeMap::new()),
        });
        families.push(family.clone());
        Ok(family)
    }

    /// A counter: what only goes up, as builds run. Its name usually ends in `_total`.
    pub fn counter(&self, name: &str, help: &str, labels: &[&str]) -> Result<Counter, MetricError> {
        self.register(name, help, Kind::Counter, labels, Vec::new()).map(Counter)
    }

    /// A gauge: what goes up and down, as builds waiting or memory held.
    pub fn gauge(&self, name: &str, help: &str, labels: &[&str]) -> Result<Gauge, MetricError> {
        self.register(name, help, Kind::Gauge, labels, Vec::new()).map(Gauge)
    }

    /// A histogram with these ranges, which have to grow; `+Inf` is added by itself. Use
    /// [`DEFAULT_BUCKETS`] for durations in seconds.
    pub fn histogram(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
        buckets: &[f64],
    ) -> Result<Histogram, MetricError> {
        let finite: Vec<f64> = buckets.iter().copied().filter(|bound| bound.is_finite()).collect();
        let grows = finite.windows(2).all(|pair| pair[0] < pair[1]);
        if finite.is_empty() || !grows || buckets.iter().any(|bound| bound.is_nan()) {
            return Err(MetricError::BadBuckets);
        }
        self.register(name, help, Kind::Histogram, labels, finite).map(Histogram)
    }

    /// Every metric, in the Prometheus text format, in the order they were registered.
    pub fn render(&self) -> String {
        let families = lock(&self.families).clone();
        let mut out = String::new();
        for family in families {
            let _ = writeln!(out, "# HELP {} {}", family.name, escape_help(&family.help));
            let _ = writeln!(out, "# TYPE {} {}", family.name, family.kind.name());
            for (values, series) in lock(&family.series).iter() {
                let labels: Vec<(String, String)> =
                    family.labels.iter().cloned().zip(values.iter().cloned()).collect();
                match series {
                    Series::Value(value) => {
                        let _ =
                            writeln!(out, "{}{} {}", family.name, label_set(&labels, None), number(*value));
                    }
                    Series::Histogram { counts, sum, count } => {
                        let mut running = 0;
                        for (bound, hits) in family.buckets.iter().zip(counts) {
                            running += hits;
                            let le = number(*bound);
                            let _ = writeln!(
                                out,
                                "{}_bucket{} {running}",
                                family.name,
                                label_set(&labels, Some(&le))
                            );
                        }
                        let _ = writeln!(
                            out,
                            "{}_bucket{} {count}",
                            family.name,
                            label_set(&labels, Some("+Inf"))
                        );
                        let _ =
                            writeln!(out, "{}_sum{} {}", family.name, label_set(&labels, None), number(*sum));
                        let _ = writeln!(out, "{}_count{} {count}", family.name, label_set(&labels, None));
                    }
                }
            }
        }
        out
    }
}

fn number(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_owned()
    } else if value == f64::INFINITY {
        "+Inf".to_owned()
    } else if value == f64::NEG_INFINITY {
        "-Inf".to_owned()
    } else {
        format!("{value}")
    }
}

fn escape_help(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\n', "\\n")
}

fn escape_value(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn label_set(labels: &[(String, String)], le: Option<&str>) -> String {
    let mut parts: Vec<String> =
        labels.iter().map(|(name, value)| format!("{name}=\"{}\"", escape_value(value))).collect();
    if let Some(le) = le {
        parts.push(format!("le=\"{le}\""));
    }
    if parts.is_empty() { String::new() } else { format!("{{{}}}", parts.join(",")) }
}

/// What only goes up.
#[derive(Clone, Debug)]
pub struct Counter(Arc<Family>);

impl Counter {
    /// The series of these label values, one for each label of the counter.
    pub fn with(&self, values: &[&str]) -> Result<CounterSeries, MetricError> {
        Ok(CounterSeries { family: self.0.clone(), key: self.0.key(values)? })
    }
}

/// One series of a [`Counter`].
#[derive(Clone, Debug)]
pub struct CounterSeries {
    family: Arc<Family>,
    key: Vec<String>,
}

impl CounterSeries {
    /// Adds one.
    pub fn inc(&self) {
        self.add(1.0);
    }

    /// Adds `amount`. A counter only goes up, so a negative amount, or one that is not a number,
    /// is ignored.
    pub fn add(&self, amount: f64) {
        if amount.is_nan() || amount < 0.0 {
            return;
        }
        self.family.update(&self.key, |series| {
            if let Series::Value(value) = series {
                *value += amount;
            }
        });
    }

    /// Its value now.
    pub fn get(&self) -> f64 {
        self.family.value(&self.key)
    }
}

/// What goes up and down.
#[derive(Clone, Debug)]
pub struct Gauge(Arc<Family>);

impl Gauge {
    /// The series of these label values, one for each label of the gauge.
    pub fn with(&self, values: &[&str]) -> Result<GaugeSeries, MetricError> {
        Ok(GaugeSeries { family: self.0.clone(), key: self.0.key(values)? })
    }
}

/// One series of a [`Gauge`].
#[derive(Clone, Debug)]
pub struct GaugeSeries {
    family: Arc<Family>,
    key: Vec<String>,
}

impl GaugeSeries {
    /// Sets it to `value`.
    pub fn set(&self, value: f64) {
        self.family.update(&self.key, |series| {
            if let Series::Value(held) = series {
                *held = value;
            }
        });
    }

    /// Adds `amount`.
    pub fn add(&self, amount: f64) {
        self.family.update(&self.key, |series| {
            if let Series::Value(held) = series {
                *held += amount;
            }
        });
    }

    /// Takes `amount` away.
    pub fn sub(&self, amount: f64) {
        self.add(-amount);
    }

    /// Its value now.
    pub fn get(&self) -> f64 {
        self.family.value(&self.key)
    }
}

/// What counts observations into ranges.
#[derive(Clone, Debug)]
pub struct Histogram(Arc<Family>);

impl Histogram {
    /// The series of these label values, one for each label of the histogram.
    pub fn with(&self, values: &[&str]) -> Result<HistogramSeries, MetricError> {
        Ok(HistogramSeries { family: self.0.clone(), key: self.0.key(values)? })
    }
}

/// One series of a [`Histogram`].
#[derive(Clone, Debug)]
pub struct HistogramSeries {
    family: Arc<Family>,
    key: Vec<String>,
}

impl HistogramSeries {
    /// Counts `value` in the first range that holds it, and in the sum.
    pub fn observe(&self, value: f64) {
        let place = self.family.buckets.iter().position(|bound| value <= *bound);
        self.family.update(&self.key, |series| {
            if let Series::Histogram { counts, sum, count } = series {
                if let Some(place) = place {
                    counts[place] += 1;
                }
                *sum += value;
                *count += 1;
            }
        });
    }
}

/// The memory the process holds now (its resident set), in bytes. On Linux it is read from
/// `/proc/self/status`; elsewhere it is `None`, as there is no file to read it from.
pub fn process_rss_bytes() -> Option<u64> {
    rss_from_status(&std::fs::read_to_string("/proc/self/status").ok()?)
}

/// The memory the process `pid` holds now (its resident set), in bytes: a child the service
/// started, such as a plugin. On Linux it is read from `/proc/<pid>/status`; on macOS `ps` says
/// it; elsewhere, and when the process is gone, it is `None`.
pub fn rss_bytes_of(pid: u32) -> Option<u64> {
    if let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) {
        return rss_from_status(&status);
    }
    if cfg!(target_os = "macos") {
        let out =
            std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().ok()?;
        let kilobytes: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
        return kilobytes.checked_mul(1024);
    }
    None
}

/// The resident set of a `/proc/<pid>/status` file, in bytes.
fn rss_from_status(status: &str) -> Option<u64> {
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kilobytes: u64 =
        line.trim_start_matches("VmRSS:").trim().trim_end_matches("kB").trim().parse().ok()?;
    kilobytes.checked_mul(1024)
}
