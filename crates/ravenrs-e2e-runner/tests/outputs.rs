//! The F5 outputs (#668): the HTML report, the schemas, `e2e.toml` and the Rust a scenario
//! becomes.
#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use std::path::PathBuf;

use ravenrs_e2e_runner::config::Config;
use ravenrs_e2e_runner::report::{self, Report, render};
use ravenrs_e2e_runner::run::run_files;
use ravenrs_e2e_runner::rust;
use ravenrs_e2e_runner::session::{Failure, Session};
use ravenrs_e2e_wire::{Scenario, scenario_schema};
use serde_json::Value;

fn nothing(_: &str) -> Option<Result<Session, Box<Failure>>> {
    None
}

fn failed() -> Report {
    // One folder per call: the tests that use it run at the same time, and each removes its own.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ravenrs-e2e-runner-html-{}-{call}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.raventest");
    std::fs::write(&file, "scenario: Pedido <novo> & \"rápido\"\nopen Loja\nclick #salvar | Salvar\n")
        .unwrap();
    let report = run_files(&file, nothing).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    report
}

#[test]
fn the_html_page_shows_the_failure_escaped_and_what_did_not_run() {
    let page = render(&[failed()], true, "html");
    assert!(page.starts_with("<!doctype html>"), "{page}");
    assert!(page.contains("Pedido &lt;novo&gt; &amp; &quot;rápido&quot;"), "escaped");
    assert!(!page.contains("<novo>"));
    assert!(page.contains("<code>E2E_APP_START</code>"));
    assert!(page.contains("Como corrigir"));
    assert!(page.contains("não rodou"));
    assert!(page.contains("prefers-color-scheme:dark"), "reads in dark too");
    assert!(page.contains("width=device-width"), "fits a phone");
    assert!(!page.contains("http://") && !page.contains("<script"), "needs nothing from the network");
}

#[test]
fn a_kept_run_has_its_page() {
    let out = std::env::temp_dir().join(format!("ravenrs-e2e-runner-save-{}", std::process::id()));
    let dir = failed().save(&out).unwrap();
    assert!(std::fs::read_to_string(dir.join("relatorio.html")).unwrap().contains("E2E_APP_START"));
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn aggregate_film_links_find_each_file_and_saved_pages_remain_portable() {
    let root = std::env::temp_dir().join(format!("rts-html-links-{}", std::process::id()));
    let out = root.join("suíte # & anexos");
    let reports: Vec<_> = ["One", "Two"]
        .into_iter()
        .map(|name| {
            let folder = out.join(name);
            std::fs::create_dir_all(&folder).unwrap();
            let film = folder.join("filme-1.ravenmovie");
            std::fs::write(&film, b"a film attachment").unwrap();
            let mut report = failed();
            report.scenario = name.to_owned();
            report.films = vec![film.to_string_lossy().into_owned()];
            report
        })
        .collect();
    let hrefs = |html: &str| -> Vec<String> {
        html.split("href=\"")
            .skip(1)
            .map(|part| part.split('"').next().unwrap().replace("&amp;", "&"))
            .collect()
    };
    let aggregate = report::render(&reports, true, "html");
    let links = hrefs(&aggregate);
    assert_eq!(links.len(), 2);
    assert_ne!(links[0], links[1], "equal film names in different folders need different links");
    for (link, report) in links.iter().zip(&reports) {
        assert!(link.contains("%20") && link.contains("%23"), "{link}");
        let url = url::Url::parse(link).unwrap();
        assert_eq!(url.scheme(), "file");
        assert_eq!(url.fragment(), None);
        assert_eq!(url.query(), None);
        assert_eq!(url.to_file_path().unwrap(), std::fs::canonicalize(&report.films[0]).unwrap());
    }
    let saved = reports[0].save(&out).unwrap();
    let page = std::fs::read_to_string(saved.join("relatorio.html")).unwrap();
    let links = hrefs(&page);
    assert_eq!(links, ["filme-1.ravenmovie"]);
    let moved = root.join("moved");
    std::fs::rename(saved, &moved).unwrap();
    let base = url::Url::from_directory_path(&moved).unwrap();
    assert!(
        base.join(&links[0]).unwrap().to_file_path().unwrap().is_file(),
        "the page and film can move together"
    );
    std::fs::remove_dir_all(root).unwrap();
}

/// Every `.raventest` file kept in the repository.
fn every_scenario() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    let mut stack = vec![root.join("examples"), root.join("tools")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().is_some_and(|n| n != "target" && n != "node_modules") {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "raventest") {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn every_scenario_of_the_repository_goes_to_json_and_back_within_the_schema() {
    let schema = scenario_schema();
    let words: Vec<&str> = schema["properties"]["blocks"]["items"]["properties"]["kind"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    let needs: Vec<&str> = schema["properties"]["blocks"]["items"]["if"]["properties"]["kind"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    let files = every_scenario();
    assert!(files.len() >= 8, "{files:?}");
    for file in files {
        let scenario = Scenario::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let json = scenario.to_json();
        for block in json["blocks"].as_array().unwrap() {
            let kind = block["kind"].as_str().unwrap();
            assert!(words.contains(&kind), "{}: {kind}", file.display());
            assert!(!needs.contains(&kind) || block.get("text").is_some(), "{}: {block}", file.display());
            for key in block.as_object().unwrap().keys() {
                assert!(["kind", "target", "text", "title"].contains(&key.as_str()), "{key}");
            }
        }
        assert_eq!(Scenario::from_json(&json).unwrap(), scenario, "{}", file.display());
    }
}

#[test]
fn a_scenario_in_json_is_read_with_the_rules_of_the_file() {
    let json: Value = serde_json::from_str(
        r##"{"scenario": "Pedido", "blocks": [{"kind": "abrir", "target": "Loja"}, {"kind": "preencher", "target": "#nome", "text": "Ana"}]}"##,
    )
    .unwrap();
    let scenario = Scenario::from_json(&json).unwrap();
    assert_eq!(scenario.blocks[1].kind.word(), "fill");
    assert!(scenario.write().contains("fill #nome = Ana"));
    let missing: Value =
        serde_json::from_str(r##"{"scenario": "x", "blocks": [{"kind": "type", "target": "#a"}]}"##).unwrap();
    assert!(Scenario::from_json(&missing).unwrap_err().message.contains("text"));
    let unknown: Value =
        serde_json::from_str(r##"{"scenario": "x", "blocks": [{"kind": "pular", "target": "#a"}]}"##)
            .unwrap();
    assert!(Scenario::from_json(&unknown).is_err());
}

#[test]
fn the_report_schema_names_every_field_the_json_has() {
    let schema = report::schema();
    let fields = schema["properties"].as_object().unwrap();
    let report = failed().json();
    for key in report.as_object().unwrap().keys() {
        assert!(fields.contains_key(key), "the schema misses {key}");
    }
    let block_fields = schema["properties"]["blocks"]["items"]["properties"].as_object().unwrap();
    for key in report["blocks"][0].as_object().unwrap().keys() {
        assert!(block_fields.contains_key(key), "the schema misses blocks[].{key}");
    }
}

#[test]
fn e2e_toml_gives_the_folder_format_and_out_and_refuses_what_it_does_not_know() {
    let config =
        Config::parse("folder = \"testes\"\nformat = \"junit\"\nout = \"saida\"\nfilm = \"na-falha\"\nthemes = \"escuro\"\n")
            .unwrap();
    assert_eq!(config.folder, PathBuf::from("testes"));
    assert_eq!(config.format.as_deref(), Some("junit"));
    assert_eq!(config.out, Some(PathBuf::from("saida")));
    assert_eq!(
        config.film,
        Some(ravenrs_e2e_runner::driver::FilmMode::OnFailure),
        "film counts now, in Portuguese too"
    );
    assert!(config.planned_note().unwrap().contains("themes"));
    assert!(Config::parse("film = \"talvez\"").unwrap_err().contains("on-failure"));
    assert_eq!(Config::parse("").unwrap(), Config::default());
    assert!(Config::parse("formato = \"x\"").unwrap_err().contains("formato"));
    assert!(Config::parse("format = \"xml\"").unwrap_err().contains("junit"));
    assert!(Config::parse("folder = 3").unwrap_err().contains("folder"));
    let missing = Config::read(std::path::Path::new("/não/existe")).unwrap();
    assert_eq!(missing, Config::default());
}

#[test]
fn a_scenario_becomes_a_rust_test_of_its_app() {
    let scenario = Scenario::parse(
        "scenario: Lançar despesa\nopen Loja\ntype #nome = Ana \"A\" | Nome\nwait #ok\nopen Loja | De novo\ndrag #a = #b\n",
    )
    .unwrap();
    let code = rust::test(&scenario, "loja", "tests/e2e/a.raventest").unwrap();
    assert!(code.contains("fn scenario_lancar_despesa()"), "{code}");
    assert!(code.contains("CARGO_BIN_EXE_loja"));
    assert!(code.contains(r#"text: "Ana \"A\"".into()"#), "escaped: {code}");
    assert!(code.contains("Command::Wait { target: \"#ok\".into(), text: None }"));
    assert_eq!(code.matches("let mut app = open(").count(), 1);
    assert_eq!(code.matches("    app = open(").count(), 1, "the second Open replaces the app");
    // Built by hand: the file's reader already refuses it, JSON from a tool may not.
    let headless = Scenario {
        name: "x".into(),
        blocks: vec![ravenrs_e2e_wire::Block {
            kind: ravenrs_e2e_wire::Kind::Click,
            target: "#a".into(),
            text: None,
            title: "Clicar".into(),
        }],
    };
    assert!(rust::test(&headless, "loja", "x").unwrap_err().contains("abrir"));
    assert_eq!(rust::ident("2 Cenários — Ação!"), "cenario_2_cenarios_acao");
}

#[test]
fn generated_tests_compile_with_reserved_titles_and_helper_names() {
    // Use the real channel library Cargo built for this suite, without another Cargo build
    // or a second dependency tree. The generated tests only compile; they do not open an app.
    // Cargo kept every library beside the test (target/debug/deps); its newer build directory
    // keeps each unit apart (target/debug/build/<package>/<hash>/out). Both are searched.
    let exe = std::env::current_exe().unwrap();
    let mut dependencies = vec![exe.parent().unwrap().to_path_buf()];
    if let Some(profile) = exe.ancestors().skip(1).find(|p| p.join("build").is_dir()) {
        for package in std::fs::read_dir(profile.join("build")).unwrap().flatten() {
            for unit in std::fs::read_dir(package.path()).into_iter().flatten().flatten() {
                let out = unit.path().join("out");
                if out.is_dir() {
                    dependencies.push(out);
                }
            }
        }
    }
    let wire = dependencies
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().path()))
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("libravenrs_e2e_wire-") && name.ends_with(".rlib")
            })
        })
        .max_by_key(|path| path.metadata().unwrap().modified().unwrap())
        .unwrap();
    let folder = std::env::temp_dir().join(format!("ravenrs-generated-rust-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let mut modules = String::new();
    for (i, title) in [
        "type",
        "self",
        "Self",
        "async",
        "await",
        "match",
        "mod",
        "gen",
        "try",
        "open",
        "step",
        "2 Cenários — Ação!",
        "!!!",
        "Lançar despesa",
    ]
    .iter()
    .enumerate()
    {
        let scenario =
            Scenario::parse(&format!("scenario: {title}\nopen Loja\nfill #nome = Ana\ncheck #nome = Ana\n"))
                .unwrap();
        let code = rust::test(&scenario, "loja", "tests/e2e/gerado.raventest").unwrap();
        std::fs::write(folder.join(format!("scenario_{i}.rs")), code).unwrap();
        modules.push_str(&format!("mod scenario_{i};\n"));
    }
    let source = folder.join("generated.rs");
    std::fs::write(&source, modules).unwrap();
    // One `-L` per build unit: a warm target with several workspaces has thousands (#1086), more
    // than a command line takes. rustc reads them from a file instead.
    let search: String =
        dependencies.iter().map(|dir| format!("-L\ndependency={}\n", dir.display())).collect();
    let arguments = folder.join("search-paths");
    std::fs::write(&arguments, search).unwrap();
    let result = std::process::Command::new("rustc")
        .args(["--edition=2024", "--test", "--emit=metadata"])
        .arg("--extern")
        .arg(format!("ravenrs_e2e_wire={}", wire.display()))
        .arg(format!("@{}", arguments.display()))
        .arg("--out-dir")
        .arg(&folder)
        .arg(&source)
        .env("CARGO_BIN_EXE_loja", "not-run")
        .output()
        .unwrap();
    std::fs::remove_dir_all(folder).unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}
