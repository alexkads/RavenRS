//! The `/metrics` of the plugin `metrics` (#913) on `raven-ci servir` running: behind the sign-in
//! by default, open at `/hooks/metrics` only when its settings say so.

use serde_json::json;

use crate::server::{create_user, start_with, token};

#[test]
fn metrics_ask_to_sign_in_and_open_for_prometheus_only_when_told() {
    let running = start_with("metricas", |_| {});
    assert!(create_user(&running.dir, "leo", "Leitores").contains("Leitores"));
    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let url = |path: &str| format!("http://{}{path}", running.address);
    let get = |path: &str, key: Option<&str>| {
        let mut request = agent.get(&url(path));
        if let Some(key) = key {
            request = request.header("Authorization", &format!("Bearer {key}"));
        }
        let mut answer = request.call().expect("answered");
        (answer.status().as_u16(), answer.body_mut().read_to_string().unwrap_or_default())
    };

    assert_eq!(get("/api/ci/metrics", None).0, 401);
    let leo = token(&running, "leo");
    let (status, text) = get("/api/ci/metrics", Some(&leo));
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("# TYPE raven_ci_queue_builds gauge"), "{text}");
    assert!(text.contains("raven_ci_server_memory_bytes"), "{text}");
    assert_eq!(get("/hooks/metrics", None).0, 404, "closed until the settings open it");

    let admin = token(&running, "ana");
    let config = json!({ "plugins": { "metrics": { "settings": { "open": true } } } });
    let mut applied = agent
        .post(&url("/api/ci/config"))
        .header("Authorization", &format!("Bearer {admin}"))
        .header("Content-Type", "application/json")
        .send(config.to_string())
        .expect("answered");
    let said = applied.body_mut().read_to_string().unwrap_or_default();
    assert_eq!(applied.status().as_u16(), 200, "{said}");
    let (status, text) = get("/hooks/metrics", None);
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("raven_ci_plugins{state=\"on\"}"), "{text}");
}
