//! The Studio's own scenario, "O Studio testa o Studio", run the two ways it is run: by the
//! Studio's engine with no window (what the CLI, the pane and the AI tool do), and by pressing
//! "Rodar" on a Studio driven through `ravenrs-e2e`.

use raven_test_studio::cache::RunCache;
use raven_test_studio::{app, run_bundled};
use ravenrs::prelude::*;
use ravenrs_e2e::E2e;

#[test]
fn the_studio_passes_its_own_test() {
    let report = run_bundled("autoteste", None).expect("the scenario comes with the Studio");
    assert!(report.passed(), "{}", report.text());
    assert_eq!(report.outcomes.len(), 18);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn pressing_run_in_the_studio_runs_its_own_test_to_the_end() {
    let mut studio = E2e::new(app(), Theme::raven_dark()).viewport(Size::new(1440.0, 900.0), 1.0);
    studio.by_test_id("summary").expect().to_have_name("Ainda não rodou").unwrap();
    studio.by_test_id("run").click().unwrap();
    studio.by_test_id("summary").expect().to_have_name("18 de 18 passos passaram").unwrap();
    studio.by_test_id("node:skipped").expect().to_mention("Concluído").unwrap();
}

#[test]
fn the_window_tests_itself_as_it_opens_and_says_so_in_its_status_bar() {
    let mut studio = E2e::new(raven_test_studio::self_checked_app(None), Theme::raven_dark())
        .viewport(Size::new(1440.0, 900.0), 1.0);
    studio.by_test_id("health").expect().to_have_name("Autoteste: 18 de 18 passos passaram").unwrap();
    // A Studio a scenario opens does not test itself again, or the self-test would never end.
    let mut inner = E2e::new(app(), Theme::raven_dark()).viewport(Size::new(1440.0, 900.0), 1.0);
    assert_eq!(inner.by_test_id("health").count().unwrap(), 0);
}

#[test]
fn a_broken_scenario_reports_the_code_the_fix_and_the_screen() {
    let report = run_bundled("quebrado", None).unwrap();
    assert!(!report.passed());
    assert_eq!(report.exit_code(), 1);
    let failed = report.failure().unwrap();
    assert_eq!(failed.code.as_deref(), Some("E2E_NOT_FOUND"));
    assert!(failed.fix.is_some(), "the fix comes from ravenrs-e2e");
    assert!(
        failed.screen.as_deref().is_some_and(|s| s.contains("#salvar")),
        "the screen of the app under test"
    );
    let json = report.json();
    assert_eq!(json["verdict"], "failed");
    assert_eq!(json["blocks"][1]["code"], "E2E_NOT_FOUND");
    assert!(report.text().contains("não verificado"));
}

#[test]
fn a_run_comes_back_from_the_cache_until_the_scenario_or_the_build_changes() {
    let dir = std::env::temp_dir().join(format!("raven-test-studio-cache-{}", std::process::id()));
    let file = dir.join("runs.json");
    let mut cache = RunCache::at(&file);
    let first = run_bundled("lancamento", Some(&mut cache)).unwrap();
    assert!(first.cached_at.is_none(), "the first run runs");
    let mut again = RunCache::at(&file);
    let second = run_bundled("lancamento", Some(&mut again)).unwrap();
    assert!(second.cached_at.is_some(), "the same scenario on the same build comes from the cache");
    assert_eq!(second.outcomes, first.outcomes);
    assert_eq!(second.json()["from_cache"], true);
    let other = RunCache::key(&raven_test_studio::sample::scenario(), Some("another-build"));
    assert!(again.get(&other.unwrap()).is_none(), "another build is not the same key");
    again.clear().unwrap();
    assert!(!file.exists());
    let _ = std::fs::remove_dir_all(dir);
}
