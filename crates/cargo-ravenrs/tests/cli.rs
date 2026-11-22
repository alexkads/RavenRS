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
        ("relatorios", "raven-report-designer"),
    ] {
        for target in ["desktop", "web", "both"] {
            let files = project_files("sample-app", template, target).unwrap();
            let manifest = &files.iter().find(|f| f.path == "Cargo.toml").unwrap().contents;
            assert!(!manifest.contains("workspace = true"));
            let parsed: toml_edit::DocumentMut = manifest.parse().unwrap();
            assert_eq!(
                parsed["package"]["rust-version"].as_str(),
                Some(if template == "app-sqlite" { "1.89" } else { "1.88" })
            );
            assert!(!manifest.contains("path ="));
            assert!(manifest.contains(&format!("={VERSION}")));
            let app = ravenrs_e2e_runner::app::App::declared_in(manifest).unwrap();
            assert_eq!(app.name, "sample-app");
            assert_eq!(app.binary, "sample-app");
            assert_eq!(app.features, ["e2e-source"]);
            let scenario =
                &files.iter().find(|file| file.path == "tests/e2e/start.raventest").unwrap().contents;
            let scenario = ravenrs_e2e_wire::Scenario::parse(scenario).unwrap();
            assert_eq!(scenario.blocks[0].kind, ravenrs_e2e_wire::Kind::Open);
            assert_eq!(scenario.blocks[0].target, app.name);
            assert!(scenario.blocks.len() > 1, "the initial scenario checks the screen");
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
fn cargo_names_keep_case_underscores_and_repeated_separators_in_all_entry_points() {
    for name in ["MyApp", "Main", "Native", "my--app", "my__app", "my-app-"] {
        let files = project_files(name, "hello", "both").unwrap();
        let krate = name.replace('-', "_");
        assert!(files.iter().find(|f| f.path == "src/main.rs").unwrap().contents.contains("fn main()"));
        assert!(
            files.iter().find(|f| f.path == "src/main.rs").unwrap().contents.contains("ravenrs::native::run")
        );
        assert!(
            files
                .iter()
                .find(|f| f.path == "src/main.rs")
                .unwrap()
                .contents
                .contains(&format!("{krate}::app()"))
        );
        assert!(
            files
                .iter()
                .find(|f| f.path == "web/index.html")
                .unwrap()
                .contents
                .contains(&format!("pkg/{krate}.js"))
        );
        assert!(
            files
                .iter()
                .find(|f| f.path == "tests/screen.rs")
                .unwrap()
                .contents
                .contains(&format!("{krate}::app()"))
        );
    }
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
    let plugin: serde_json::Value =
        serde_json::from_str(include_str!("../../../plugins/ravenrs/.mcp.json")).unwrap();
    let generated: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(plugin["mcpServers"]["ravenrs"], generated["mcpServers"]["ravenrs"]);
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

/// Actual generated apps, outside the workspace: no window/GPU and no persistent database.
#[test]
#[ignore = "builds and runs all four native templates in release; explicit F6 validation"]
fn generated_apps_serve_scenarios_and_frames_without_persistent_storage() {
    use ravenrs_e2e_runner::{
        run,
        session::{Failure, Session},
    };
    use ravenrs_e2e_wire::{Client, Command as WireCommand, SERVE_FLAG};
    use std::time::Duration;

    thread_local! {
        static OPENED: std::cell::RefCell<Option<Session>> = const { std::cell::RefCell::new(None) };
    }
    fn open(name: &str) -> Option<Result<Session, Box<Failure>>> {
        if name != "sample-app" {
            return None;
        }
        OPENED.with(|session| session.borrow_mut().take().map(Ok))
    }

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let target = workspace.join("target/generated-projects");
    for template in ["hello", "app", "app-sqlite", "relatorios"] {
        let dir = TempDir::new().unwrap();
        write_project(dir.path(), template, "desktop");
        patch_checkout(dir.path(), &workspace);
        eprintln!("Building and driving {template}");
        success(
            &Command::new("cargo")
                .args(["build", "--release", "--offline", "--features", "e2e-source"])
                .current_dir(dir.path())
                .env("CARGO_TARGET_DIR", &target)
                .output()
                .unwrap(),
        );
        let binary = target.join("release").join(format!("sample-app{}", std::env::consts::EXE_SUFFIX));
        let mut process = Command::new(&binary);
        process.arg(SERVE_FLAG).current_dir(dir.path());
        let mut client = Client::spawn(process, Duration::from_secs(20)).unwrap();
        assert_eq!(client.app(), "sample-app");
        let scenario = fs::read_to_string(dir.path().join("tests/e2e/start.raventest")).unwrap();
        assert_eq!(client.scenarios(), &[scenario]);
        assert!(!client.send(&WireCommand::Snapshot).unwrap().unwrap().is_empty(), "{template}");
        assert!(!client.frame().unwrap().is_empty(), "{template} must provide a vector frame");
        client.send(&WireCommand::Bye).unwrap();
        drop(client);
        assert!(!dir.path().join("tasks.sqlite").exists(), "{template} opened persistent storage");
        // Even an existing, invalid database must stay untouched: the test branch uses memory.
        let sentinel = b"database owned by the person using the app";
        fs::write(dir.path().join("tasks.sqlite"), sentinel).unwrap();
        let mut process = Command::new(binary);
        process.arg(SERVE_FLAG).current_dir(dir.path());
        let client = Client::spawn(process, Duration::from_secs(20)).unwrap();
        OPENED.with(|session| *session.borrow_mut() = Some(Session::Apart(client)));
        let report = run::run_files(&dir.path().join("tests/e2e/start.raventest"), open).unwrap();
        assert!(report.passed(), "{template}: {}", report.text());
        if template == "app-sqlite" {
            success(
                &Command::new("cargo")
                    .args(["test", "--release", "--offline", "--features", "e2e-source", "--test", "screen"])
                    .current_dir(dir.path())
                    .env("CARGO_TARGET_DIR", &target)
                    .output()
                    .unwrap(),
            );
        }
        assert_eq!(fs::read(dir.path().join("tasks.sqlite")).unwrap(), sentinel);
        assert!(!dir.path().join("tasks.sqlite-wal").exists());
        assert!(!dir.path().join("tasks.sqlite-shm").exists());
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

#[test]
fn repeating_with_another_template_or_target_refuses_before_touching_existing_files() {
    let dir = TempDir::new().unwrap();
    success(&cli(dir.path(), &["new", "sample-app", "--yes", "--no-check", "--no-git"]));
    let project = dir.path().join("sample-app");
    fs::write(project.join("src/lib.rs"), "// user work must stay intact\n").unwrap();
    fs::write(project.join("AGENTS.md"), "Personal instructions\n").unwrap();
    let paths = ["Cargo.toml", "src/lib.rs", "AGENTS.md", ".ravenrs-project.toml", ".mcp.json"];
    let before: Vec<_> = paths.iter().map(|p| fs::read(project.join(p)).unwrap()).collect();
    for (template, target) in [("hello", "both"), ("app-sqlite", "desktop"), ("app-sqlite", "both")] {
        let output = cli(
            dir.path(),
            &["new", "sample-app", "--template", template, "--target", target, "--yes", "--no-check"],
        );
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("hello/desktop"), "{error}");
        assert!(error.contains("Use outra pasta"), "{error}");
        for (path, original) in paths.iter().zip(&before) {
            assert_eq!(fs::read(project.join(path)).unwrap(), *original, "{path}");
        }
        assert!(!project.join(".git").exists());
    }
}

#[test]
#[ignore = "runs every generated native screen against real dependencies"]
fn every_template_main_screen_renders() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let target_dir = workspace.join("target/generated-projects");
    for template in ["hello", "app", "app-sqlite", "relatorios"] {
        let dir = TempDir::new().unwrap();
        write_project(dir.path(), template, "both");
        if std::env::var_os("RAVENRS_TEST_PUBLISHED").is_none() {
            patch_checkout(dir.path(), &workspace);
        }
        eprintln!("Rendering {template}/both/native");
        success(
            &Command::new("cargo")
                .args(["test", "--test", "screen", "--quiet"])
                .current_dir(dir.path())
                .env("CARGO_TARGET_DIR", &target_dir)
                .output()
                .unwrap(),
        );
    }
}

#[test]
fn repeat_without_flags_keeps_the_original_nondefault_project_settings() {
    let dir = TempDir::new().unwrap();
    success(&cli(
        dir.path(),
        &[
            "new",
            "sample-app",
            "--template",
            "app-sqlite",
            "--target",
            "both",
            "--yes",
            "--no-check",
            "--no-git",
        ],
    ));
    let project = dir.path().join("sample-app");
    let source = project.join("src/lib.rs");
    fs::write(&source, "// user's application\n").unwrap();
    let manifest = fs::read(project.join("Cargo.toml")).unwrap();
    success(&cli(dir.path(), &["new", "sample-app", "--yes", "--no-check", "--no-git"]));
    assert_eq!(fs::read_to_string(&source).unwrap(), "// user's application\n");
    assert_eq!(fs::read(project.join("Cargo.toml")).unwrap(), manifest);
}

#[test]
fn doctor_reads_inherited_renamed_and_target_versions_without_touching_manifests() {
    use cargo_ravenrs::ravenrs_versions;
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    // Virtual workspace with a glob, an excluded member and a member pointing elsewhere.
    write(
        "Cargo.toml",
        &format!(
            "[workspace]\nmembers = [\"apps/*\", \"tools/cli\"]\nexclude = [\"apps/old\"]\n\n\
             [workspace.dependencies]\nravenrs = \"={VERSION}\"\n\
             ui = {{ package = \"ravenrs-widgets\", version = \"={VERSION}\" }}\nserde = \"1\"\n"
        ),
    );
    write(
        "apps/desk/Cargo.toml",
        "[package]\nname = \"desk\"\nversion = \"0.1.0\"\n\n[dependencies]\nravenrs.workspace = true\n\
         ui = { workspace = true, features = [\"x\"] }\n\n\
         [target.'cfg(target_arch = \"wasm32\")'.dependencies]\nravenrs-platform-web = \"=0.0.9\"\n",
    );
    write("apps/old/Cargo.toml", "[package]\nname = \"old\"\n\n[dependencies]\nravenrs = \"=0.0.1\"\n");
    write(
        "tools/cli/Cargo.toml",
        &format!(
            "[package]\nname = \"cli\"\n\n[dev-dependencies]\nraven = {{ package = \"ravenrs-app\", version = \"^{VERSION}\" }}\n"
        ),
    );
    let before: Vec<String> = ["Cargo.toml", "apps/desk/Cargo.toml", "tools/cli/Cargo.toml"]
        .iter()
        .map(|p| fs::read_to_string(root.join(p)).unwrap())
        .collect();

    // From the virtual root: workspace table plus every member, the excluded one skipped.
    let all = ravenrs_versions(root).unwrap();
    let packages: Vec<_> = all.iter().map(|d| (d.package.as_str(), d.version.as_str())).collect();
    assert!(packages.contains(&("ravenrs", &*format!("={VERSION}"))));
    assert!(packages.contains(&("ravenrs-widgets", &*format!("={VERSION}"))));
    assert!(packages.contains(&("ravenrs-app", &*format!("^{VERSION}"))));
    assert!(packages.contains(&("ravenrs-platform-web", "=0.0.9")));
    assert!(!packages.iter().any(|(_, v)| *v == "=0.0.1"), "excluded member was read");
    assert!(!packages.iter().any(|(p, _)| *p == "serde"));

    // From a member: inherited entries resolve through the nearest ancestor workspace.
    let member = ravenrs_versions(&root.join("apps/desk")).unwrap();
    let inherited = member.iter().find(|d| d.package == "ravenrs-widgets").unwrap();
    assert!(inherited.origin.contains("dependencies.ui"), "{}", inherited.origin);
    assert_eq!(inherited.version, format!("={VERSION}"));

    // Doctor reports the stale target-specific dependency, from the member and from the root.
    for at in [root.join("apps/desk"), root.to_path_buf()] {
        let output = cli(&at, &["doctor"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("ravenrs-platform-web") && stderr.contains("0.0.9"), "{stderr}");
    }

    // `package.workspace` names a workspace that is not an ancestor.
    write(
        "outside/ws/Cargo.toml",
        "[workspace]\nmembers = []\n\n[workspace.dependencies]\nravenrs = \"=0.0.2\"\n",
    );
    write(
        "outside/pkg/Cargo.toml",
        "[package]\nname = \"pkg\"\nworkspace = \"../ws\"\n\n[dependencies]\nravenrs.workspace = true\n",
    );
    let explicit = ravenrs_versions(&root.join("outside/pkg")).unwrap();
    assert_eq!(explicit[0].version, "=0.0.2");

    // A missing workspace entry is an error, not a silent pass.
    write(
        "apps/desk/Cargo.toml",
        "[package]\nname = \"desk\"\n\n[dependencies]\nravenrs-app.workspace = true\n",
    );
    assert!(ravenrs_versions(&root.join("apps/desk")).unwrap_err().contains("não declara ravenrs-app"));

    assert_eq!(fs::read_to_string(root.join("Cargo.toml")).unwrap(), before[0]);
    assert_eq!(fs::read_to_string(root.join("tools/cli/Cargo.toml")).unwrap(), before[2]);
}

#[test]
fn an_unknown_command_says_the_version_and_how_to_update() {
    let dir = TempDir::new().unwrap();
    let out = cli(dir.path(), &["ravenrs", "comando-futuro"]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains(env!("CARGO_PKG_VERSION")), "{err}");
    assert!(err.contains("'comando-futuro'"), "{err}");
    assert!(err.contains("cargo install --git https://github.com/alexkads/RavenRS cargo-ravenrs"), "{err}");
}

#[test]
fn version_warns_when_the_project_uses_a_newer_ravenrs() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"futuro\"\nversion = \"0.1.0\"\n\n[dependencies]\nravenrs = \"=999.1.0\"\n",
    )
    .unwrap();
    let out = cli(dir.path(), &["ravenrs", "--version"]);
    assert!(out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("mais novo"), "{err}");
    let doctor = cli(dir.path(), &["ravenrs", "doctor"]);
    let said = String::from_utf8_lossy(&doctor.stderr).to_string() + &String::from_utf8_lossy(&doctor.stdout);
    assert!(!doctor.status.success());
    assert!(said.contains("mais novo") && said.contains("cargo install --git"), "{said}");
}

#[test]
fn version_says_nothing_more_when_the_project_matches() {
    let dir = TempDir::new().unwrap();
    write_project(dir.path(), "hello", "desktop");
    let out = cli(dir.path(), &["ravenrs", "--version"]);
    assert!(out.status.success());
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
}
