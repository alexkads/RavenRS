//! The step `cargo` (#911) on a crate the test makes, compiled for real in the core: the
//! diagnostics in the log and in the attachment, the target of the job kept between builds, and
//! the refusals.
#![allow(clippy::expect_used, clippy::panic, reason = "tests fail loudly")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, PipelineFormat};
use raven_ci_plugin_cargo::{ATTACHMENT_KIND, CargoPlugin};
use raven_ci_plugin_format_toml::{TomlFormat, TomlFormatPlugin};
use serde_json::json;

/// A folder of the test, removed at its end.
struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn folder(what: &str) -> Folder {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("raven-ci-cargo-{what}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    Folder(dir)
}

/// A crate in `dir` whose library is `code`.
fn crate_with(dir: &Path, code: &str) -> PathBuf {
    std::fs::create_dir_all(dir.join("src")).expect("src");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"exemplo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("manifest");
    std::fs::write(dir.join("src/lib.rs"), code).expect("code");
    dir.join("Cargo.toml")
}

// The warning is of a lint that runs before the types are checked: with an error of types,
// rustc does not run the later ones (an unused variable), so the test would see no warning.
const BROKEN: &str = "pub fn dois() -> u32 {\n    let _sobra = (1);\n    \"dois\"\n}\n";
const FIXED: &str = "pub fn dois() -> u32 {\n    2\n}\n";

fn ci(data: &Path) -> Ci {
    let ci = Ci::open(data).expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(CargoPlugin::new()));
    ci.apply_plugins().expect("on");
    ci
}

fn pipeline(manifest: &Path) -> String {
    format!(
        "[[stages]]\nname = \"Rust\"\nsteps = [{{ cargo = \"check --manifest-path '{}'\" }}]\n",
        manifest.display()
    )
}

/// Runs a build of `job`; its status, its log and the summary of its attachment `cargo`.
fn run(
    ci: &Ci,
    job: &str,
    text: &str,
) -> (BuildStatus, Vec<String>, Option<serde_json::Value>, Option<String>) {
    ci.save_job(JobSpec::new(job, "toml", text).every_request()).expect("saved");
    let number = ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let build = ci.build(job, number).expect("build");
    let log = ci.build_log(job, number, 0).expect("log").into_iter().map(|l| l.text).collect();
    let summary = ci
        .attachments_of(job, Some(number), ATTACHMENT_KIND)
        .expect("attachments")
        .into_iter()
        .next()
        .map(|a| a.summary);
    (build.status, log, summary, build.reason)
}

fn has_cargo() -> bool {
    std::process::Command::new("cargo").arg("--version").output().is_ok_and(|o| o.status.success())
}

#[test]
fn an_error_and_a_warning_go_to_the_log_and_to_the_attachment_with_their_place() {
    if !has_cargo() {
        eprintln!("pulado: sem cargo nesta máquina");
        return;
    }
    let (data, code) = (folder("dados"), folder("crate"));
    let ci = ci(&data.0);
    let manifest = crate_with(&code.0, BROKEN);
    let (status, log, summary, reason) = run(&ci, "rust/quebrado", &pipeline(&manifest));
    assert_eq!(status, BuildStatus::Failed, "{log:#?}");
    let reason = reason.unwrap_or_default();
    assert!(reason.contains("o cargo terminou com o código 101"), "{reason}");
    assert!(reason.contains("src/lib.rs:3:5: mismatched types"), "{reason}");
    let text = log.join("\n");
    assert!(text.contains("mismatched types"), "{text}");
    assert!(text.contains("unnecessary parentheses"), "{text}");
    assert!(text.contains("1 erro(s) e 1 aviso(s)"), "{text}");
    assert!(!text.contains("\"reason\":"), "the JSON of cargo stays out of the log: {text}");
    assert_eq!(summary, Some(json!({ "errors": 1, "warnings": 1, "steps": 1 })));

    // The same crate, fixed: it passes, with nothing to say.
    std::fs::write(code.0.join("src/lib.rs"), FIXED).expect("fixed");
    let (status, log, summary, _) = run(&ci, "rust/quebrado", &pipeline(&manifest));
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    assert_eq!(summary, Some(json!({ "errors": 0, "warnings": 0, "steps": 0 })));
}

#[test]
fn the_builds_of_a_job_share_its_target_and_another_job_has_its_own() {
    if !has_cargo() {
        eprintln!("pulado: sem cargo nesta máquina");
        return;
    }
    let (data, code) = (folder("dados"), folder("crate"));
    let ci = ci(&data.0);
    let manifest = crate_with(&code.0, FIXED);
    let target_of = |log: &[String]| {
        log.iter().find_map(|l| l.trim().strip_prefix("CARGO_TARGET_DIR do job: ").map(PathBuf::from))
    };
    let (status, first, _, _) = run(&ci, "apps/site", &pipeline(&manifest));
    assert_eq!(status, BuildStatus::Succeeded, "{first:#?}");
    let target = target_of(&first).expect("said");
    assert!(target.join("debug").is_dir(), "the target was used: {}", target.display());
    let (_, second, _, _) = run(&ci, "apps/site", &pipeline(&manifest));
    assert_eq!(target_of(&second), Some(target.clone()));
    assert!(!second.iter().any(|l| l.contains("Checking exemplo")), "nothing to compile again: {second:#?}");
    let (_, other, _, _) = run(&ci, "apps/outro", &pipeline(&manifest));
    assert_ne!(target_of(&other), Some(target));
}

#[test]
fn with_no_cargo_on_the_agent_the_step_says_so() {
    let data = folder("dados");
    let ci = ci(&data.0);
    ci.configure_plugin("cargo", json!({ "program": "cargo-que-nao-existe" })).expect("configured");
    let (status, _, _, reason) =
        run(&ci, "rust/sem", "[[stages]]\nname = \"R\"\nsteps = [{ cargo = \"check\" }]\n");
    assert_eq!(status, BuildStatus::Failed);
    assert!(reason.unwrap_or_default().contains("falta o cargo neste agente"));
}

#[test]
fn xcache_asked_and_missing_is_said_and_the_build_goes_on() {
    if !has_cargo() {
        eprintln!("pulado: sem cargo nesta máquina");
        return;
    }
    let (data, code) = (folder("dados"), folder("crate"));
    let ci = ci(&data.0);
    ci.configure_plugin("cargo", json!({ "xcache": true, "xcache_program": "xcache-que-nao-existe" }))
        .expect("configured");
    let manifest = crate_with(&code.0, FIXED);
    let (status, log, _, _) = run(&ci, "rust/cache", &pipeline(&manifest));
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    assert!(log.iter().any(|l| l.contains("compilando sem o cache")), "{log:#?}");
}

#[test]
fn a_pipeline_with_a_step_cargo_gets_its_publisher_once() {
    let pipeline = TomlFormat
        .read("x", "[[stages]]\nname = \"A\"\nsteps = [{ cargo = \"check\" }, { cargo = \"test\" }]\n")
        .expect("reads");
    let ids: Vec<&str> = pipeline.publishers.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["cargo"]);
    let none = TomlFormat.read("x", "[[stages]]\nname = \"A\"\nsteps = [\"make\"]\n").expect("reads");
    assert!(none.publishers.is_empty());
}
