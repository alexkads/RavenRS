use raven_ci_core::JobSpec;
use raven_ci_plugin_api::{Attachment, BuildRequest, BuildStatus};
use ravenrs_flows::ManualClock;
use serde_json::json;

use crate::common::{ci_at, ready};

const DAY: i64 = 24 * 60 * 60 * 1000;

fn ready_at(clock: &ManualClock) -> raven_ci_core::Ci {
    let ci = ci_at(clock);
    ci.add_plugin(std::sync::Arc::new(raven_ci_plugin_api::testing::Fake::new(
        raven_ci_plugin_api::Point::Step,
    )));
    ci.add_plugin(std::sync::Arc::new(raven_ci_plugin_api::testing::Fake::new(
        raven_ci_plugin_api::Point::PipelineFormat,
    )));
    ci.apply_plugins().expect("on");
    ci
}

#[test]
fn a_job_in_a_path_is_in_its_folder_and_the_folders_above() {
    let ci = ready();
    ci.save_job(JobSpec::new("apps/site", "fake-lines", "Build: ok")).expect("saved");
    ci.save_job(JobSpec::new("apps/web/painel", "fake-lines", "Build: ok")).expect("saved");
    ci.save_job(JobSpec::new("solto", "fake-lines", "Build: ok")).expect("saved");
    ci.set_folder_title("apps", "Aplicativos").expect("named");
    let folders = ci.folders().expect("listed");
    let paths: Vec<(&str, Option<&str>, Vec<&str>)> = folders
        .iter()
        .map(|f| (f.path.as_str(), f.title.as_deref(), f.jobs.iter().map(String::as_str).collect()))
        .collect();
    assert_eq!(
        paths,
        [
            ("", None, vec!["solto"]),
            ("apps", Some("Aplicativos"), vec!["apps/site"]),
            ("apps/web", None, vec!["apps/web/painel"]),
        ]
    );
    assert!(ci.save_job(JobSpec::new("apps/../x", "fake-lines", "")).is_err(), "no way out of a folder");
    assert!(ci.save_job(JobSpec::new("a b", "fake-lines", "")).is_err());
}

#[test]
fn the_stages_come_in_the_order_of_the_pipeline_with_each_step_status() {
    let ci = ready();
    ci.save_job(JobSpec::new(
        "site",
        "fake-lines",
        "Build: compila\nBuild: empacota\nTeste: !fail\nFim: nunca",
    ))
    .expect("saved");
    let number = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    let before = ci.stages("site", number).expect("stages");
    assert!(before.iter().all(|stage| stage.status == "pending"));
    ci.work_until_idle("e1").expect("runs");
    let stages = ci.stages("site", number).expect("stages");
    let names: Vec<(&str, &str, usize)> =
        stages.iter().map(|s| (s.name.as_str(), s.status.as_str(), s.steps.len())).collect();
    assert_eq!(names, [("Build", "done", 2), ("Teste", "failed", 1), ("Fim", "pending", 1)]);
    assert_eq!(stages[1].steps[0].error.as_deref(), Some("o eco falhou, como pedido"));
    let build = ci.build("site", number).expect("kept");
    assert_eq!(build.status, BuildStatus::Failed);
    assert_eq!(build.reason.as_deref(), Some("o passo step-3 falhou: o eco falhou, como pedido"));
}

#[test]
fn keeping_two_builds_forgets_the_first_with_what_it_left() {
    let ci = ready();
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok").keep_builds(2).every_request())
        .expect("saved");
    let first = ci.request_build(BuildRequest::new("site", "um")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let id = ci
        .attach("site", first, &Attachment::new("resumo", "resumo.md", "text/markdown", b"# ok".to_vec()))
        .expect("kept");
    assert_eq!(ci.attachment_bytes(id).expect("bytes"), b"# ok");
    for cause in ["dois", "três"] {
        ci.request_build(BuildRequest::new("site", cause)).expect("asked");
        ci.work_until_idle("e1").expect("runs");
    }
    let numbers: Vec<u64> = ci.builds("site").expect("listed").iter().map(|b| b.number).collect();
    assert_eq!(numbers, [3, 2]);
    assert!(ci.build("site", first).is_err());
    assert!(ci.attachments("site", first).expect("listed").is_empty());
    assert!(ci.attachment_bytes(id).is_err(), "the bytes went with it");
}

#[test]
fn keeping_one_day_forgets_older_builds_when_the_clock_moves() {
    let clock = ManualClock::at(0);
    let ci = ready_at(&clock);
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok").keep_days(1).every_request()).expect("saved");
    ci.request_build(BuildRequest::new("site", "velho")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    clock.advance(2 * DAY);
    ci.request_build(BuildRequest::new("site", "novo")).expect("asked");
    ci.work_until_idle("e1").expect("runs");
    let causes: Vec<String> = ci.builds("site").expect("listed").into_iter().map(|b| b.cause).collect();
    assert_eq!(causes, ["novo"]);
    assert_eq!(ci.prune().expect("prunes"), 0, "nothing more to forget");
}

#[test]
fn an_attachment_keeps_its_summary_for_the_screens() {
    let ci = ready();
    ci.save_job(JobSpec::new("site", "fake-lines", "Build: ok")).expect("saved");
    let number = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    let junit = Attachment::new("junit", "relatorio.xml", "application/xml", b"<testsuite/>".to_vec())
        .summary(json!({ "passed": 41, "failed": 1 }));
    ci.attach("site", number, &junit).expect("kept");
    let kept = ci.attachments("site", number).expect("listed");
    assert_eq!((kept[0].kind.as_str(), kept[0].size), ("junit", 12));
    assert_eq!(kept[0].summary["failed"], 1);
}
