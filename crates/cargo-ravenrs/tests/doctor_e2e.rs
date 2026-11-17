//! Doctor reads e2e configuration without building or launching the project.
#![allow(clippy::unwrap_used, reason = "tests fail with context")]

use cargo_ravenrs::VERSION;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn write(root: &Path, name: &str, contents: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn project(channel: bool, library: bool) -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for package in ["ravenrs", "ravenrs-e2e-wire"] {
        write(
            root,
            &format!("deps/{package}/Cargo.toml"),
            &format!("[package]\nname = {package:?}\nversion = {VERSION:?}\nedition = \"2024\"\n"),
        );
        write(root, &format!("deps/{package}/src/lib.rs"), "");
    }
    let wire = if channel {
        format!(
            "ravenrs-e2e-wire = {{ path = \"deps/ravenrs-e2e-wire\", version = \"={VERSION}\" }}\n\n[package.metadata.ravenrs.e2e]\napp = \"Shop\"\nbinary = \"shop\"\n"
        )
    } else {
        String::new()
    };
    write(
        root,
        "Cargo.toml",
        &format!(
            "[package]\nname = \"shop\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nravenrs = {{ path = \"deps/ravenrs\", version = \"={VERSION}\" }}\n{wire}"
        ),
    );
    write(
        root,
        "src/main.rs",
        "fn main() { panic!(\"doctor must not launch this app\"); }\n// SERVE_FLAG: deliberately only a static hint, not a working channel.\n",
    );
    if library {
        fs::rename(root.join("src/main.rs"), root.join("src/lib.rs")).unwrap();
    }
    write(
        root,
        "build.rs",
        "fn main() { std::fs::write(\"BUILD_WAS_RUN\", \"unexpected\").unwrap(); panic!(\"doctor must not build\"); }\n",
    );
    write(root, ".ravenrs-project.toml", "ai = false\n");
    dir
}

fn doctor(root: &Path) -> Output {
    let executable = Path::new(env!("CARGO_BIN_EXE_cargo-ravenrs"));
    let path = std::env::join_paths(
        std::iter::once(executable.parent().unwrap().to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let out = Command::new(executable).arg("doctor").current_dir(root).env("PATH", path).output().unwrap();
    assert!(!root.join("BUILD_WAS_RUN").exists(), "doctor ran the build script");
    out
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn doctor_warns_about_missing_e2e_even_without_ai_and_does_not_build() {
    let dir = project(false, false);
    let out = doctor(dir.path());
    assert!(out.status.success(), "{out:?}");
    let text = stdout(&out);
    assert!(text.contains("falta [package.metadata.ravenrs.e2e]"), "{text}");
    assert!(text.contains("canal não identificado"), "{text}");
    assert!(text.contains("sem cenários"), "{text}");
    assert!(text.contains("--no-ai"), "{text}");
    assert!(!dir.path().join(".ravenrs-ai.toml").exists());
}

#[test]
fn doctor_does_not_mistake_static_channel_hints_for_a_successful_handshake() {
    let dir = project(true, false);
    write(dir.path(), "tests/e2e/start.raventest", "scenario: Start\nopen Shop\n");
    let out = doctor(dir.path());
    assert!(out.status.success(), "{out:?}");
    let text = stdout(&out);
    assert!(text.contains("indícios do canal"), "{text}");
    assert!(text.contains("resposta em execução não verificada"), "{text}");
    assert!(!text.contains("falta [package.metadata") && !text.contains("sem cenários"), "{text}");
}

#[test]
fn doctor_refuses_invalid_e2e_config_and_scenarios_before_mcp() {
    let dir = project(true, false);
    for (file, contents, expected) in [
        ("e2e.toml", "film = \"unknown\"\n", "film"),
        ("tests/e2e/broken.raventest", "scenario: Broken\nopen Shop\njump #save\n", "linha 3"),
    ] {
        write(dir.path(), file, contents);
        let out = doctor(dir.path());
        assert!(!out.status.success(), "{out:?}");
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(text.contains(expected) && text.contains(file), "{text}");
        fs::remove_file(dir.path().join(file)).unwrap();
    }
    let manifest = dir.path().join("Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    for (suffix, expected) in [("features = [false]\n", "features"), ("binary = \"missing\"\n", "missing")] {
        write(dir.path(), "Cargo.toml", &format!("{}{suffix}", text.replace("binary = \"shop\"\n", "")));
        let out = doctor(dir.path());
        assert!(!out.status.success(), "{out:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(expected), "{out:?}");
    }
}

#[test]
fn doctor_does_not_require_a_channel_in_a_library() {
    let dir = project(false, true);
    let out = doctor(dir.path());
    assert!(out.status.success(), "{out:?}");
    assert!(!stdout(&out).contains("! E2E"), "{out:?}");
}
