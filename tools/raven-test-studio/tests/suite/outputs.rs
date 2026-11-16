//! What a run leaves for whoever reads it later (#661): JUnit for build servers, a
//! `trace.jsonl` with the time of each step, and the screen of the app when it failed, kept in
//! `target/e2e/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use raven_test_studio::report::{Report, folder_name};
use raven_test_studio::run_named;
use serde_json::Value;

fn kept(file: &str) -> String {
    format!("{}/tests/e2e/amostras/{file}", env!("CARGO_MANIFEST_DIR"))
}

/// A folder of its own for one test, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("raven-test-studio-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn broken() -> Report {
    run_named(&kept("quebrado.raventest"), None).unwrap()
}

/// `report` as if every block of it had passed.
fn passed(report: &Report) -> Report {
    let mut good = report.clone();
    for outcome in &mut good.outcomes {
        outcome.error = None;
        outcome.code = None;
        outcome.screen = None;
    }
    good.not_run.clear();
    good.total = good.outcomes.len();
    good
}

#[test]
fn each_step_says_how_long_it_took_and_the_cache_keeps_it() {
    let report = run_named(&kept("lancar-despesa.raventest"), None).unwrap();
    assert!(report.passed(), "{}", report.text());
    let sum: u64 = report.outcomes.iter().map(|o| o.ms).sum();
    assert!(sum <= report.duration_ms, "the steps fit in the run: {sum} > {}", report.duration_ms);
    let json = report.json();
    assert!(json["blocks"][0]["duration_ms"].is_u64());
    let back = Report::from_json(&json).unwrap();
    assert_eq!(
        back.outcomes.iter().map(|o| o.ms).collect::<Vec<_>>(),
        report.outcomes.iter().map(|o| o.ms).collect::<Vec<_>>()
    );
}

#[test]
fn junit_marks_the_failure_with_its_code_and_what_came_after_as_skipped() {
    let report = broken();
    assert_eq!(report.not_run, vec!["Salvar".to_owned()], "{}", report.text());
    let xml = Report::junit(std::slice::from_ref(&report));
    assert!(xml.starts_with("<?xml"), "{xml}");
    assert!(xml.contains("tests=\"3\" failures=\"1\" skipped=\"1\""), "{xml}");
    assert!(xml.contains("<failure type=\"E2E_NOT_FOUND\""), "{xml}");
    assert!(xml.contains("name=\"2 Clicar no que não existe: #nao-existe\""), "{xml}");
    assert!(xml.contains("name=\"3 Salvar\""), "{xml}");
    assert!(xml.contains("<skipped"), "{xml}");
    // The screen goes in the failure, escaped: a tree line starts with `#` and may hold `<`.
    assert!(xml.contains("tela do app testado:"), "{xml}");
    assert_eq!(xml.matches("<testcase").count(), 3);
    assert_eq!(xml.matches("</testsuite>").count(), 1);

    let good = Report::junit(&[passed(&report)]);
    assert!(good.contains("failures=\"0\" skipped=\"0\""), "{good}");
    assert!(!good.contains("<failure"), "{good}");
}

#[test]
fn junit_escapes_what_xml_does_not_take_as_is() {
    let mut report = passed(&broken());
    report.scenario = "Pedido <novo> & \"rápido\" de 'hoje'\u{7}".to_owned();
    let xml = Report::junit(&[report]);
    assert!(
        xml.contains("name=\"Pedido &lt;novo&gt; &amp; &quot;rápido&quot; de &apos;hoje&apos;\""),
        "{xml}"
    );
}

#[test]
fn the_trace_is_one_json_event_per_line() {
    let report = broken();
    let trace = report.trace();
    let events: Vec<Value> = trace.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let kinds: Vec<&str> = events.iter().map(|e| e["event"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["start", "step", "step", "skipped", "end"]);
    assert_eq!(events[2]["code"], "E2E_NOT_FOUND");
    assert_eq!(events[3]["title"], "Salvar");
    assert_eq!(events[4]["verdict"], "failed");
    assert!(events[1]["duration_ms"].is_u64());
}

#[test]
fn a_failure_keeps_the_screen_and_a_pass_takes_it_away() {
    let out = scratch("save");
    let report = broken();
    let dir = report.save(&out).unwrap();
    assert_eq!(dir, out.join("Cenário-quebrado"));
    let tree = std::fs::read_to_string(dir.join("arvore.txt")).unwrap();
    assert!(tree.contains("#salvar"), "{tree}");
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("relatorio.json")).unwrap()).unwrap();
    assert_eq!(json["verdict"], "failed");
    assert_eq!(std::fs::read_to_string(dir.join("trace.jsonl")).unwrap(), report.trace());

    passed(&report).save(&out).unwrap();
    assert!(!dir.join("arvore.txt").exists(), "an old failure's screen does not outlive a pass");
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_scenario_name_never_leaves_the_folder_given() {
    assert_eq!(folder_name("../../etc/passwd"), "etc-passwd");
    assert_eq!(folder_name("Lançar despesa"), "Lançar-despesa");
    assert_eq!(folder_name("2 cenários de /tmp/x"), "2-cenários-de-tmp-x");
    assert_eq!(folder_name("///"), "cenario");
}

fn studio(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_raven-test-studio")).args(args).output().unwrap()
}

#[test]
fn the_command_line_prints_junit_and_jsonl_and_keeps_the_run_with_out() {
    let out = scratch("cli");
    let file = kept("quebrado.raventest");
    let folder = out.to_str().unwrap();
    let run = studio(&["--run", &file, "--format", "junit", "--out", folder, "--no-cache"]);
    assert_eq!(run.status.code(), Some(1));
    let stdout = String::from_utf8(run.stdout).unwrap();
    assert!(stdout.starts_with("<?xml") && stdout.contains("E2E_NOT_FOUND"), "{stdout}");
    let stderr = String::from_utf8(run.stderr).unwrap();
    assert!(stderr.contains("gravado em"), "where it was kept goes to stderr: {stderr}");
    assert!(Path::new(folder).join("Cenário-quebrado").join("arvore.txt").exists());

    let run = studio(&["--run", &file, "--format", "jsonl", "--no-cache"]);
    assert_eq!(run.status.code(), Some(1));
    let stdout = String::from_utf8(run.stdout).unwrap();
    assert!(stdout.lines().all(|l| serde_json::from_str::<Value>(l).is_ok()), "{stdout}");
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn an_unknown_format_is_refused_with_the_ones_it_takes() {
    let run = studio(&["--run", &kept("quebrado.raventest"), "--format", "xml"]);
    assert_eq!(run.status.code(), Some(2));
    let stderr = String::from_utf8(run.stderr).unwrap();
    assert!(stderr.contains("text, json, jsonl, junit"), "{stderr}");
}

#[test]
fn a_project_keeps_its_runs_where_cargo_builds_it() {
    use raven_test_studio::project::of_metadata;
    let root = Path::new("/src/loja/apps/caixa");
    let metadata = serde_json::json!({ "packages": [], "target_directory": "/src/loja/target" });
    assert_eq!(of_metadata(root, &metadata).e2e_folder(), Path::new("/src/loja/target/e2e"));
    let metadata = serde_json::json!({ "packages": [] });
    assert_eq!(of_metadata(root, &metadata).e2e_folder(), root.join("target").join("e2e"));
}
