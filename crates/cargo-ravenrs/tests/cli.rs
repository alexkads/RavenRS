//! Exercise generated projects and the actual commands registered for MCP clients.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests fail with context")]
use cargo_ravenrs::{SKILL, VERSION, configure_ai, project_files};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn write_project(root: &Path, template: &str, target: &str) {
    for file in project_files("sample-app", template, target).unwrap() {
        let path = root.join(file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, file.contents).unwrap();
    }
}

fn cli(root: &Path, args: &[&str]) -> Output {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let executable = bin.join(format!("cargo-ravenrs{}", std::env::consts::EXE_SUFFIX));
    if !executable.exists() {
        fs::copy(env!("CARGO_BIN_EXE_cargo-ravenrs"), &executable).unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    Command::new(executable).args(args).env("PATH", path).current_dir(root).output().unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn templates_embed_the_versioned_examples_and_published_dependencies() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (template, example) in [
        ("hello", "hello-world"),
        ("app", "tutorial-tasks"),
        ("app-sqlite", "tasks-sqlite"),
        ("relatorios", "report-designer"),
    ] {
        for target in ["desktop", "web", "both"] {
            let files = project_files("sample-app", template, target).unwrap();
            let manifest = &files.iter().find(|f| f.path == "Cargo.toml").unwrap().contents;
            assert!(!manifest.contains("workspace = true"));
            assert!(!manifest.contains("path ="));
            assert!(manifest.contains(&format!("={VERSION}")));
            assert_eq!(files.iter().any(|f| f.path == "web/index.html"), target != "desktop");
            let source = &files.iter().find(|f| f.path == "src/lib.rs").unwrap().contents;
            assert_eq!(
                source,
                &fs::read_to_string(workspace.join(format!("examples/{example}/src/lib.rs"))).unwrap()
            );
        }
    }
    for name in ["../bad", "0app", "fn", "match", "CON", "com1", "has space"] {
        assert!(project_files(name, "hello", "both").is_err());
    }
    assert!(project_files("valid", "unknown", "both").is_err());
    assert!(project_files("valid", "hello", "unknown").is_err());
}

#[test]
fn ai_is_idempotent_preserves_user_content_and_copies_the_skill() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_project(root, "app", "both");
    fs::write(root.join("AGENTS.md"), "House instructions\n").unwrap();
    fs::create_dir(root.join(".codex")).unwrap();
    fs::write(
        root.join(".codex/config.toml"),
        "# personal comment\nmodel = \"my-model\"\n[mcp_servers.other]\ncommand = \"other\"\n",
    )
    .unwrap();
    fs::write(root.join(".mcp.json"), r#"{"mcpServers":{"other":{"command":"other"}},"custom":true}"#)
        .unwrap();
    configure_ai(root, false).unwrap();
    assert_eq!(fs::read_to_string(root.join(".claude/skills/ravenrs/SKILL.md")).unwrap(), SKILL);
    let paths = [
        "AGENTS.md",
        "CLAUDE.md",
        ".codex/config.toml",
        ".mcp.json",
        ".vscode/mcp.json",
        ".cursor/mcp.json",
        ".claude/settings.json",
        ".claude/skills/ravenrs/SKILL.md",
    ];
    let before: Vec<_> = paths.iter().map(|p| fs::read(root.join(p)).unwrap()).collect();
    configure_ai(root, false).unwrap();
    for (path, before) in paths.iter().zip(before) {
        assert_eq!(fs::read(root.join(path)).unwrap(), before, "{path}");
    }
    let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("House instructions\n"));
    let codex = fs::read_to_string(root.join(".codex/config.toml")).unwrap();
    assert!(codex.contains("# personal comment"));
    assert!(codex.contains("mcp_servers.other"));
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(json["custom"], true);
    assert_eq!(json["mcpServers"]["other"]["command"], "other");
    let skill_path = root.join(".claude/skills/ravenrs/SKILL.md");
    fs::write(&skill_path, format!("{SKILL}\nPersonal skill notes\n")).unwrap();
    configure_ai(root, false).unwrap();
    assert!(fs::read_to_string(skill_path).unwrap().ends_with("Personal skill notes\n"));
}

#[test]
fn ai_rejects_malformed_files_without_partially_overwriting_configuration() {
    let dir = TempDir::new().unwrap();
    write_project(dir.path(), "hello", "desktop");
    fs::write(dir.path().join("AGENTS.md"), "<!-- ravenrs:begin --> broken").unwrap();
    assert!(configure_ai(dir.path(), false).is_err());
    assert!(!dir.path().join(".mcp.json").exists());
}

#[test]
fn doctor_probes_every_registered_command_and_detects_stale_versions_and_files() {
    let dir = TempDir::new().unwrap();
    write_project(dir.path(), "hello", "both");
    configure_ai(dir.path(), false).unwrap();
    success(&cli(dir.path(), &["ravenrs", "doctor"]));
    fs::write(dir.path().join(".claude/skills/ravenrs/SKILL.md"), "stale skill").unwrap();
    let output = cli(dir.path(), &["doctor"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("skill"));
    let manifest = dir.path().join("Cargo.toml");
    fs::write(&manifest, fs::read_to_string(&manifest).unwrap().replace(VERSION, "0.0.1")).unwrap();
    let output = cli(dir.path(), &["doctor"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("0.0.1"));
}

#[test]
fn new_is_repeatable_and_never_overwrites_user_code() {
    let dir = TempDir::new().unwrap();
    let args = ["new", "sample-app", "--yes", "--no-check", "--no-git"];
    success(&cli(dir.path(), &args));
    let path = dir.path().join("sample-app/src/lib.rs");
    fs::write(&path, "// user code\n").unwrap();
    success(&cli(dir.path(), &args));
    assert_eq!(fs::read_to_string(path).unwrap(), "// user code\n");
    fs::create_dir(dir.path().join("occupied")).unwrap();
    assert!(!cli(dir.path(), &["new", "occupied", "--yes", "--no-check", "--no-git"]).status.success());
}

#[test]
fn marketplace_mode_is_versioned_and_does_not_copy_the_skill() {
    let dir = TempDir::new().unwrap();
    write_project(dir.path(), "app", "both");
    configure_ai(dir.path(), true).unwrap();
    let settings = fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap();
    assert!(settings.contains(&format!("v{VERSION}")));
    assert!(!dir.path().join(".claude/skills/ravenrs/SKILL.md").exists());
    success(&cli(dir.path(), &["doctor"]));
}

#[test]
fn no_ai_projects_still_pass_doctor_and_ai_can_be_added_later() {
    let dir = TempDir::new().unwrap();
    success(&cli(dir.path(), &["new", "sample-app", "--yes", "--no-check", "--no-git", "--no-ai"]));
    let project = dir.path().join("sample-app");
    assert!(!project.join("AGENTS.md").exists());
    success(&cli(&project, &["doctor"]));
    configure_ai(&project, false).unwrap();
    success(&cli(&project, &["doctor"]));
}

#[test]
fn plugin_switch_removes_only_the_unmodified_managed_skill() {
    let dir = TempDir::new().unwrap();
    write_project(dir.path(), "app", "desktop");
    configure_ai(dir.path(), false).unwrap();
    configure_ai(dir.path(), true).unwrap();
    assert!(!dir.path().join(".claude/skills/ravenrs/SKILL.md").exists());
    configure_ai(dir.path(), false).unwrap();
    let path = dir.path().join(".claude/skills/ravenrs/SKILL.md");
    fs::write(&path, format!("{SKILL}\nMy notes\n")).unwrap();
    assert!(configure_ai(dir.path(), true).is_err());
    assert!(fs::read_to_string(path).unwrap().ends_with("My notes\n"));
}

/// This is deliberately an explicit check.sh step: it runs Cargo 20 times.
#[test]
#[ignore = "compiles every template × target; run via scripts/check.sh or --ignored"]
fn every_template_and_target_compiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let target_dir = workspace.join("target/generated-projects");
    for template in ["hello", "app", "app-sqlite", "relatorios"] {
        for target in ["desktop", "web", "both"] {
            let dir = TempDir::new().unwrap();
            write_project(dir.path(), template, target);
            // Release validation can instead use actual crates.io packages with this env var.
            if std::env::var_os("RAVENRS_TEST_PUBLISHED").is_none() {
                patch_checkout(dir.path(), &workspace);
            }
            for triple in if target == "both" {
                vec![None, Some("wasm32-unknown-unknown")]
            } else if target == "web" {
                vec![Some("wasm32-unknown-unknown")]
            } else {
                vec![None]
            } {
                let mut command = Command::new("cargo");
                command
                    .args(["check", "--tests", "--quiet"])
                    .current_dir(dir.path())
                    .env("CARGO_TARGET_DIR", &target_dir);
                if let Some(triple) = triple {
                    command.args(["--target", triple]);
                }
                eprintln!("Checking {template}/{target}/{triple:?}");
                success(&command.output().unwrap());
            }
        }
    }
}

fn patch_checkout(root: &Path, workspace: &Path) {
    let manifest_path = root.join("Cargo.toml");
    let mut manifest = fs::read_to_string(&manifest_path).unwrap();
    manifest.push_str("\n[patch.crates-io]\n");
    let dirs: Vec<PathBuf> =
        fs::read_dir(workspace.join("crates")).unwrap().map(|e| e.unwrap().path()).collect();
    for dir in dirs {
        let cargo = dir.join("Cargo.toml");
        if !cargo.is_file() {
            continue;
        }
        let parsed: toml_edit::DocumentMut = fs::read_to_string(cargo).unwrap().parse().unwrap();
        let name = parsed["package"]["name"].as_str().unwrap();
        if name.starts_with("ravenrs") {
            manifest.push_str(&format!("{name} = {{ path = {:?} }}\n", dir.to_string_lossy()));
        }
    }
    fs::write(manifest_path, manifest).unwrap();
}
