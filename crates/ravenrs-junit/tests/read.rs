//! What the tools write, read: Maven Surefire, pytest, cargo nextest, suites inside suites, text in
//! CDATA and entities; what this crate writes, read back the same; and XML that is not JUnit.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use ravenrs_junit::{Case, Outcome, Problem, Suite, Suites, read};

const SUREFIRE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" name="com.loja.CarrinhoTest" time="0.052" tests="3" errors="1" skipped="0" failures="1">
  <properties><property name="java.version" value="21"/></properties>
  <testcase name="somaItens" classname="com.loja.CarrinhoTest" time="0.011"/>
  <testcase name="aplicaCupom" classname="com.loja.CarrinhoTest" time="0.020">
    <failure message="expected: &lt;90&gt; but was: &lt;100&gt;" type="org.opentest4j.AssertionFailedError"><![CDATA[org.opentest4j.AssertionFailedError: expected: <90> but was: <100>
	at com.loja.CarrinhoTest.aplicaCupom(CarrinhoTest.java:31)]]></failure>
    <system-out><![CDATA[log do caso]]></system-out>
  </testcase>
  <testcase name="frete" classname="com.loja.CarrinhoTest" time="0.021">
    <error message="NPE" type="java.lang.NullPointerException">java.lang.NullPointerException at Frete.java:9 &amp; mais</error>
  </testcase>
</testsuite>
"#;

const PYTEST: &str = r#"<?xml version="1.0" encoding="utf-8"?><testsuites name="pytest tests"><testsuite name="pytest" errors="0" failures="0" skipped="1" tests="2" time="0.031" timestamp="2026-11-22T10:00:00" hostname="mac"><testcase classname="tests.test_api" name="test_health" time="0.001" /><testcase classname="tests.test_api" name="test_lento" time="0.000"><skipped type="pytest.skip" message="lento demais">tests/test_api.py:12: lento demais</skipped></testcase></testsuite></testsuites>"#;

const NEXTEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="2" failures="1" errors="0" uuid="x" timestamp="2026-11-22T10:00:00Z" time="0.120">
    <testsuite name="raven-ci-core::suite" tests="2" disabled="0" errors="0" failures="1">
        <testcase name="model::a_job_is_kept" classname="raven-ci-core::suite" timestamp="x" time="0.010">
        </testcase>
        <testcase name="model::a_build_fails" classname="raven-ci-core::suite" timestamp="x" time="0.110">
            <failure type="test failure">thread 'main' panicked</failure>
            <system-out>saída</system-out>
            <system-err>erro</system-err>
        </testcase>
    </testsuite>
</testsuites>
"#;

#[test]
fn surefire_a_failure_an_error_cdata_and_entities() {
    let suites = read(SUREFIRE).expect("reads");
    assert_eq!(suites.suites.len(), 1);
    let suite = &suites.suites[0];
    assert_eq!((suite.name.as_str(), suite.time_ms), ("com.loja.CarrinhoTest", Some(52)));
    assert_eq!(suite.cases.len(), 3);
    let Outcome::Failure(problem) = &suite.cases[1].outcome else {
        panic!("a failure: {:?}", suite.cases[1])
    };
    assert_eq!(problem.kind.as_deref(), Some("org.opentest4j.AssertionFailedError"));
    assert_eq!(problem.message.as_deref(), Some("expected: <90> but was: <100>"));
    assert!(problem.text.contains("CarrinhoTest.java:31"), "{problem:?}");
    let Outcome::Error(problem) = &suite.cases[2].outcome else { panic!("an error") };
    assert_eq!(problem.text, "java.lang.NullPointerException at Frete.java:9 & mais");
    let summary = suites.summary();
    assert_eq!((summary.tests, summary.failures, summary.errors, summary.skipped), (3, 1, 1, 0));
    assert_eq!(summary.failed, ["com.loja.CarrinhoTest.aplicaCupom", "com.loja.CarrinhoTest.frete"]);
    assert_eq!(summary.passed(), 1);
    assert!(!summary.ok());
}

#[test]
fn pytest_on_one_line_with_a_skipped() {
    let suites = read(PYTEST).expect("reads");
    assert_eq!(suites.name.as_deref(), Some("pytest tests"));
    let summary = suites.summary();
    assert_eq!((summary.tests, summary.skipped, summary.time_ms), (2, 1, 31));
    assert!(summary.ok());
    assert_eq!(suites.suites[0].cases[1].outcome, Outcome::skipped("lento demais"));
}

#[test]
fn nextest_with_its_names_and_the_output_of_a_case() {
    let suites = read(NEXTEST).expect("reads");
    let summary = suites.summary();
    assert_eq!((summary.tests, summary.failures), (2, 1));
    assert_eq!(summary.failed, ["raven-ci-core::suite.model::a_build_fails"]);
    assert_eq!(summary.time_ms, 120, "the sum of the cases when the suite says no time");
}

#[test]
fn suites_inside_suites_come_in_the_order_they_open() {
    let xml = r#"<testsuites><testsuite name="fora"><testcase name="a"/><testsuite name="dentro"><testcase name="b"/></testsuite><testcase name="c"/></testsuite><testsuite name="vazia"/></testsuites>"#;
    let suites = read(xml).expect("reads");
    let names: Vec<&str> = suites.suites.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["fora", "dentro", "vazia"]);
    assert_eq!(suites.suites[0].cases.len(), 2);
    assert_eq!(suites.suites[0].cases[0].classname, "fora", "the class is the suite when not said");
    assert_eq!(suites.summary().tests, 3);
}

#[test]
fn what_it_writes_it_reads_back() {
    let suites = Suites::named("run & \"aspas\"")
        .suite(
            Suite::new("a <suíte>")
                .time_ms(1250)
                .case(Case::new("a", "passa").time_ms(5))
                .case(Case::new("a", "falha").time_ms(7).outcome(Outcome::Failure(Problem {
                    kind: Some("CODE".to_owned()),
                    message: Some("disse 'não'".to_owned()),
                    text: "linha 1\nlinha 2 < 3".to_owned(),
                })))
                .case(Case::new("a", "quebra").time_ms(3).outcome(Outcome::error("sem texto")))
                .case(Case::new("a", "pula").time_ms(0).outcome(Outcome::Skipped { message: None }))
                .system_out("[[ATTACHMENT|/tmp/filme.mp4]]\n"),
        )
        .suite(Suite::new("vazia").time_ms(0));
    let xml = suites.to_xml();
    assert!(xml.contains(r#"errors="1""#), "{xml}");
    assert!(!xml.contains('\u{7}'));
    assert_eq!(read(&xml).expect("reads"), suites, "{xml}");
    // A time not measured is written as 0, and read back as 0.
    let unmeasured = Suites::default().suite(Suite::new("s").case(Case::new("s", "x")));
    assert_eq!(read(&unmeasured.to_xml()).expect("reads").suites[0].cases[0].time_ms, Some(0));
}

#[test]
fn control_characters_are_left_out_and_xml_that_is_not_junit_is_refused() {
    let suites = Suites::default().suite(Suite::new("s").case(Case::new("s", "sino\u{7}")));
    assert!(suites.to_xml().contains(r#"name="sino""#));
    let broken = read("<testsuite><testcase name=\"a\"></testsuite>").expect_err("broken");
    assert!(broken.to_string().contains("JUnit ilegível"), "{broken}");
    assert!(read("<html><body/></html>").expect_err("not junit").reason.contains("testsuite"));
    assert!(read("").is_err());
}
