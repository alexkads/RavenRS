//! What a downloaded project holds.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;

use ravenrs_project::{Project, RAVENRS_VERSION, WASM_BINDGEN_VERSION};

const SOURCE: &str = "use ravenrs::prelude::*;\n\nfn greeting() -> impl View {\n    Text::new(\"Oi\")\n}\n";

fn file(project: &Project, path: &str) -> String {
    project
        .files()
        .unwrap()
        .into_iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("{path}"))
        .contents
}

#[test]
fn every_file_a_program_needs_is_there() {
    let project = Project::new("Olá, mundo!").source(SOURCE);
    let paths: Vec<String> = project.files().unwrap().into_iter().map(|file| file.path).collect();
    assert_eq!(
        paths,
        ["Cargo.toml", "src/lib.rs", "src/main.rs", "web/index.html", "README.md", ".gitignore"]
    );
    assert_eq!(project.crate_name(), "ol-mundo");
    let manifest = file(&project, "Cargo.toml");
    assert!(manifest.contains("name = \"ol-mundo\""));
    assert!(manifest.contains(&format!("ravenrs = {{ version = \"{RAVENRS_VERSION}\" }}")), "{manifest}");
    let lib = file(&project, "src/lib.rs");
    assert!(lib.starts_with("//! Olá, mundo!, made in the RavenRS playground."));
    assert!(lib.contains(SOURCE.trim_end()), "the code is kept as written");
    assert!(lib.contains("pub fn app() -> impl ravenrs::widgets::View {\n    greeting()\n}"));
    assert!(file(&project, "src/main.rs").contains("ol_mundo::app()"));
    assert!(file(&project, "web/index.html").contains("./pkg/ol_mundo.js"));
}

#[test]
fn crates_and_features_are_said_once() {
    let project = Project::new("App")
        .source(SOURCE)
        .dependency("ravenrs", &["code"])
        .dependency("ravenrs-app", &[])
        .dependency("ravenrs", &["code", "live"]);
    let manifest = file(&project, "Cargo.toml");
    assert!(manifest.contains(&format!(
        "ravenrs = {{ version = \"{RAVENRS_VERSION}\", features = [\"code\", \"live\"] }}"
    )));
    assert!(manifest.contains(&format!("ravenrs-app = {{ version = \"{RAVENRS_VERSION}\" }}")));
    let local = Project::new("App").source(SOURCE).local_crates("/repo");
    assert!(file(&local, "Cargo.toml").contains("ravenrs = { path = \"/repo/crates/ravenrs\" }"));
}

#[test]
fn records_go_with_it_and_helpers_follow_the_code() {
    let project = Project::new("App").source(SOURCE).helper("fn picture() {}").records("{\"format\":1}");
    assert_eq!(file(&project, "src/seed.json"), "{\"format\":1}");
    let lib = file(&project, "src/lib.rs");
    assert!(lib.contains("pub const SEED: &str = include_str!(\"seed.json\");"));
    assert!(lib.find("fn greeting").unwrap() < lib.find("fn picture").unwrap());
}

#[test]
fn code_with_nothing_to_start_from_says_why() {
    assert!(
        Project::new("App").source("use ravenrs::prelude::*;").files().unwrap_err().contains("no function")
    );
    assert!(Project::new("App").source("fn (").files().unwrap_err().starts_with("line 1, column"));
    assert_eq!(Project::new("42").crate_name(), "app-42");
    assert_eq!(Project::new("").crate_name(), "app");
}

#[test]
fn the_zip_holds_the_files_in_a_folder_of_its_own() {
    let project = Project::new("Hello").source(SOURCE);
    let bytes = project.zip().unwrap();
    assert_eq!(bytes, project.zip().unwrap(), "the same project makes the same file");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut lib = String::new();
    archive.by_name("hello/src/lib.rs").unwrap().read_to_string(&mut lib).unwrap();
    assert_eq!(lib, file(&project, "src/lib.rs"));
    assert_eq!(archive.len(), 6);
}

#[test]
fn the_pinned_wasm_bindgen_is_the_one_the_crates_use() {
    let workspace = include_str!("../../../Cargo.toml");
    assert!(workspace.contains(&format!("wasm-bindgen = \"={WASM_BINDGEN_VERSION}\"")));
}
