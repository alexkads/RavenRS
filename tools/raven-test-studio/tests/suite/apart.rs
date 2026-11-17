//! Apps opened apart, in a child process over the test channel, such as Raven Finanças.

use std::path::PathBuf;

use raven_test_studio::apps::{is_older, open_finance_from, pick_installed};
use raven_test_studio::report::Report;
use raven_test_studio::runner::{Step, step_of};
use raven_test_studio::sample;

#[test]
fn an_installed_copy_from_before_the_channel_is_never_opened() {
    let copy = |path: &str, version: Option<&str>| (PathBuf::from(path), version.map(str::to_owned));
    // 0.1.38 would read --e2e-serve as the path of a database and open its window.
    assert_eq!(pick_installed([copy("/velho", Some("0.1.38"))], "0.1.39"), Err(Some("0.1.38".to_owned())));
    assert_eq!(
        pick_installed([copy("/velho", Some("0.1.38")), copy("/novo", Some("0.1.40"))], "0.1.39"),
        Ok(PathBuf::from("/novo"))
    );
    // A copy that does not say its version is not trusted with the option either.
    assert_eq!(pick_installed([copy("/mudo", None)], "0.1.39"), Err(None));
    assert!(is_older("0.1.9", "0.1.39") && !is_older("0.2.0", "0.1.39") && !is_older("0.1.39", "0.1.39"));
}

#[test]
fn scenario_files_run_from_any_folder_as_the_qa_team_keeps_them() {
    let folder = std::env::temp_dir().join(format!("rts-pasta-do-qa-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    let form = "cenário: Formulário de exemplo\nabrir Novo lançamento\ndigitar #descricao = Feira | Descrição\ndigitar #valor = 9,90 | Valor\nclicar #salvar | Salvar\nconferir #resultado = Salvo: Feira, R$ 9,90 | Conferir\n";
    std::fs::write(folder.join("a-formulario.raventest"), form).expect("file");
    let one = raven_test_studio::run_files(&folder.join("a-formulario.raventest")).expect("runs");
    assert_eq!(one.scenario, "Formulário de exemplo");
    assert!(one.passed(), "{}", one.text());
    assert!(!one.apart, "the sample form is compiled into the Studio");

    // A folder runs every file in it, one after the other.
    std::fs::write(
        folder.join("b-quebrado.raventest"),
        "scenario: Quebrado\nopen Novo lançamento\nclick #nao-existe\n",
    )
    .expect("file");
    let both = raven_test_studio::run_files(&folder).expect("runs");
    assert_eq!(both.total, 7);
    assert!(!both.passed(), "the second fails on purpose");
    assert_eq!(both.outcomes.last().and_then(|o| o.code.as_deref()), Some("E2E_NOT_FOUND"));

    // A file that does not read says which line.
    std::fs::write(folder.join("c-errado.raventest"), "scenario: x\nopen App\npular\n").expect("file");
    let why = raven_test_studio::run_files(&folder).expect_err("a wrong line");
    assert!(why.contains("c-errado.raventest: linha 3"), "{why}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn an_app_apart_is_told_from_one_compiled_in() {
    let apart = ravenrs_e2e_wire::Scenario::parse("scenario: x\nopen Raven Finanças\nclick Lançamentos\n")
        .expect("reads");
    assert!(sample::opens_apart(&sample::from_scenarios(&[apart])));
    assert!(!sample::opens_apart(&sample::self_test()), "the self-test runs compiled in");
    let graph = sample::from_scenarios(&[]);
    assert!(graph.nodes.is_empty());
    let one = sample::from_scenarios(&[ravenrs_e2e_wire::Scenario::parse(
        "scenario: y\nopen Novo lançamento\nclick #salvar | Salvar\n",
    )
    .expect("reads")]);
    let steps: Vec<Step> = one.nodes.iter().map(|n| step_of(n).expect("a step")).collect();
    assert_eq!(steps[0], Step::Open("Novo lançamento".to_owned()));
}

#[test]
fn something_that_is_not_the_app_fails_to_open_with_a_reason() {
    // This test binary does not speak the channel: it rejects the option and ends.
    let this = std::env::current_exe().expect("this binary");
    let why = open_finance_from(&this).expect_err("not an app on the channel");
    assert_eq!(why.code.as_deref(), Some("E2E_APP_START"), "{why}");
}

#[test]
fn a_report_of_an_app_apart_says_what_that_did_not_check() {
    let json = serde_json::json!({ "scenario": "x", "blocks": [], "total": 0, "opened_apart": true });
    let report = Report::from_json(&json).expect("a report");
    assert!(report.not_verified()[0].contains("à parte"), "{:?}", report.not_verified());
    assert_eq!(report.json()["opened_apart"], true);
}
