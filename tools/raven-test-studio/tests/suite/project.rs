//! A Rust project the Studio is pointed at, and the scenarios an AI suggests for its apps.

use std::path::{Path, PathBuf};

use raven_test_studio::apps::App;
use raven_test_studio::project::{self, Project};
use raven_test_studio::suggest;
use serde_json::json;

/// A small project on disk: an app with the test channel and a scenario, an app without the
/// channel, a library of components, and a package that does not use RavenRS.
fn sample_project() -> (PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!("rts-projeto-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let write = |path: &str, text: &str| {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("parent")).expect("folder");
        std::fs::write(file, text).expect("file");
    };
    write(
        "loja/src/main.rs",
        "fn main() { if std::env::args().nth(1).as_deref() == Some(\"--e2e-serve\") {} }\n\
         struct Caixa;\nimpl View for Caixa {}\n\
         pub fn tela() -> impl View { Button::new(\"Pagar\").test_id(\"pagar\") }\n",
    );
    write("loja/tests/e2e/pagar.raventest", "scenario: Pagar\nopen Loja\nclick #pagar\n");
    write("loja/tests/e2e/errado.raventest", "scenario: Errado\nopen Loja\npular\n");
    write("estoque/src/main.rs", "struct Lista;\nimpl<'a> Component for Lista {}\n");
    write("botoes/src/lib.rs", "pub struct Grande;\nimpl View for Grande {}\n");
    write("calculo/src/lib.rs", "pub fn soma() {}\n");
    let package = |name: &str, deps: &[&str], bins: &[&str], metadata: serde_json::Value| {
        json!({
            "name": name,
            "manifest_path": root.join(name).join("Cargo.toml"),
            "dependencies": deps.iter().map(|d| json!({ "name": d })).collect::<Vec<_>>(),
            "targets": bins.iter().map(|b| json!({ "kind": ["bin"], "name": b })).chain([json!({ "kind": ["lib"], "name": name })]).collect::<Vec<_>>(),
            "metadata": metadata,
        })
    };
    let metadata = json!({ "packages": [
        package("loja", &["ravenrs", "ravenrs-e2e-wire"], &["loja"], json!({ "ravenrs": { "e2e": { "app": "Loja", "since": "1.2.0" } } })),
        package("estoque", &["ravenrs-app-ui"], &["estoque"], json!(null)),
        package("botoes", &["ravenrs-widgets-base"], &[], json!(null)),
        package("calculo", &["serde"], &[], json!(null)),
        json!({ "name": "fora", "manifest_path": "/outro/lugar/Cargo.toml", "dependencies": [{ "name": "ravenrs" }], "targets": [] }),
    ]});
    (root, metadata)
}

#[test]
fn a_project_says_its_apps_their_channel_scenarios_and_screens() {
    let (root, metadata) = sample_project();
    let project: Project = project::of_metadata(&root, &metadata);
    let names: Vec<&str> = project.found.iter().map(|f| f.package.as_str()).collect();
    assert_eq!(names, ["botoes", "estoque", "loja"], "only RavenRS packages inside the folder");

    let loja = &project.found[2];
    assert_eq!(loja.name, "Loja", "the name it declares for the Studio");
    assert!(loja.is_app() && loja.channel);
    assert_eq!(loja.scenarios.len(), 2);
    assert!(loja.scenarios[0].1.as_ref().is_err_and(|why| why.contains("linha 3")), "{:?}", loja.scenarios);
    assert_eq!(loja.scenarios[1].1.as_deref(), Ok("Pagar"));
    let screens: Vec<(&str, &str, usize)> =
        loja.screens.iter().map(|s| (s.name.as_str(), s.kind, s.line)).collect();
    assert_eq!(screens, [("Caixa", "View", 3), ("tela", "fn", 4)]);
    assert_eq!(loja.screens[0].test_ids, ["pagar"]);

    let estoque = &project.found[1];
    assert!(estoque.is_app() && !estoque.channel, "an app without the channel");
    assert_eq!(estoque.screens[0].kind, "Component");
    assert!(!project.found[0].is_app(), "a library of components");

    let text = project.text();
    assert!(text.contains("canal de teste: não") && text.contains("ravenrs-e2e-wire"), "{text}");
    assert!(text.contains("biblioteca"), "{text}");
    let json = project.json();
    assert_eq!(json["apps"], 2);
    assert_eq!(json["libraries"], 1);
    assert_eq!(project.scenario_folders(), [root.join("loja/tests/e2e")]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_app_declares_itself_for_the_studio_in_its_cargo_toml() {
    let finance =
        App::declared_in(include_str!("../../../../examples/family-finance/Cargo.toml")).expect("declared");
    assert_eq!(finance.name, "Raven Finanças");
    assert_eq!((finance.package.as_str(), finance.binary.as_str()), ("family-finance", "family-finance"));
    assert_eq!(finance.since, "0.1.39");
    assert_eq!(finance.variable, "RAVEN_FINANCAS");
    assert_eq!(finance.installed.len(), 3);
    assert!(App::declared_in("[package]\nname = \"x\"\n").is_none(), "one that declares nothing");
}

#[test]
fn a_folder_that_is_not_a_rust_project_says_so() {
    let empty = std::env::temp_dir().join(format!("rts-vazio-{}", std::process::id()));
    std::fs::create_dir_all(&empty).expect("folder");
    let why = project::read(&empty).expect_err("no Cargo.toml");
    assert!(why.contains("não é um projeto Rust"), "{why}");
    let _ = std::fs::remove_dir_all(&empty);
    assert!(project::read(Path::new("/nao/existe")).is_err());
}

#[test]
fn the_scenarios_in_an_ai_answer_are_read_one_by_one_and_checked() {
    let answer = "Aqui vão:\n```raventest\nscenario: Pagar\nopen Loja\nclick #pagar | Pagar\n\ncenário: Conferir\nabrir Loja\nconferir #total = R$ 0,00\n```\nscenario: Outro app\nopen Banco\n";
    let found = suggest::scenarios_in(answer);
    assert_eq!(found.len(), 3);
    assert!(found[1].starts_with("cenário: Conferir"));
    assert!(ravenrs_e2e_wire::Scenario::parse(&found[0]).is_ok());

    let (root, metadata) = sample_project();
    let project = project::of_metadata(&root, &metadata);
    let (system, user) =
        suggest::prompt("Loja", "- button \"Pagar\" #pagar", &project.found[2], &["Pagar".to_owned()]);
    assert!(system.contains("scenario:") && system.contains("never invent a test id"), "{system}");
    assert!(
        user.contains("#pagar") && user.contains("Caixa") && user.contains("suggest others: Pagar"),
        "{user}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
