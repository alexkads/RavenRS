//! The webhook of GitHub (#910) on `raven-ci servir` running: the route answers with no one signed
//! in, takes only what the secret of the vault signed, and the rest of the API still asks to sign
//! in.

use std::io::Write;
use std::process::Stdio;

use serde_json::{Value, json};

use crate::server::{create_user, raven, start_with, token};

const SECRET: &str = "segredo-do-webhook-de-teste";

#[test]
fn a_signed_push_asks_for_a_build_with_no_sign_in_and_an_unsigned_one_is_refused() {
    let running = start_with("webhook", |_| {});
    assert!(create_user(&running.dir, "bia", "Executores").contains("Executores"));
    let mut child = raven(&running.dir)
        .args(["credencial", "criar", "webhook-site", "--tipo", "token"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(json!({ "token": SECRET }).to_string().as_bytes())
        .expect("written");
    let made = child.wait_with_output().expect("ends");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stdout));

    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let url = |path: &str| format!("http://{}{path}", running.address);
    let admin = token(&running, "ana");
    let job = json!({
        "name": "site",
        "pipeline": "[[stages]]\nname = \"A\"\nsteps = [\"echo a\"]\n",
        "settings": { "triggers": [
            { "id": "github-webhook", "repository": "dono/site", "secret": "webhook-site", "branch": "main" }
        ] },
    });
    let mut saved = agent
        .post(&url("/api/ci/jobs"))
        .header("Authorization", &format!("Bearer {admin}"))
        .header("Content-Type", "application/json")
        .send(job.to_string())
        .expect("answered");
    let said = saved.body_mut().read_to_string().unwrap_or_default();
    assert_eq!(saved.status().as_u16(), 200, "{said}");
    assert!(said.contains("\"warnings\":[]"), "{said}");

    let body = json!({
        "ref": "refs/heads/main",
        "after": "89abcdef0123456789abcdef0123456789abcdef",
        "repository": { "full_name": "dono/site" },
        "pusher": { "name": "ana" },
    })
    .to_string();
    let deliver = |signature: Option<String>, id: &str| {
        let mut request = agent
            .post(&url("/hooks/github"))
            .header("Content-Type", "application/json")
            .header("X-GitHub-Event", "push")
            .header("X-GitHub-Delivery", id);
        if let Some(signature) = signature {
            request = request.header("X-Hub-Signature-256", &signature);
        }
        let mut answer = request.send(body.clone()).expect("answered");
        let text = answer.body_mut().read_to_string().unwrap_or_default();
        (answer.status().as_u16(), serde_json::from_str::<Value>(&text).unwrap_or(Value::Null))
    };

    // The refusals are the plugin's, not the sign-in's: the route was reached with no token.
    let refused = json!("A notificação não foi aceita.");
    let (status, answer) = deliver(None, "sem-assinatura");
    assert_eq!((status, &answer["message"]), (401, &refused), "{answer}");
    let (status, answer) = deliver(Some(ravenrs_app_server::sign(b"outro", body.as_bytes())), "errada");
    assert_eq!((status, &answer["message"]), (401, &refused), "{answer}");
    let (status, answer) =
        deliver(Some(ravenrs_app_server::sign(SECRET.as_bytes(), body.as_bytes())), "certa");
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["builds"][0]["job"], "site");
    assert_eq!(answer["builds"][0]["number"], 1);

    // The rest of the API still asks to sign in.
    let mut jobs = agent.get(&url("/api/ci/jobs")).call().expect("answered");
    assert_eq!(jobs.status().as_u16(), 401);
    let _ = jobs.body_mut().read_to_string();
}
