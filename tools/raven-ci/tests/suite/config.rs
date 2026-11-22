//! The configuration as code (#898), on the binary against `raven-ci servir` running: a file with
//! two jobs applied with no restart, the one with `cron` built by its schedule, the file changed and
//! applied again (the next build runs the new pipeline), a job taken out kept until `--remover`,
//! and the routes refused to who is not an administrator.

use std::path::Path;
use std::process::Output;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::server::{Running, create_user, raven, start_with, token};

fn apply(running: &Running, key: &str, file: &Path, more: &[&str]) -> Output {
    raven(&running.dir)
        .arg("aplicar")
        .arg(file)
        .args(more)
        .args(["--servidor", &format!("http://{}", running.address)])
        .env("RAVEN_CI_TOKEN", key)
        .output()
        .expect("runs")
}

fn said(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn get(running: &Running, key: &str, path: &str) -> Value {
    let mut answer = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent()
        .get(&format!("http://{}{path}", running.address))
        .header("Authorization", &format!("Bearer {key}"))
        .call()
        .expect("answered");
    serde_json::from_str(&answer.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null)
}

/// The log of the last build of `job` that ended, once one whose log has `text` ended.
fn wait_for(running: &Running, key: &str, job: &str, text: &str) -> String {
    let began = Instant::now();
    loop {
        let builds = get(running, key, &format!("/api/ci/builds?job={job}"));
        for build in builds.as_array().into_iter().flatten() {
            let ended = !matches!(build["status"].as_str(), Some("queued" | "running") | None);
            let number = build["number"].as_u64().unwrap_or_default();
            if ended {
                let lines = get(running, key, &format!("/api/ci/log?job={job}&number={number}&after=0"));
                let log: Vec<&str> =
                    lines.as_array().into_iter().flatten().filter_map(|l| l["text"].as_str()).collect();
                if log.iter().any(|line| line.contains(text)) {
                    return log.join("\n");
                }
            }
        }
        assert!(began.elapsed() < Duration::from_secs(30), "no build of {job} said {text}: {builds}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn file(dir: &Path, agenda: &str, with_manual: bool) -> std::path::PathBuf {
    let mut text = "[plugins.cron]\nsettings = { tick_ms = 100 }\n\n[folders]\nnoite = \"Noturnos\"\n\n[[jobs]]\nname = \"noite/agenda\"\nfile = \"agenda.toml\"\nsettings = { triggers = [{ id = \"cron\", every = \"1s\" }] }\n".to_owned();
    if with_manual {
        text.push_str("\n[[jobs]]\nname = \"manual\"\npipeline = \"\"\"\n[[stages]]\nname = \"A mão\"\nsteps = [\"echo manual\"]\n\"\"\"\nsettings = { triggers = [{ id = \"api\" }] }\n");
    }
    std::fs::write(
        dir.join("agenda.toml"),
        format!("[[stages]]\nname = \"Agenda\"\nsteps = [\"echo {agenda}\"]\n"),
    )
    .expect("written");
    let path = dir.join("raven-ci.toml");
    std::fs::write(&path, text).expect("written");
    path
}

#[test]
fn a_file_applied_to_a_running_server_makes_its_jobs_and_a_change_takes_with_no_restart() {
    let running = start_with("config", |_| {});
    let key = token(&running, "ana");
    let config = running.dir.join("config");
    std::fs::create_dir_all(&config).expect("a folder");

    let first = apply(&running, &key, &file(&config, "primeira-versao", true), &[]);
    assert_eq!(first.status.code(), Some(0), "{}", said(&first));
    let text = said(&first);
    assert!(text.contains("noite/agenda: criado") && text.contains("manual: criado"), "{text}");
    wait_for(&running, &key, "noite/agenda", "primeira-versao");

    let again = apply(&running, &key, &file(&config, "segunda-versao", true), &[]);
    assert!(said(&again).contains("noite/agenda: mudado"), "{}", said(&again));
    wait_for(&running, &key, "noite/agenda", "segunda-versao");
    let same = apply(&running, &key, &file(&config, "segunda-versao", true), &[]);
    assert!(said(&same).contains("nada mudou"), "{}", said(&same));

    let kept = apply(&running, &key, &file(&config, "segunda-versao", false), &[]);
    assert!(said(&kept).contains("manual: fora do arquivo; fica"), "{}", said(&kept));
    let removed = apply(&running, &key, &file(&config, "segunda-versao", false), &["--remover"]);
    assert!(said(&removed).contains("manual: removido"), "{}", said(&removed));
    let jobs = get(&running, &key, "/api/ci/jobs");
    let names: Vec<&str> = jobs.as_array().into_iter().flatten().filter_map(|j| j["name"].as_str()).collect();
    assert_eq!(names, ["noite/agenda"]);
}

#[test]
fn only_administrators_change_jobs_and_a_wrong_field_is_named() {
    let running = start_with("config-papeis", |_| {});
    assert!(create_user(&running.dir, "bia", "Executores").contains("Executores"));
    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let post = |path: &str, key: Option<&str>, body: &Value| {
        let mut request = agent
            .post(&format!("http://{}{path}", running.address))
            .header("Content-Type", "application/json");
        if let Some(key) = key {
            request = request.header("Authorization", &format!("Bearer {key}"));
        }
        let mut answer = request.send(body.to_string()).expect("answered");
        (answer.status().as_u16(), answer.body_mut().read_to_string().unwrap_or_default())
    };
    let job = json!({ "name": "site", "pipeline": "[[stages]]\nname = \"A\"\nsteps = [\"echo a\"]\n" });
    assert_eq!(post("/api/ci/jobs", None, &job).0, 401);
    let bia = token(&running, "bia");
    assert_eq!(post("/api/ci/jobs", Some(&bia), &job).0, 403);
    let ana = token(&running, "ana");
    let (status, body) = post("/api/ci/jobs", Some(&ana), &job);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"created\":true"), "{body}");
    let (status, body) =
        post("/api/ci/jobs", Some(&ana), &json!({ "name": "x", "pipeline": "y", "agenda": 1 }));
    assert_eq!(status, 400);
    assert!(body.contains("agenda"), "{body}");
    assert_eq!(post("/api/ci/config", Some(&bia), &json!({})).0, 403);
}

#[test]
fn a_file_is_applied_to_the_folder_of_data_with_no_server() {
    let dir = std::env::temp_dir().join(format!("raven-ci-aplicar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    let path = file(&dir, "local", true);
    let done = raven(&dir.join("dados"))
        .arg("aplicar")
        .arg(&path)
        .env_remove("RAVEN_CI_SERVIDOR")
        .output()
        .expect("runs");
    assert_eq!(done.status.code(), Some(0), "{}", said(&done));
    assert!(said(&done).contains("noite/agenda: criado"), "{}", said(&done));
    let broken = dir.join("quebrado.toml");
    std::fs::write(&broken, "[[jobs]]\nname = \"a\"\nfile = \"nao-existe.toml\"\n").expect("written");
    let refused = raven(&dir.join("dados"))
        .arg("aplicar")
        .arg(&broken)
        .env_remove("RAVEN_CI_SERVIDOR")
        .output()
        .expect("runs");
    assert_eq!(refused.status.code(), Some(2));
    assert!(said(&refused).contains("jobs.a.file"), "{}", said(&refused));
    let _ = std::fs::remove_dir_all(&dir);
}
