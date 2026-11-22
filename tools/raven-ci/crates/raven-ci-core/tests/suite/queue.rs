use std::sync::Arc;
use std::time::{Duration, Instant};

use raven_ci_core::{Ci, Executors, JobSpec};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{BuildRequest, BuildStatus, Point};
use serde_json::json;

use crate::common::ready;

#[test]
fn the_build_of_priority_ten_leaves_before_the_one_that_came_first() {
    let ci = ready();
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    ci.save_job(JobSpec::new("urgente", "fake-lines", "Build: ok")).expect("saved");
    let first = ci.request_build(BuildRequest::new("site", "antes")).expect("asked");
    let urgent = ci.request_build(BuildRequest::new("urgente", "depois").priority(10)).expect("asked");
    let line: Vec<String> = ci.queue().expect("listed").into_iter().map(|b| b.job).collect();
    assert_eq!(line, ["urgente", "site"]);
    assert_eq!(ci.work_once("e1").expect("runs"), Some(("urgente".to_owned(), urgent.number)));
    assert_eq!(ci.work_once("e1").expect("runs"), Some(("site".to_owned(), first.number)));
}

#[test]
fn three_equal_requests_in_a_row_make_one_build() {
    let ci = ready();
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    let tickets: Vec<_> = (0..3)
        .map(|_| {
            ci.request_build(BuildRequest::new("site", "push").parameter("ramo", json!("main")))
                .expect("asked")
        })
        .collect();
    assert_eq!(
        tickets.iter().map(|t| (t.number, t.joined)).collect::<Vec<_>>(),
        [(1, false), (1, true), (1, true)]
    );
    assert_eq!(ci.build("site", 1).expect("kept").joined, 2);
    let other =
        ci.request_build(BuildRequest::new("site", "push").parameter("ramo", json!("dev"))).expect("asked");
    assert_eq!((other.number, other.joined), (2, false), "other parameters make another build");
    assert_eq!(ci.work_until_idle("e1").expect("runs"), 2);
}

#[test]
fn a_build_cancelled_in_the_queue_never_runs() {
    let ci = ready();
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    let number = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    assert_eq!(ci.cancel("site", number, "pedido errado").expect("cancels"), BuildStatus::Cancelled);
    assert_eq!(ci.work_until_idle("e1").expect("runs"), 0);
    assert_eq!(ci.build("site", number).expect("kept").reason.as_deref(), Some("pedido errado"));
}

#[test]
fn a_build_cancelled_while_it_runs_stops_its_step_through_the_token() {
    let ci = Arc::new(ready());
    ci.save_job(JobSpec::new("lento", "fake-lines", "Espera: ~60000")).expect("saved");
    let number = ci.request_build(BuildRequest::new("lento", "teste")).expect("asked").number;
    let executors = Executors::start(ci.clone(), 1, Duration::from_millis(10));
    let began = Instant::now();
    while ci.build("lento", number).expect("kept").status != BuildStatus::Running {
        assert!(began.elapsed() < Duration::from_secs(10), "it started");
        std::thread::sleep(Duration::from_millis(10));
    }
    ci.cancel("lento", number, "parado pela Ana").expect("asks");
    while ci.build("lento", number).expect("kept").status == BuildStatus::Running {
        assert!(began.elapsed() < Duration::from_secs(10), "it did not wait the minute out");
        std::thread::sleep(Duration::from_millis(10));
    }
    executors.stop();
    let build = ci.build("lento", number).expect("kept");
    assert_eq!(build.status, BuildStatus::Cancelled);
    assert_eq!(build.reason.as_deref(), Some("parado pela Ana"));
}

#[test]
fn a_rebuild_gets_a_new_number_with_the_same_parameters() {
    let ci = ready();
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    let first = ci
        .request_build(BuildRequest::new("site", "push").parameter("E2E", json!(true)).by("ana"))
        .expect("asked")
        .number;
    ci.work_until_idle("e1").expect("runs");
    let again = ci.rebuild("site", first, Some("bia")).expect("asked again");
    assert_eq!(again, 2);
    let build = ci.build("site", again).expect("kept");
    assert_eq!(build.parameters["E2E"], true);
    assert_eq!(
        (build.origin, build.actor.as_deref(), build.cause.as_str()),
        (Some(1), Some("bia"), "repetição do #1")
    );
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.build("site", again).expect("kept").status, BuildStatus::Succeeded);
}

#[test]
fn moving_a_build_in_the_queue_changes_which_leaves_first() {
    let ci = ready();
    ci.save_job(JobSpec::new("a", "fake-lines", "Build: ok")).expect("saved");
    ci.save_job(JobSpec::new("b", "fake-lines", "Build: ok")).expect("saved");
    ci.request_build(BuildRequest::new("a", "x")).expect("asked");
    let b = ci.request_build(BuildRequest::new("b", "x")).expect("asked").number;
    ci.set_priority("b", b, 5).expect("moved");
    assert_eq!(ci.work_once("e1").expect("runs").map(|(job, _)| job).as_deref(), Some("b"));
}

#[test]
fn with_no_plugin_the_core_answers_and_says_what_is_missing() {
    let ci = Ci::in_memory().expect("a core with no plugin");
    assert!(ci.registry().list().expect("listed").is_empty());
    ci.save_job(JobSpec::new("site", "toml", "[stages]")).expect("a job can be kept");
    let error = ci.request_build(BuildRequest::new("site", "teste")).expect_err("nothing reads it");
    assert_eq!(error.to_string(), "nenhum plugin ligado lê pipelines do formato toml");

    ci.add_plugin(Arc::new(Fake::new(Point::PipelineFormat)));
    ci.apply_plugins().expect("on");
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    let error = ci.request_build(BuildRequest::new("site", "teste")).expect_err("no step");
    assert_eq!(error.to_string(), "o passo step-1 usa fake-echo, que nenhum plugin ligado oferece");
    assert!(ci.request_build(BuildRequest::new("nao-existe", "teste")).is_err());
}

#[test]
fn a_step_switched_off_after_a_build_was_asked_ends_it_as_failed_saying_why() {
    let ci = ready();
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    let number = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    ci.disable_plugin("fake-step").expect("off");
    ci.work_until_idle("e1").expect("runs");
    let build = ci.build("site", number).expect("kept");
    assert_eq!(build.status, BuildStatus::Failed);
    assert!(build.reason.as_deref().is_some_and(|why| why.contains("fake-echo")), "{:?}", build.reason);
}

#[test]
fn a_trigger_asks_the_core_for_builds() {
    let ci = Arc::new(ready());
    let trigger = Fake::new(Point::Trigger);
    let probe = trigger.probe();
    ci.add_plugin(Arc::new(trigger));
    ci.apply_plugins().expect("on");
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    let on = ci.registry().on();
    let watch = on.iter().flat_map(|p| p.triggers()).next().expect("the trigger");
    watch.watch("site", &json!({}), ci.clone()).expect("watches");
    assert_eq!(probe.fire("site").expect("asks"), 1);
    assert_eq!(ci.build("site", 1).expect("kept").cause, "disparado pelo gatilho falso");
}
