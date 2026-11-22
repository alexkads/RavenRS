//! Upgrade actual generated project files without building, running or overwriting custom code.
#![allow(clippy::unwrap_used, reason = "tests fail with context")]

use cargo_ravenrs::{VERSION, project_files};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;
use tempfile::TempDir;
use toml_edit::DocumentMut;

const LEGACY: &str = include_str!("fixtures/pre-e2e-main.rs");

fn write(root: &Path, file: &str, text: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn project(kind: &str, legacy: bool) -> TempDir {
    let dir = TempDir::new().unwrap();
    for file in project_files("sample-app", kind, "desktop").unwrap() {
        write(dir.path(), &file.path, &file.contents);
    }
    write(
        dir.path(),
        ".ravenrs-project.toml",
        &format!("version = {VERSION:?}\ntemplate = {kind:?}\ntarget = \"desktop\"\nai = false\n"),
    );
    if legacy {
        write(dir.path(), "src/main.rs", LEGACY);
        let path = dir.path().join("Cargo.toml");
        let mut doc: DocumentMut = fs::read_to_string(&path).unwrap().parse().unwrap();
        doc["package"].as_table_like_mut().unwrap().remove("metadata");
        doc.as_table_mut().remove("features");
        let deps =
            doc["target"]["cfg(not(target_arch = \"wasm32\"))"]["dependencies"].as_table_like_mut().unwrap();
        deps.remove("ravenrs-e2e");
        deps.remove("ravenrs-e2e-wire");
        fs::write(path, doc.to_string()).unwrap();
        fs::remove_file(dir.path().join("tests/e2e/start.raventest")).unwrap();
        if kind == "app-sqlite" {
            let path = dir.path().join("tests/screen.rs");
            fs::write(&path, fs::read_to_string(&path).unwrap().replace("::tasks().into_view()", "::app()"))
                .unwrap();
        }
    }
    dir
}

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-ravenrs")).args(args).current_dir(root).output().unwrap()
}

fn success(out: &Output) {
    assert!(out.status.success(), "{out:?}");
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    (fs::read(&path).unwrap(), fs::metadata(&path).unwrap().modified().unwrap()),
                );
            }
        }
    }
    files
}

#[test]
fn install_upgrades_each_old_template_preserves_custom_files_and_is_idempotent() {
    for kind in ["hello", "app", "app-sqlite", "relatorios"] {
        let dir = project(kind, true);
        let root = dir.path();
        let manifest = root.join("Cargo.toml");
        fs::write(&manifest, format!("# My dependency notes\n{}\n[package.metadata.user]\nkeep = 'literal text' # keep this comment\n", fs::read_to_string(&manifest).unwrap())).unwrap();
        let lib = root.join("src/lib.rs");
        fs::write(&lib, format!("{}\n// Custom business notes\n", fs::read_to_string(&lib).unwrap()))
            .unwrap();
        let before_lib = fs::read(&lib).unwrap();
        write(
            root,
            "build.rs",
            "fn main() { std::fs::write(\"BUILD_WAS_RUN\", \"bad\").unwrap(); panic!(\"install must not build\"); }\n",
        );
        let scenario = "// My scenario stays intact\nscenario: Custom\nopen sample-app\n";
        write(root, "tests/e2e/start.raventest", scenario);
        let out = cli(root, &["e2e", "install"]);
        success(&out);
        assert!(!root.join("BUILD_WAS_RUN").exists());
        assert!(!root.join("target").exists(), "installation did not compile the project");
        assert!(!root.join("tasks.sqlite").exists());
        assert_eq!(fs::read(&lib).unwrap(), before_lib);
        assert_eq!(fs::read_to_string(root.join("tests/e2e/start.raventest")).unwrap(), scenario);
        let text = fs::read_to_string(&manifest).unwrap();
        assert!(
            text.contains("# My dependency notes")
                && text.contains("keep = 'literal text' # keep this comment"),
            "{text}"
        );
        let app = ravenrs_e2e_runner::app::App::declared_in(&text).unwrap();
        assert_eq!(app.name, "sample-app");
        assert_eq!(app.features, ["e2e-source"]);
        assert!(fs::read_to_string(root.join("src/main.rs")).unwrap().contains("serve_filmed"));
        if kind == "app-sqlite" {
            assert!(
                fs::read_to_string(root.join("tests/screen.rs")).unwrap().contains("tasks().into_view()")
            );
        }
        let before = snapshot(root);
        let out = cli(root, &["e2e", "install"]);
        success(&out);
        assert!(String::from_utf8_lossy(&out.stdout).contains("nenhum arquivo alterado"), "{out:?}");
        assert_eq!(snapshot(root), before, "idempotence includes modification times: {kind}");
    }
}

#[test]
fn current_templates_need_no_installation_and_custom_tests_are_preserved() {
    let dir = project("hello", false);
    let before = snapshot(dir.path());
    success(&cli(dir.path(), &["e2e", "install"]));
    assert_eq!(snapshot(dir.path()), before);
    let dir = project("app-sqlite", true);
    write(dir.path(), "tests/screen.rs", "// A custom integration test\n");
    success(&cli(dir.path(), &["e2e", "install"]));
    assert_eq!(
        fs::read_to_string(dir.path().join("tests/screen.rs")).unwrap(),
        "// A custom integration test\n"
    );
}

#[test]
fn formatting_an_entry_does_not_prevent_installation_or_cause_rewrites() {
    let dir = project("hello", true);
    write(dir.path(), "rustfmt.toml", "max_width = 60\n");
    let format_main =
        || success(&Command::new("rustfmt").arg("src/main.rs").current_dir(dir.path()).output().unwrap());
    format_main();
    assert_ne!(fs::read_to_string(dir.path().join("src/main.rs")).unwrap(), LEGACY);
    success(&cli(dir.path(), &["e2e", "install"]));
    format_main();
    let before = snapshot(dir.path());
    success(&cli(dir.path(), &["e2e", "install"]));
    assert_eq!(snapshot(dir.path()), before, "a formatted installed entry is preserved byte for byte");
    let main = dir.path().join("src/main.rs");
    fs::write(&main, fs::read_to_string(&main).unwrap().replace('\n', "\r\n")).unwrap();
    let before = snapshot(dir.path());
    success(&cli(dir.path(), &["e2e", "install"]));
    assert_eq!(snapshot(dir.path()), before, "CRLF is preserved too");
}

#[test]
fn partial_configuration_keeps_custom_features_and_enables_serve_without_defaults() {
    let dir = project("hello", true);
    let path = dir.path().join("Cargo.toml");
    let text = fs::read_to_string(&path).unwrap().replace("[dependencies]\n", &format!("[dependencies]\nravenrs-e2e-wire = {{ version = \"={VERSION}\", default-features = false, features = [\"e2e-source\"] }} # wire options\n"));
    fs::write(&path, format!("{text}\n[features]\ncustom = []\ne2e-source = [\"custom\"]\n[package.metadata.ravenrs.e2e]\nfeatures = [\"custom\"]\n")).unwrap();
    success(&cli(dir.path(), &["e2e", "install"]));
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# wire options"));
    let doc: DocumentMut = text.parse().unwrap();
    let wire = &doc["dependencies"]["ravenrs-e2e-wire"];
    assert_eq!(wire["default-features"].as_bool(), Some(false));
    let features: Vec<_> =
        wire["features"].as_array().unwrap().iter().map(|value| value.as_str().unwrap()).collect();
    assert_eq!(features, ["e2e-source", "serve"]);
    let app = ravenrs_e2e_runner::app::App::declared_in(&text).unwrap();
    assert_eq!(app.features, ["custom", "e2e-source"]);
    let features: Vec<_> = doc["features"]["e2e-source"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(features, ["custom", "ravenrs/e2e-source"]);
}

#[test]
fn an_invalid_or_unknown_entry_is_refused_before_any_project_file_changes() {
    for case in [
        "main",
        "main-comment",
        "version",
        "config",
        "scenario",
        "features",
        "binary",
        "metadata",
        "state",
        "library",
        "workspace",
        "outside-folder",
        "metadata-features",
    ] {
        let dir = project("hello", true);
        let root = dir.path();
        match case {
            "main" => write(root, "src/main.rs", "fn main() { panic!(\"user entry\"); }\n"),
            "main-comment" => {
                write(root, "src/main.rs", &format!("{LEGACY}\n// Keep my custom startup notes\n"))
            }
            "version" => {
                let path = root.join("Cargo.toml");
                fs::write(&path, fs::read_to_string(&path).unwrap().replace(VERSION, "0.0.1")).unwrap();
            }
            "config" => write(root, "e2e.toml", "film = \"bad\"\n"),
            "scenario" => {
                write(root, "tests/e2e/start.raventest", "scenario: Broken\nopen sample-app\njump #bad\n")
            }
            "features" => {
                let path = root.join("Cargo.toml");
                fs::write(
                    &path,
                    format!("{}\n[features]\ne2e-source = [false]\n", fs::read_to_string(&path).unwrap()),
                )
                .unwrap();
            }
            "binary" => {
                let path = root.join("Cargo.toml");
                fs::write(
                    &path,
                    format!(
                        "{}\n[[bin]]\nname = \"custom\"\npath = \"src/main.rs\"\n",
                        fs::read_to_string(&path).unwrap()
                    ),
                )
                .unwrap();
            }
            "metadata" => {
                let path = root.join("Cargo.toml");
                fs::write(
                    &path,
                    format!(
                        "{}\n[package.metadata.ravenrs.e2e]\napp = \"Another app\"\n",
                        fs::read_to_string(&path).unwrap()
                    ),
                )
                .unwrap();
            }
            "state" => fs::remove_file(root.join(".ravenrs-project.toml")).unwrap(),
            "library" => fs::remove_file(root.join("src/main.rs")).unwrap(),
            "workspace" => write(root, "Cargo.toml", "[workspace]\nmembers = []\n"),
            "outside-folder" => write(root, "e2e.toml", "folder = \"../external\"\n"),
            "metadata-features" => {
                let path = root.join("Cargo.toml");
                fs::write(
                    &path,
                    format!(
                        "{}\n[package.metadata.ravenrs.e2e]\nfeatures = [false]\n",
                        fs::read_to_string(&path).unwrap()
                    ),
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let before = snapshot(root);
        let out = cli(root, &["e2e", "install"]);
        assert_eq!(out.status.code(), Some(2), "{case}: {out:?}");
        assert_eq!(snapshot(root), before, "{case} modified a project file on refusal");
    }
}

#[test]
#[cfg(unix)]
fn installation_refuses_a_linked_entry_without_touching_its_target() {
    let dir = project("hello", true);
    let other = TempDir::new().unwrap();
    write(other.path(), "main.rs", LEGACY);
    fs::remove_file(dir.path().join("src/main.rs")).unwrap();
    std::os::unix::fs::symlink(other.path().join("main.rs"), dir.path().join("src/main.rs")).unwrap();
    let before = snapshot(dir.path());
    let target = snapshot(other.path());
    let out = cli(dir.path(), &["e2e", "install"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("link"));
    assert_eq!(snapshot(dir.path()), before);
    assert_eq!(snapshot(other.path()), target);
}

/// Reuses the checkout rather than assuming its partially published version is available.
fn patch_checkout(root: &Path, workspace: &Path) {
    let path = root.join("Cargo.toml");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\n[patch.crates-io]\n");
    for entry in fs::read_dir(workspace.join("crates")).unwrap() {
        let folder = entry.unwrap().path();
        if !folder.join("Cargo.toml").is_file() {
            continue;
        }
        let doc: DocumentMut = fs::read_to_string(folder.join("Cargo.toml")).unwrap().parse().unwrap();
        let name = doc["package"]["name"].as_str().unwrap();
        if name.starts_with("ravenrs") {
            text.push_str(&format!("{name} = {{ path = {:?} }}\n", folder.to_string_lossy()));
        }
    }
    fs::write(path, text).unwrap();
}

#[test]
#[ignore = "builds and drives four migrated legacy apps in release; explicit installation validation"]
fn installed_legacy_apps_run_their_scenarios_and_frames_without_opening_a_database() {
    use ravenrs_e2e_runner::{
        run,
        session::{Failure, Session},
    };
    use ravenrs_e2e_wire::{Client, Command as WireCommand, SERVE_FLAG};
    use std::time::Duration;
    thread_local! { static OPENED: std::cell::RefCell<Option<Session>> = const { std::cell::RefCell::new(None) }; }
    fn open(name: &str) -> Option<Result<Session, Box<Failure>>> {
        if name != "sample-app" {
            return None;
        }
        OPENED.with(|session| session.borrow_mut().take().map(Ok))
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let target = workspace.join("target/generated-projects");
    for kind in ["hello", "app", "app-sqlite", "relatorios"] {
        let dir = project(kind, true);
        let root = dir.path();
        patch_checkout(root, &workspace);
        success(&cli(root, &["e2e", "install"]));
        assert!(!root.join("tasks.sqlite").exists());
        let sentinel = b"existing database must remain untouched";
        fs::write(root.join("tasks.sqlite"), sentinel).unwrap();
        eprintln!("Building and driving installed legacy {kind}");
        success(
            &Command::new("cargo")
                .args(["build", "--release", "--offline", "--features", "e2e-source"])
                .current_dir(root)
                .env("CARGO_TARGET_DIR", &target)
                .output()
                .unwrap(),
        );
        let mut process =
            Command::new(target.join("release").join(format!("sample-app{}", std::env::consts::EXE_SUFFIX)));
        process.arg(SERVE_FLAG).current_dir(root);
        let mut client = Client::spawn(process, Duration::from_secs(20)).unwrap();
        assert_eq!(client.app(), "sample-app");
        assert!(!client.send(&WireCommand::Snapshot).unwrap().unwrap().is_empty());
        assert!(!client.frame().unwrap().is_empty());
        OPENED.with(|session| *session.borrow_mut() = Some(Session::Apart(client)));
        let report = run::run_files(&root.join("tests/e2e/start.raventest"), open).unwrap();
        assert!(report.passed(), "{kind}: {}", report.text());
        assert_eq!(fs::read(root.join("tasks.sqlite")).unwrap(), sentinel);
        for suffix in ["-journal", "-wal", "-shm"] {
            assert!(!root.join(format!("tasks.sqlite{suffix}")).exists());
        }
    }
}

#[test]
fn installation_respects_custom_scenario_folders_and_dependency_comments() {
    let dir = project("hello", true);
    let root = dir.path();
    let path = root.join("Cargo.toml");
    let mut doc: DocumentMut = fs::read_to_string(&path).unwrap().parse().unwrap();
    doc["dependencies"]["ravenrs-e2e-wire"] = toml_edit::value(format!("={VERSION}"));
    let text = doc.to_string().replace(
        &format!("ravenrs-e2e-wire = \"={VERSION}\""),
        &format!("ravenrs-e2e-wire = \"={VERSION}\" # protocol note"),
    );
    fs::write(&path, text).unwrap();
    write(root, "e2e.toml", "folder = \"qa/fluxos\"\n");
    let scenario = "scenario: Custom folder\nopen sample-app\n";
    write(root, "qa/fluxos/start.raventest", scenario);
    success(&cli(root, &["e2e", "install", "--project", root.to_str().unwrap()]));
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# protocol note"), "{text}");
    let doc: DocumentMut = text.parse().unwrap();
    assert!(
        doc["dependencies"]["ravenrs-e2e-wire"]["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature.as_str() == Some("serve"))
    );
    assert!(
        doc["target"]["cfg(not(target_arch = \"wasm32\"))"]["dependencies"].get("ravenrs-e2e-wire").is_none()
    );
    assert!(fs::read_to_string(root.join("src/main.rs")).unwrap().contains("../qa/fluxos/start.raventest"));
    assert_eq!(fs::read_to_string(root.join("qa/fluxos/start.raventest")).unwrap(), scenario);
    assert!(!root.join("tests/e2e/start.raventest").exists());
    let before = snapshot(root);
    success(&cli(root, &["e2e", "install"]));
    assert_eq!(snapshot(root), before);
}
