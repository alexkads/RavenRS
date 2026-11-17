//! Running `.raventest` files straight from their blocks, with no app compiled in: what
//! `cargo ravenrs e2e run` does.
#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use std::path::{Path, PathBuf};

use ravenrs_e2e_runner::report::{FORMATS, render};
use ravenrs_e2e_runner::run::{files_at, run_files};
use ravenrs_e2e_runner::session::{Session, Step};
use ravenrs_e2e_wire::{Block, Command, Kind};

/// Knows no app: every Open block fails.
fn nothing(_: &str) -> Option<Result<Session, String>> {
    None
}

fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ravenrs-e2e-runner-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

const TWO_STEPS: &str =
    "scenario: Pedido\nopen Loja\nclick #salvar | Salvar\ncheck #aviso = Salvo | Conferir\n";

#[test]
fn an_app_no_one_knows_fails_with_a_code_and_the_rest_does_not_run() {
    let dir = scratch("unknown", &[("a.raventest", TWO_STEPS)]);
    let report = run_files(&dir, nothing).unwrap();
    assert_eq!(report.exit_code(), 1);
    let failed = report.failure().unwrap();
    assert_eq!(failed.code.as_deref(), Some("E2E_APP_START"));
    assert!(failed.fix.as_deref().is_some_and(|f| f.contains("package.metadata.ravenrs.e2e")));
    assert_eq!(report.not_run, ["Salvar", "Conferir"]);
    assert_eq!(report.total, 3);
    let xml = render(std::slice::from_ref(&report), false, "junit");
    assert!(xml.contains("<failure type=\"E2E_APP_START\""), "{xml}");
    assert_eq!(xml.matches("<skipped").count(), 2, "{xml}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_runs_its_files_in_name_order_under_one_title() {
    let dir = scratch("order", &[("b.raventest", TWO_STEPS), ("a.raventest", TWO_STEPS), ("notas.txt", "x")]);
    let files = files_at(&dir).unwrap();
    let names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["a.raventest", "b.raventest"]);
    let report = run_files(&dir, nothing).unwrap();
    assert!(report.scenario.starts_with("2 cenários de "), "{}", report.scenario);
    assert_eq!(report.total, 6);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn what_cannot_be_read_says_the_file_and_the_line() {
    let dir = scratch("broken", &[("a.raventest", "scenario: x\nopen Loja\npular #a\n")]);
    let why = run_files(&dir, nothing).unwrap_err();
    assert!(why.contains("a.raventest") && why.contains("linha 3"), "{why}");
    assert!(files_at(Path::new("/não/existe")).is_err());
    let empty = scratch("empty", &[]);
    assert!(files_at(&empty).unwrap_err().contains("não tem arquivos"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&empty);
}

#[test]
fn a_block_becomes_its_step_or_says_what_is_missing() {
    let block = |kind, target: &str, text: Option<&str>| Block {
        kind,
        target: target.into(),
        text: text.map(Into::into),
        title: String::new(),
    };
    assert_eq!(Step::of(&block(Kind::Open, "Loja", None)), Ok(Step::Open("Loja".into())));
    assert_eq!(
        Step::of(&block(Kind::Click, "#salvar", None)),
        Ok(Step::Do(Command::Click { target: "#salvar".into() }))
    );
    assert!(Step::of(&block(Kind::Type, "#nome", None)).unwrap_err().contains("texto"));
    assert!(Step::of(&block(Kind::Drag, "#a", None)).unwrap_err().contains("arrastar"));
    assert!(Step::of(&block(Kind::Click, " ", None)).unwrap_err().contains("alvo"));
}

#[test]
fn every_format_renders() {
    let dir = scratch("formats", &[("a.raventest", TWO_STEPS)]);
    let report = run_files(&dir, nothing).unwrap();
    for format in FORMATS {
        assert!(!render(std::slice::from_ref(&report), false, format).is_empty(), "{format}");
    }
    let one: serde_json::Value =
        serde_json::from_str(&render(std::slice::from_ref(&report), false, "json")).unwrap();
    assert_eq!(one["verdict"], "failed");
    let list: serde_json::Value =
        serde_json::from_str(&render(std::slice::from_ref(&report), true, "json")).unwrap();
    assert!(list.is_array());
    let _ = std::fs::remove_dir_all(&dir);
}
