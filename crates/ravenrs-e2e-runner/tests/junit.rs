//! The JUnit XML of `--format junit`, byte for byte (written with `ravenrs-junit` since #880), and
//! read back by the same crate a CI reads it with.
#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use ravenrs_e2e_runner::report::Report;
use serde_json::json;

fn reports() -> Vec<Report> {
    let passed = json!({
        "scenario": "Loja & <carrinho>",
        "total": 2,
        "duration_ms": 1250,
        "blocks": [
            { "id": "a", "kind": "open", "title": "Abrir", "line": "Loja", "duration_ms": 1000 },
            { "id": "b", "kind": "click", "title": "Salvar", "line": "#salvar", "duration_ms": 250 },
        ],
    });
    let failed = json!({
        "scenario": "Cupom",
        "total": 3,
        "duration_ms": 30,
        "not_run": ["Conferir"],
        "films": ["/tmp/filme \"1\".ravenmovie"],
        "blocks": [
            { "id": "a", "kind": "open", "title": "Abrir", "line": "Loja", "duration_ms": 10 },
            {
                "id": "b", "kind": "check", "title": "Total", "line": "#total = 90", "duration_ms": 20,
                "error": "E2E_NOT_FOUND: veio 100", "code": "E2E_NOT_FOUND",
                "fix": "confira o cupom", "screen": "Janela\n  total: 100",
            },
        ],
    });
    vec![Report::from_json(&passed).unwrap(), Report::from_json(&failed).unwrap()]
}

/// What it wrote before `ravenrs-junit`: the move changed no byte.
const EXPECTED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="ravenrs-e2e" tests="5" failures="1" skipped="1" time="1.280">
  <testsuite name="Loja &amp; &lt;carrinho&gt;" tests="2" failures="0" skipped="0" time="1.250">
    <testcase classname="Loja &amp; &lt;carrinho&gt;" name="1 Abrir: Loja" time="1.000"/>
    <testcase classname="Loja &amp; &lt;carrinho&gt;" name="2 Salvar: #salvar" time="0.250"/>
  </testsuite>
  <testsuite name="Cupom" tests="3" failures="1" skipped="1" time="0.030">
    <testcase classname="Cupom" name="1 Abrir: Loja" time="0.010"/>
    <testcase classname="Cupom" name="2 Total: #total = 90" time="0.020">
      <failure type="E2E_NOT_FOUND" message="E2E_NOT_FOUND: veio 100">E2E_NOT_FOUND: veio 100
como corrigir: confira o cupom
tela do app testado:
Janela
  total: 100</failure>
    </testcase>
    <testcase classname="Cupom" name="3 Conferir" time="0">
      <skipped message="um passo antes falhou"/>
    </testcase>
    <system-out>[[ATTACHMENT|/tmp/filme &quot;1&quot;.ravenmovie]]
</system-out>
  </testsuite>
</testsuites>
"#;

#[test]
fn the_junit_is_the_same_byte_for_byte_and_reads_back() {
    let xml = Report::junit(&reports());
    assert_eq!(xml, EXPECTED);
    let summary = ravenrs_junit::read(&xml).unwrap().summary();
    assert_eq!((summary.tests, summary.failures, summary.skipped, summary.time_ms), (5, 1, 1, 1280));
    assert_eq!(summary.failed, ["Cupom.2 Total: #total = 90"]);
}
