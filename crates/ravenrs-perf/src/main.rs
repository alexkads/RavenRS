//! `ravenrs-perf gate`: the exact counters against the baseline, for `scripts/check.sh`.
//! `ravenrs-perf report`: the counters, to look at.
//! `ravenrs-perf time`: how long frames take, for `scripts/perf-time.sh`.

use std::alloc::System;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use ravenrs_perf::{
    Bench, COUNTED_ROUNDS, Counters, FrameTime, Outcome, PerfScene, REPORTED_ONLY, capacity, scenes,
};
use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const BASELINE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../perf/baseline.json");

type Table = BTreeMap<String, BTreeMap<String, u64>>;

fn allocations() -> u64 {
    let stats = GLOBAL.stats();
    (stats.allocations + stats.reallocations) as u64
}

fn matching(filter: Option<&str>) -> Vec<PerfScene> {
    scenes::all().into_iter().filter(|s| filter.is_none_or(|f| s.name.contains(f))).collect()
}

fn run(bench: &Bench, filter: Option<&str>) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    for scene in matching(filter) {
        for interaction in scene.interactions {
            outcomes.push(bench.measure(&scene, interaction, None));
        }
    }
    outcomes
}

/// What a run is held to. Allocations of a whole application depend on its data, on its
/// release notes and on the clock, so there they are only reported; everything else is exact.
fn compared(name: &str, counters: &Counters) -> BTreeMap<String, u64> {
    counters
        .iter()
        .filter(|(counter, _)| !REPORTED_ONLY.contains(counter))
        .filter(|(counter, _)| !(name.starts_with("app-") && **counter == "ui_allocations"))
        .map(|(counter, value)| ((*counter).to_owned(), *value))
        .collect()
}

/// What differs between two runs of the same code: anything here makes the gate useless.
fn not_repeatable(first: &[Outcome], second: &[Outcome]) -> Vec<String> {
    let mut problems = Vec::new();
    for (a, b) in first.iter().zip(second) {
        for (counter, value) in compared(&a.name, &a.counters) {
            let other = compared(&b.name, &b.counters).get(&counter).copied().unwrap_or(0);
            if other != value {
                problems.push(format!("{}: {counter} is not repeatable: {value} then {other}", a.name));
            }
        }
    }
    problems
}

fn over_ceilings(outcomes: &[Outcome]) -> Vec<String> {
    let scenes = scenes::all();
    let mut problems = Vec::new();
    for outcome in outcomes {
        let Some((scene_name, interaction)) = outcome.name.split_once('/') else { continue };
        let Some(scene) = scenes.iter().find(|s| s.name == scene_name) else { continue };
        for ceiling in scene.ceilings {
            if ceiling.interaction != "*" && ceiling.interaction != interaction {
                continue;
            }
            let limit = ceiling.per_round * u64::from(COUNTED_ROUNDS);
            let value = outcome.counters.get(ceiling.counter).copied().unwrap_or(0);
            if value > limit {
                problems.push(format!(
                    "{}: {} is {value}, over its ceiling of {limit}. Find the cause in the \
                     diagnostics panel; raise the ceiling in the scene only if the cost is worth it.",
                    outcome.name, ceiling.counter
                ));
            }
        }
    }
    problems
}

fn differences(current: &Table, saved: &Table) -> Vec<String> {
    let mut found = Vec::new();
    for (name, counters) in current {
        let Some(old) = saved.get(name) else {
            found.push(format!("{name}: new, not in the baseline"));
            continue;
        };
        for (counter, value) in counters {
            let was = old.get(counter).copied().unwrap_or(0);
            if was != *value {
                let elements = counters.get("elements").copied().unwrap_or(0);
                found.push(format!("{name}: {counter} was {was}, is {value} ({elements} elements)"));
            }
        }
    }
    found.extend(
        saved
            .keys()
            .filter(|name| !current.contains_key(*name))
            .map(|name| format!("{name}: in the baseline, no longer measured")),
    );
    found
}

fn gate(bench: &Bench, bless: bool) -> Result<(), String> {
    let first = run(bench, None);
    let second = run(bench, None);
    let mut problems = not_repeatable(&first, &second);
    problems.extend(over_ceilings(&first));
    let current: Table = first.iter().map(|o| (o.name.clone(), compared(&o.name, &o.counters))).collect();
    if bless {
        if !problems.is_empty() {
            return Err(problems.join("\n"));
        }
        let text = serde_json::to_string_pretty(&current).map_err(|e| e.to_string())?;
        std::fs::write(BASELINE, text + "\n").map_err(|e| e.to_string())?;
        println!("baseline written; review its diff");
        return Ok(());
    }
    let saved = std::fs::read_to_string(Path::new(BASELINE))
        .map_err(|e| format!("{BASELINE}: {e}; create it with `gate --bless`"))?;
    let saved: Table = serde_json::from_str(&saved).map_err(|e| e.to_string())?;
    let found = differences(&current, &saved);
    if cfg!(target_os = "macos") {
        problems.extend(found);
    } else {
        // The baseline was taken on the reference Mac; elsewhere only the ceilings bind.
        for difference in found {
            println!("note: {difference}");
        }
    }
    if problems.is_empty() {
        println!("{} interactions match the baseline", first.len());
        return Ok(());
    }
    Err(format!(
        "{}\n\nIf this change is meant: cargo run --release -p ravenrs-perf -- gate --bless, and review the diff.",
        problems.join("\n")
    ))
}

fn report(bench: &Bench, filter: Option<&str>) {
    println!(
        "{:<36} {:>8} {:>6} {:>6} {:>7} {:>9} {:>9} {:>6} {:>10} {:>6}",
        "per round",
        "elements",
        "builds",
        "paints",
        "shapes",
        "ui_allocs",
        "instances",
        "draws",
        "bytes",
        "frames"
    );
    for o in run(bench, filter) {
        let get = |name: &str| o.counters.get(name).copied().unwrap_or(0);
        let per = |name: &str| get(name) as f64 / f64::from(COUNTED_ROUNDS);
        println!(
            "{:<36} {:>8} {:>6.1} {:>6.1} {:>7.1} {:>9.1} {:>9.0} {:>6.1} {:>10.0} {:>6}",
            o.name,
            get("elements"),
            per("builds"),
            per("paints"),
            per("shapes"),
            per("ui_allocations"),
            per("instances"),
            per("draw_calls"),
            per("uploaded_bytes"),
            get("frames"),
        );
    }
}

fn percentile(mut values: Vec<Duration>, at: f64) -> Duration {
    if values.is_empty() {
        return Duration::ZERO;
    }
    values.sort();
    values[((values.len() - 1) as f64 * at).round() as usize]
}

fn time(bench: &Bench, filter: Option<&str>) {
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let mut out = serde_json::Map::new();
    for scene in matching(filter) {
        for interaction in scene.interactions {
            let o = bench.measure(&scene, interaction, Some(120));
            let of =
                |pick: fn(&FrameTime) -> Duration| ms(percentile(o.frames.iter().map(pick).collect(), 0.99));
            let gpu: Vec<Duration> = o.frames.iter().filter_map(|f| f.gpu).collect();
            out.insert(
                o.name.clone(),
                serde_json::json!({
                    "elements": o.counters.get("elements"),
                    "frames": o.frames.len(),
                    "ui": of(|f| f.ui),
                    "build": of(|f| f.build),
                    "layout": of(|f| f.layout),
                    "place": of(|f| f.place),
                    "paint": of(|f| f.paint),
                    "render": of(|f| f.render),
                    "gpu": ms(percentile(gpu, 0.99)),
                }),
            );
        }
    }
    println!("{}", serde_json::Value::Object(out));
}

/// How much of each kind of work fits in a frame, on this machine.
fn capacity(bench: &Bench, verbose: bool) {
    println!("{}", bench.adapter());
    println!(
        "{:<14} {:<14} {:>12} {:>12} {:>10} {:>10} {:>8}",
        "work", "counting", "fits 120 Hz", "fits 60 Hz", "cpu share", "ms each k", "draws/k"
    );
    for load in &capacity::LOADS {
        let curve = bench.capacity(load);
        let show = |fit: Option<usize>| fit.map_or_else(|| "all".to_owned(), |n| n.to_string());
        let Some(last) = curve.last() else { continue };
        println!(
            "{:<14} {:<14} {:>12} {:>12} {:>9.0}% {:>10.3} {:>8.0}",
            load.name,
            load.unit,
            show(capacity::fits(&curve, capacity::BUDGET_120)),
            show(capacity::fits(&curve, capacity::BUDGET_60)),
            last.cpu / last.total * 100.0,
            last.total * 1000.0 / last.amount as f64 * 1000.0,
            last.draw_calls as f64 * 1000.0 / last.amount as f64,
        );
        if let Some(drawn) = capacity::drops_from(&curve) {
            println!("    leaves shapes out: only {drawn} of them fit in the coverage atlas");
        }
        if verbose {
            for point in &curve {
                println!(
                    "    {:>9} {:>8.2} ms total {:>8.2} ms cpu {:>8} draws",
                    point.amount,
                    point.total * 1000.0,
                    point.cpu * 1000.0,
                    point.draw_calls
                );
            }
        }
    }
}

/// Compares two runs of `time`, so a change shows what it did to each scene.
fn diff(before: &str, after: &str) -> Result<(), String> {
    let read = |path: &str| -> Result<serde_json::Value, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
    };
    let (before, after) = (read(before)?, read(after)?);
    let Some(scenes) = after.as_object() else { return Err("not a run of `time`".to_owned()) };
    println!("{:<38} {:>10} {:>10} {:>8}", "CPU p99, ms", "before", "after", "change");
    for (name, values) in scenes {
        let (Some(new), Some(old)) = (values["ui"].as_f64(), before[name]["ui"].as_f64()) else { continue };
        if new == 0.0 && old == 0.0 {
            continue;
        }
        let change = if old > 0.0 { (new / old - 1.0) * 100.0 } else { 0.0 };
        println!("{name:<38} {old:>10.2} {new:>10.2} {change:>+7.0}%");
    }
    Ok(())
}

fn show_visits(title: &str, visits: Vec<ravenrs_perf::Visit>) {
    println!("{title:<34} {:>10} {:>7} {:>10}", "worst ms", "frames", "rasterized");
    for visit in visits {
        println!(
            "{:<34} {:>10.2} {:>7} {:>10}",
            visit.name,
            visit.worst.as_secs_f64() * 1000.0,
            visit.frames,
            visit.rasterized
        );
    }
}

/// The first time each screen opens, and the second: where a person sees stutter.
fn cold(bench: &Bench) {
    let (root, page) = scenes::apps::finance_tour();
    let mut visits: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for round in ["first", "second"] {
        for number in (1..8).chain([0]) {
            let page = page.clone();
            visits.push((format!("page {number}, {round} time"), Box::new(move || page.set(number))));
        }
    }
    show_visits("finance app", bench.tour(root, |_| {}, visits));

    let (root, session) = scenes::apps::feature_center();
    let mut visits: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for round in ["first", "second"] {
        for list in scenes::apps::CENTER_LISTS {
            let session = session.clone();
            visits.push((
                format!("{list}, {round} time"),
                Box::new(move || {
                    if let Some(session) = session.borrow().as_ref() {
                        session.open_view(&format!("{list}_ListView"));
                    }
                }),
            ));
        }
    }
    show_visits("feature center", bench.tour(root, scenes::apps::sign_in, visits));
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let filter = args.iter().skip(1).find(|a| !a.starts_with("--")).map(String::as_str);
    // Comparing two files needs no GPU.
    if let (Some("diff"), [_, before, after]) = (args.first().map(String::as_str), args.as_slice()) {
        return match diff(before, after) {
            Ok(()) => ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("{why}");
                ExitCode::FAILURE
            }
        };
    }
    let bench = match Bench::new(allocations) {
        Ok(bench) => bench,
        Err(why) => {
            eprintln!("the gate needs a GPU, and found none: {why}");
            return ExitCode::FAILURE;
        }
    };
    let result = match args.first().map(String::as_str) {
        Some("gate") => gate(&bench, flag("--bless")),
        Some("report") => {
            report(&bench, filter);
            Ok(())
        }
        Some("time") => {
            time(&bench, filter);
            Ok(())
        }
        Some("capacity") => {
            capacity(&bench, flag("--curve"));
            Ok(())
        }
        Some("cold") => {
            cold(&bench);
            Ok(())
        }
        _ => Err("use: ravenrs-perf gate [--bless] | report [filter] | time [filter] | cold | capacity [--curve] | diff before.json after.json".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
