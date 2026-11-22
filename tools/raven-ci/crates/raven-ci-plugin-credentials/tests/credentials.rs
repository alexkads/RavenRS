//! Credentials (#876): made by name in the vault of a core on disk, read by a plugin with
//! `Host::secret`, hidden in the log of a build, never in clear on disk, and refused when their
//! fields do not fit.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use raven_ci_core::{Ci, JobSpec};
use raven_ci_plugin_api::{BuildRequest, BuildStatus, CiError, CiPlugin, Host, Permission};
use raven_ci_plugin_credentials::CredentialsPlugin;
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use ravenrs_app::{Module, ModuleManifest};
use serde_json::json;

/// A plugin that reads the credential `deploy` when it starts, as the git plugin reads its own.
struct Reader(Arc<Mutex<Option<String>>>);

impl Module for Reader {
    fn name(&self) -> &str {
        "leitor"
    }
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version("1.0.0")
            .unwrap_or_default()
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for Reader {
    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        *self.0.lock().expect("lock") = Some(host.secret("deploy")?);
        Ok(())
    }
}

struct Disk(PathBuf);

impl Drop for Disk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ci(name: &str) -> (Disk, Ci) {
    let dir = std::env::temp_dir().join(format!("raven-ci-credenciais-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ci = Ci::open(&dir).expect("a core on disk");
    ci.add_plugin(Arc::new(CredentialsPlugin));
    ci.add_plugin(Arc::new(TomlFormatPlugin));
    ci.add_plugin(Arc::new(ShellPlugin::new()));
    ci.apply_plugins().expect("on");
    (Disk(dir), ci)
}

/// Every byte of every file under `dir`, as text where it can be.
fn everything_in(dir: &Path) -> String {
    let mut all = String::new();
    for entry in std::fs::read_dir(dir).expect("a folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            all.push_str(&everything_in(&path));
        } else if let Ok(bytes) = std::fs::read(&path) {
            all.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    all
}

#[test]
fn a_token_is_sealed_listed_with_no_value_and_never_in_clear_on_disk() {
    let (disk, ci) = ci("disco");
    ci.save_credential("github", "token", &json!({ "token": "ghp_segredo_123456789" }), Some("ana"))
        .expect("saved");
    let listed = ci.credentials().expect("listed");
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].name.as_str(), listed[0].kind.as_str()), ("github", "token"));
    assert_eq!(listed[0].created_by.as_deref(), Some("ana"));
    assert!(!everything_in(&disk.0).contains("ghp_segredo_123456789"), "the token is in clear somewhere");
    // Saved again, it is rotated: still one, with the new value.
    ci.save_credential("github", "token", &json!({ "token": "ghp_outro_987654321" }), None).expect("rotated");
    assert_eq!(ci.credentials().expect("listed").len(), 1);
    ci.delete_credential("github").expect("deleted");
    assert!(ci.credentials().expect("listed").is_empty());
}

#[test]
fn a_credential_used_in_a_step_is_hidden_in_the_log_with_what_its_kind_derives() {
    let (_disk, ci) = ci("log");
    ci.save_credential(
        "deploy",
        "user-password",
        &json!({ "user": "robo", "password": "senha-forte-42" }),
        None,
    )
    .expect("saved");
    // A plugin reads it; then the log hides it, and `user:password` too.
    let pipeline = if cfg!(windows) {
        "[[stages]]\nname = \"T\"\nsteps = ['echo robo:senha-forte-42 e senha-forte-42']\n"
    } else {
        "[[stages]]\nname = \"T\"\nsteps = ['echo \"robo:senha-forte-42 e senha-forte-42\"']\n"
    };
    ci.save_job(JobSpec::new("site", "toml", pipeline)).expect("saved");
    let read = Arc::new(Mutex::new(None));
    ci.add_plugin(Arc::new(Reader(read.clone())));
    ci.apply_plugins().expect("the reader starts");
    assert_eq!(read.lock().expect("lock").as_deref(), Some("robo:senha-forte-42"));
    let n = ci.request_build(BuildRequest::new("site", "teste")).expect("asked").number;
    ci.work_until_idle("e1").expect("runs");
    assert_eq!(ci.build("site", n).expect("build").status, BuildStatus::Succeeded);
    let log =
        ci.build_log("site", n, 0).expect("log").into_iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
    assert!(log.contains("**** e ****"), "{log}");
    assert!(!log.contains("senha-forte-42"), "{log}");
}

#[test]
fn fields_that_do_not_fit_the_kind_and_a_kind_no_plugin_brings_are_refused() {
    let (_disk, ci) = ci("recusa");
    let missing =
        ci.save_credential("x", "user-password", &json!({ "user": "robo" }), None).expect_err("no password");
    assert!(missing.to_string().contains("password"), "{missing}");
    let unknown =
        ci.save_credential("x", "token", &json!({ "token": "t", "cor": "azul" }), None).expect_err("a field");
    assert!(unknown.to_string().contains("cor"), "{unknown}");
    let kind = ci.save_credential("x", "kubeconfig", &json!({}), None).expect_err("no such kind");
    assert!(kind.to_string().contains("kubeconfig"), "{kind}");
    let name =
        ci.save_credential("../fora", "token", &json!({ "token": "t-123456" }), None).expect_err("a name");
    assert!(name.to_string().contains("não serve"), "{name}");
}
