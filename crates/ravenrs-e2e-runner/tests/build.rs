//! Builds real, small Cargo projects: compiler failures are distinct from channel failures,
//! and the executable comes from Cargo rather than a guessed target directory.
#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use std::path::PathBuf;
use std::time::Duration;

use ravenrs_e2e_runner::{app, build, report, run};
use ravenrs_e2e_wire::Scenario;

struct Project {
    root: PathBuf,
    package: String,
}

impl Project {
    fn new(name: &str, source: &str) -> Self {
        let package = format!("rts-build-{name}-{}", std::process::id());
        let root = std::env::temp_dir().join(&package);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join(".cargo")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname = {package:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n"),
        )
        .unwrap();
        std::fs::write(
            root.join(".cargo/config.toml"),
            "[build]\ntarget-dir = \"compiled app\"\n[net]\noffline = true\n",
        )
        .unwrap();
        std::fs::write(root.join("src/main.rs"), source).unwrap();
        app::learn(app::App {
            name: package.clone(),
            package: package.clone(),
            binary: package.clone(),
            source: Some(root.clone()),
            source_first: true,
            ..app::App::default()
        });
        Self { root, package }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_compiler_error_keeps_its_code_source_and_line_in_every_report() {
    let project = Project::new("error", "fn main() {\n    let _: u8 = \"not a number\";\n}\n");
    let scenario =
        Scenario::parse(&format!("scenario: Build falhou\nopen {}\nclick #save\n", project.package)).unwrap();
    let report = run::run("Build falhou", &[scenario], app::open);
    let failure = report.failure().unwrap();
    assert_eq!(failure.code.as_deref(), Some(build::APP_BUILD), "{}", report.text());
    let detail = failure.detail.as_deref().unwrap().replace('\\', "/");
    assert!(detail.contains("src/main.rs:2:"), "{detail}");
    assert!(detail.contains("mismatched types"), "{detail}");
    assert_eq!(report.not_run.len(), 1);
    assert_eq!(report.json()["blocks"][0]["code"], build::APP_BUILD);
    assert!(report::render(&[report], false, "junit").contains("<failure type=\"E2E_APP_BUILD\""));
}

const CHANNEL_APP: &str = r##"
use std::io::{self, BufRead};
fn main() {
    println!("{}", r#"{"protocol":"ravenrs-e2e-wire","version":1,"app":"Built shop"}"#);
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.contains("bye") { break; }
        println!("{}", r#"{"ok":true,"snapshot":"Built in the custom target"}"#);
    }
}
"##;

#[test]
fn a_source_app_uses_cargos_executable_in_a_custom_target_and_opens_the_channel() {
    let project = Project::new("channel", CHANNEL_APP);
    let executable =
        build::compile(&project.root, &project.package, &project.package, Duration::from_secs(60)).unwrap();
    let metadata = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--offline"])
        .current_dir(&project.root)
        .output()
        .unwrap();
    assert!(metadata.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let target = PathBuf::from(metadata["target_directory"].as_str().unwrap());
    assert!(executable.starts_with(&target), "{} versus {}", executable.display(), target.display());
    assert!(executable.is_file());
    let mut session = app::open(&project.package).unwrap().unwrap();
    assert_eq!(session.screen().as_deref(), Some("Built in the custom target"));
}

#[test]
fn a_compiled_app_without_a_channel_remains_an_app_start_failure() {
    let project = Project::new("no-channel", "fn main() {}\n");
    let scenario =
        Scenario::parse(&format!("scenario: Sem canal\nopen {}\nclick #save\n", project.package)).unwrap();
    let report = run::run("Sem canal", &[scenario], app::open);
    assert_eq!(report.failure().unwrap().code.as_deref(), Some("E2E_APP_START"), "{}", report.text());
    assert_eq!(report.not_run.len(), 1);
}

#[test]
fn a_build_past_its_deadline_is_ended_and_reported_as_a_build_failure() {
    let project = Project::new("deadline", "fn main() {}\n");
    std::fs::write(
        project.root.join("build.rs"),
        "fn main() { std::thread::sleep(std::time::Duration::from_secs(30)); }\n",
    )
    .unwrap();
    let failure =
        build::compile(&project.root, &project.package, &project.package, Duration::from_millis(100))
            .unwrap_err();
    assert_eq!(failure.code.as_deref(), Some(build::APP_BUILD));
    assert!(failure.detail.as_deref().unwrap().contains("tempo-limite"), "{failure}");
}
