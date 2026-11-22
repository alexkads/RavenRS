//! Preflight of embedded scenarios must preserve the app's build/start failure (#692).

use std::path::PathBuf;

use raven_test_studio::{apps, report::Report, run_named};

struct AppProject(PathBuf);

impl AppProject {
    fn new(label: &str, source: &str) -> Self {
        let package = format!("rts-embedded-{label}-{}", std::process::id());
        let root = std::env::temp_dir().join(&package);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join(".cargo")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname = {package:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n"),
        )
        .unwrap();
        std::fs::write(root.join(".cargo/config.toml"), "[net]\noffline = true\n").unwrap();
        std::fs::write(root.join("src/main.rs"), source).unwrap();
        apps::learn(apps::App {
            name: apps::FINANCE_APP.to_owned(),
            package: package.clone(),
            binary: package,
            source: Some(root.clone()),
            source_first: true,
            ..apps::App::default()
        });
        Self(root)
    }
}

impl Drop for AppProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_open_failure(report: &Report, code: &str) {
    assert!(!report.passed());
    let failure = report.failure().unwrap();
    assert_eq!(failure.code.as_deref(), Some(code), "{}", report.text());
    assert!(failure.meaning.is_some());
    assert!(failure.detail.is_some());
    assert!(failure.fix.is_some());
    let json = report.json();
    assert_eq!(json["blocks"][0]["code"], code);
    assert_eq!(json["blocks"][0]["detail"], failure.detail.as_deref().unwrap());
    assert!(Report::junit(std::slice::from_ref(report)).contains(&format!("<failure type=\"{code}\"")));
}

#[test]
fn an_embedded_scenarios_build_failure_keeps_its_code_and_rust_diagnostic() {
    let _project = AppProject::new("build-error", "fn main() {\n    let _: u8 = \"not a number\";\n}\n");
    let report = run_named("financas", None).unwrap();
    assert_open_failure(&report, "E2E_APP_BUILD");
    assert!(
        report.failure().unwrap().detail.as_deref().unwrap().replace('\\', "/").contains("src/main.rs:2:")
    );
}

#[test]
fn an_embedded_scenarios_start_failure_keeps_its_code() {
    // The test harness rejects --e2e-serve; this binary is deliberately not an app.
    apps::learn(apps::App {
        name: apps::FINANCE_APP.to_owned(),
        binary_path: Some(std::env::current_exe().unwrap()),
        ..apps::App::default()
    });
    assert_open_failure(&run_named("financas", None).unwrap(), "E2E_APP_START");
}

#[test]
fn an_invalid_embedded_scenario_is_not_classified_as_a_build_failure() {
    let hello = ravenrs_e2e_wire::Hello::of(apps::FINANCE_APP)
        .scenarios(["scenario: Broken\nopen Shop\njump #save\n"]);
    let json = serde_json::to_string(&hello).unwrap();
    let _project =
        AppProject::new("bad-scenario", &format!("fn main() {{ println!(\"{{}}\", {json:?}); }}\n"));
    let report = run_named("financas", None).unwrap();
    assert!(!report.passed());
    let failure = report.failure().unwrap();
    assert_eq!(failure.code, None, "the build and hello succeeded");
    assert!(failure.error.as_deref().unwrap().contains("linha 3"), "{}", report.text());
    assert!(!Report::junit(&[report]).contains("E2E_APP_BUILD"));
}
