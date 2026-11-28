//! The plugin `email` (#912) on builds run for real in the core: who hears what, when, and what a
//! job said wrong. The messages are kept by a mailer of the test instead of sent.
#![allow(clippy::expect_used, clippy::panic, reason = "tests fail loudly")]

use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus};
use raven_ci_plugin_email::EmailPlugin;
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use ravenrs_mail::{Mail, Mailer};
use serde_json::{Value, json};

#[derive(Default)]
struct Kept(Mutex<Vec<Mail>>);

impl Mailer for Kept {
    fn send(&self, mail: &Mail) -> Result<(), String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push(mail.clone());
        Ok(())
    }
}

impl Kept {
    fn subjects(&self) -> Vec<String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).iter().map(|m| m.subject.clone()).collect()
    }

    fn last(&self) -> Mail {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).last().cloned().expect("a message")
    }
}

fn ci(kept: &Arc<Kept>) -> Ci {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(Arc::new(EmailPlugin::with_mailer(kept.clone())));
    ci.apply_plugins().expect("on");
    ci
}

fn script(passes: bool) -> String {
    let command = match (passes, cfg!(windows)) {
        (true, _) => "echo ok",
        (false, false) => "echo quebrou >&2; exit 3",
        (false, true) => "echo quebrou 1>&2 & exit /b 3",
    };
    format!("[[stages]]\nname = \"Teste\"\nsteps = ['{command}']\n")
}

/// Runs one build of `job` with `notify`; how it ended and the log of the core.
fn build(ci: &Ci, job: &str, passes: bool, notify: Value) -> (BuildStatus, Vec<String>) {
    let spec =
        JobSpec::new(job, "toml", script(passes)).every_request().settings(json!({ "notify": [notify] }));
    ci.save_job(spec).expect("saved");
    let number = ci.request_build(BuildRequest::new(job, "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    let log = ci.build_log(job, number, 0).expect("log").into_iter().map(|l| l.text).collect();
    (ci.build(job, number).expect("build").status, log)
}

#[test]
fn a_build_that_fails_sends_one_message_with_why_and_one_that_passes_sends_none() {
    let kept = Arc::new(Kept::default());
    let ci = ci(&kept);
    let notify = json!({ "id": "email", "to": ["Ana <ana@example.com>"] });
    let (status, log) = build(&ci, "apps/site", false, notify.clone());
    assert_eq!(status, BuildStatus::Failed, "{log:#?}");
    assert_eq!(kept.subjects(), ["[Raven CI] apps/site #1: falhou"]);
    let mail = kept.last();
    assert_eq!(mail.to, ["Ana <ana@example.com>"]);
    assert!(mail.body.contains("O build 1 do job apps/site falhou."), "{}", mail.body);
    assert!(
        mail.body.contains("Motivo: o passo e1-1 falhou: o comando terminou com o código 3"),
        "{}",
        mail.body
    );
    assert!(mail.body.contains("Duração: "), "{}", mail.body);
    build(&ci, "apps/site", true, notify);
    assert_eq!(kept.subjects().len(), 1, "a build that passed says nothing");
}

#[test]
fn on_change_it_says_the_break_once_and_that_it_passed_again() {
    let kept = Arc::new(Kept::default());
    let ci = ci(&kept);
    let notify = json!({ "id": "email", "to": "ana@example.com", "when": "mudanca" });
    build(&ci, "site", true, notify.clone());
    build(&ci, "site", false, notify.clone());
    build(&ci, "site", false, notify.clone());
    build(&ci, "site", true, notify.clone());
    build(&ci, "site", true, notify);
    assert_eq!(kept.subjects(), ["[Raven CI] site #2: falhou", "[Raven CI] site #4: voltou a passar"]);
}

#[test]
fn always_tells_every_build() {
    let kept = Arc::new(Kept::default());
    let ci = ci(&kept);
    let notify = json!({ "id": "email", "to": ["ana@example.com"], "when": "sempre" });
    build(&ci, "site", true, notify.clone());
    build(&ci, "site", false, notify);
    assert_eq!(kept.subjects(), ["[Raven CI] site #1: passou", "[Raven CI] site #2: falhou"]);
}

#[test]
fn what_a_job_says_wrong_is_in_the_log_and_the_build_goes_on() {
    let kept = Arc::new(Kept::default());
    let ci = ci(&kept);
    for (notify, field) in [
        (json!({ "id": "email" }), "to"),
        (json!({ "id": "email", "to": ["ana"] }), "'ana' is not a valid email address"),
        (json!({ "id": "email", "to": ["ana@example.com"], "when": "as vezes" }), "as vezes não existe"),
    ] {
        let (status, log) = build(&ci, "site", false, notify);
        assert_eq!(status, BuildStatus::Failed, "the build ends as its steps say");
        assert!(log.iter().any(|l| l.contains("aviso email") && l.contains(field)), "{field}: {log:#?}");
    }
    assert!(kept.subjects().is_empty());
}

#[test]
fn with_no_mailer_given_the_settings_of_the_server_are_asked_for() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.add_plugin(Arc::new(EmailPlugin::new()));
    ci.apply_plugins().expect("on");
    let (_, log) = build(&ci, "site", false, json!({ "id": "email", "to": ["ana@example.com"] }));
    assert!(log.iter().any(|l| l.contains("aviso email") && l.contains("host")), "{log:#?}");
}
