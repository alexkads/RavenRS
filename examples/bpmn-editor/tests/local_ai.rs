//! The local model drafting a real process. It needs a model on disk, so it runs only when
//! asked: `RAVENRS_MODELS=<folder> RAVENRS_MODEL=lfm3-1.7b-q4 cargo test --manifest-path examples/bpmn-editor/Cargo.toml --test
//! local_ai -- --ignored --nocapture`.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ravenrs::prelude::UiWaker;
use ravenrs_ai::{LocalAssistant, ModelFolder, model_by_id};
use ravenrs_flow_bpmn::{plan, rules};

fn ask(assistant: &LocalAssistant, system: &str, user: &str) -> String {
    let (text, done) = (Arc::new(Mutex::new(String::new())), Arc::new(AtomicBool::new(false)));
    assistant.answer_long(system.to_owned(), user.to_owned(), text.clone(), done.clone(), UiWaker::default());
    let started = Instant::now();
    while !done.load(Ordering::Acquire) {
        assert!(started.elapsed() < Duration::from_secs(600), "the model took too long");
        std::thread::sleep(Duration::from_millis(200));
    }
    eprintln!("answered in {:.1}s", started.elapsed().as_secs_f32());
    text.lock().map(|t| t.clone()).unwrap_or_default()
}

#[test]
#[ignore = "needs a local model on disk"]
fn a_local_model_drafts_a_process_that_holds_up() {
    let folder = std::env::var("RAVENRS_MODELS").expect("RAVENRS_MODELS: the folder of the models");
    let id = std::env::var("RAVENRS_MODEL").unwrap_or_else(|_| "lfm3-1.7b-q4".into());
    let model = model_by_id(&id).expect("a model of the catalog");
    let assistant = LocalAssistant::new(ModelFolder::new(folder), model);
    let description = "Quando chega um pedido de reembolso, o funcionário envia os comprovantes. \
        O gestor analisa e decide se aprova. Se aprovar, o financeiro paga e o processo termina. \
        Se não aprovar, o funcionário é avisado e o processo termina.";
    let answer = ask(&assistant, plan::DRAFT_RULES, description);
    eprintln!("--- answer ---\n{answer}\n---");
    let drafted = plan::plan(&answer).expect("the answer is a plan");
    eprintln!("mended: {:?}", drafted.mended);
    let graph = drafted.graph;
    let kinds: Vec<&str> = graph.nodes.iter().map(|n| n.kind.as_str()).collect();
    eprintln!("kinds: {kinds:?}");
    let problems: Vec<String> = rules::problems(&graph).into_iter().map(|p| p.text).collect();
    eprintln!("problems: {problems:?}");
    assert!(kinds.contains(&"startEvent") && kinds.contains(&"endEvent"), "it starts and ends");
    assert!(kinds.contains(&"exclusiveGateway"), "the manager's decision is a gateway");
    assert!(graph.edges.len() + 1 >= graph.nodes.len(), "the steps are joined");

    let review = ask(&assistant, plan::REVIEW_RULES, &plan::summary(&graph));
    eprintln!("--- review ---\n{review}");
    assert!(!review.trim().is_empty());
}
