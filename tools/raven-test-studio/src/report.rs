//! What a run tells whoever asked for it, from `ravenrs-e2e-runner`, plus the report of a run
//! of the Studio's diagram.

use ravenrs::prelude::FlowGraph;
pub use ravenrs_e2e_runner::report::*;

use crate::runner::{self, Run};

/// Runs `graph` to the end with `run` and reports it under `scenario`.
pub fn of(scenario: &str, mut graph: FlowGraph, mut run: Run) -> Report {
    let total = runner::path(&graph).len();
    let apart = crate::sample::opens_apart(&graph);
    let started = std::time::Instant::now();
    let outcomes = run.to_end(&mut graph);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let not_run = runner::path(&graph)
        .iter()
        .skip(outcomes.len())
        .filter_map(|id| graph.nodes.iter().find(|n| n.id == *id))
        .map(|n| n.title.clone())
        .collect();
    let (films, film_note) = run.films();
    Report {
        scenario: scenario.to_owned(),
        outcomes,
        total,
        cached_at: None,
        duration_ms,
        apart,
        not_run,
        films,
        film_note,
    }
}
