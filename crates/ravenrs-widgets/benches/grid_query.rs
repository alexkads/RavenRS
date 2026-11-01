//! DataGrid query engine on 100,000 rows: the work done when the user sorts,
//! filters, searches or groups. Budget: each stays under one 60 Hz frame (16 ms).
#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use ravenrs_widgets::grid::query::run_query;
use ravenrs_widgets::grid::{GridColumn, GridQuery, Row, SortDirection, Summary, Value};

const ROWS: usize = 100_000;
const COUNTRIES: [&str; 8] = ["Brazil", "Chile", "Peru", "Mexico", "Canada", "Spain", "Japan", "Kenya"];

fn columns() -> Vec<GridColumn> {
    vec![
        GridColumn::text("Customer"),
        GridColumn::text("Country"),
        GridColumn::number("Seats").summary(Summary::Sum),
        GridColumn::number("Revenue").summary(Summary::Average),
    ]
}

fn rows() -> Vec<Row> {
    // A small linear congruential generator keeps the data deterministic.
    let mut seed = 0x2545_f491_u64;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (seed >> 33) as u32
    };
    (0..ROWS)
        .map(|i| {
            vec![
                Value::from(format!("Customer {i:06}")),
                Value::from(COUNTRIES[next() as usize % COUNTRIES.len()]),
                Value::from(next() % 500),
                Value::from(f64::from(next() % 100_000) / 100.0),
            ]
        })
        .collect()
}

fn query(f: impl FnOnce(&mut GridQuery)) -> GridQuery {
    let mut q = GridQuery::default();
    f(&mut q);
    q
}

fn bench(c: &mut Criterion) {
    let columns = columns();
    let rows = rows();
    let cases = [
        ("unsorted", GridQuery::default()),
        ("sort_number", query(|q| q.sort = vec![(2, SortDirection::Descending)])),
        (
            "sort_text_then_number",
            query(|q| q.sort = vec![(1, SortDirection::Ascending), (3, SortDirection::Descending)]),
        ),
        ("filter_range", query(|q| q.set_filter(2, "100..200".into()))),
        (
            "filter_expression",
            query(|q| q.set_filter(2, "[Seats] >= 100 And [Seats] <= 200 And [Country] <> 'Chile'".into())),
        ),
        ("search", query(|q| q.search = "chile".into())),
        ("group_with_summaries", query(|q| q.group_by = vec![1])),
    ];
    let mut group = c.benchmark_group("grid_query_100k");
    group.sample_size(20);
    for (name, q) in &cases {
        group.bench_function(*name, |b| {
            b.iter(|| run_query(black_box(&columns), black_box(&rows), black_box(q)))
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
