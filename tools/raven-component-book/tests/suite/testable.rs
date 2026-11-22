//! Every story of every component can be found by a test id the app gives it (#737): the id
//! reaches a node of the accessibility tree, so a scenario of the Raven Test Studio finds the
//! component without knowing how it is built. It is asked of every component the contract of the
//! RTS (`tools/raven-test-studio/contrato.json`) lists as keeping it.
#![allow(clippy::print_stdout, reason = "the list is the answer to who asks for it")]

use ravenrs::prelude::*;
use ravenrs::widgets::book::all;
use ravenrs_e2e::E2e;

/// The component of a title such as "Layout/Card".
fn name(title: &str) -> &str {
    title.rsplit('/').next().unwrap_or(title)
}

/// The components that no story of theirs lets a test id reach: the one the app gives it, or a
/// fixed id of a part the contract says it has (a part in a layer of its own, as the sphere of a
/// floating panel).
fn unreachable(fixed: &std::collections::HashMap<String, Vec<String>>) -> Vec<String> {
    let mut out = Vec::new();
    for meta in all() {
        let own = fixed.get(name(meta.title)).cloned().unwrap_or_default();
        let found = meta.stories.iter().any(|story| {
            let view = meta.render(story, &story.args).test_id("probe");
            let mut e2e = E2e::new(view, Theme::raven_dark()).viewport(Size::new(1200.0, 800.0), 1.0);
            e2e.by_test_id("probe").count().unwrap_or(0) == 1
                || own.iter().any(|id| e2e.by_test_id(id.clone()).count().unwrap_or(0) == 1)
        });
        if !found {
            out.push(name(meta.title).to_owned());
        }
    }
    out.sort();
    out
}

#[test]
fn a_test_id_reaches_every_component_but_those_with_nothing_to_find() {
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("../../../raven-test-studio/contrato.json")).expect("the contract");
    // What the contract says keeps it (a complete entry) has to be reached; what has no screen
    // or still waits for its family's issue is not asked yet.
    let complete: Vec<&str> = contract["tipos"]
        .as_array()
        .expect("the types")
        .iter()
        .filter(|t| t["rts"]["ids"].is_array())
        .filter_map(|t| t["tipo"].as_str())
        .collect();
    assert!(complete.len() >= 60, "the contract lists what keeps it");
    // The ids of single parts (`orb`, `close`), which a component gives itself.
    let fixed: std::collections::HashMap<String, Vec<String>> = contract["tipos"]
        .as_array()
        .expect("the types")
        .iter()
        .filter_map(|t| {
            let ids: Vec<String> = t["rts"]["ids"]
                .as_array()?
                .iter()
                .filter_map(|i| i.as_str())
                .filter(|i| !i.contains(':'))
                .map(str::to_owned)
                .collect();
            Some((t["tipo"].as_str()?.to_owned(), ids))
        })
        .collect();
    let missing: Vec<String> =
        unreachable(&fixed).into_iter().filter(|n| complete.contains(&n.as_str())).collect();
    println!("not reached: {missing:?}");
    assert!(missing.is_empty(), "a test id does not reach: {missing:?}");
}
