//! The route of the converter the panel calls (#919): the TOML, whether it is complete, and each
//! note with its line.

use raven_ci_plugin_api::{CiPlugin, HttpRequest};
use raven_ci_plugin_format_jenkinsfile::{CONVERT_ROUTE, JenkinsfilePlugin};

fn ask(body: &str) -> (u16, serde_json::Value) {
    let route = JenkinsfilePlugin.routes().into_iter().find(|r| r.path == CONVERT_ROUTE).expect("the route");
    let request = HttpRequest {
        method: "POST".into(),
        path: CONVERT_ROUTE.into(),
        body: body.as_bytes().to_vec(),
        ..HttpRequest::default()
    };
    let answer = (route.handler)(&request);
    (answer.status, serde_json::from_slice(&answer.body).expect("json"))
}

#[test]
fn the_route_converts_and_points_at_the_line_not_converted() {
    let jenkinsfile = "pipeline {\n  agent any\n  stages {\n    stage('Build') { steps { sh 'make' } }\n    stage('Resumo') { steps { script { gerar() } } }\n  }\n}\n";
    let (status, body) = ask(&serde_json::json!({ "text": jenkinsfile }).to_string());
    assert_eq!(status, 200, "{body}");
    assert!(body["toml"].as_str().expect("toml").contains("make"), "{body}");
    assert_eq!(body["complete"], false);
    let pending: Vec<&serde_json::Value> =
        body["notes"].as_array().expect("notes").iter().filter(|n| n["kind"] == "não convertido").collect();
    // The script and the stage it left empty, both on line 5.
    assert_eq!(pending.len(), 2, "{body}");
    assert!(pending.iter().all(|note| note["line"] == 5), "{body}");
    let (status, _) = ask("{}");
    assert_eq!(status, 400, "no text, no conversion");
}
