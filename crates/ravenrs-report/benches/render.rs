//! Render time of the sample reports.
#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ravenrs_report::{ReportEngine, samples};

fn render(c: &mut Criterion) {
    let report = samples::sales_by_region(10_000);
    let data = report.embedded_data().cloned().unwrap_or_default();
    let mut engine = ReportEngine::new();
    c.bench_function("sales_by_region_10k_rows", |b| b.iter(|| engine.render(&report, &data)));
    let invoice = samples::invoice();
    let data = invoice.embedded_data().cloned().unwrap_or_default();
    c.bench_function("invoice", |b| b.iter(|| engine.render(&invoice, &data)));
}

criterion_group!(benches, render);
criterion_main!(benches);
