//! The login of a registry: the kind of credential `registry`, and the configuration of a
//! moment the engine reads it from.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use raven_ci_plugin_api::{BuildScope, CredentialKind};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Rule};
use ravenrs_flows::NodeError;
use serde_json::{Value, json};

use crate::engine::Engine;

/// Where Docker Hub's logins are kept in a `config.json`.
pub const DOCKER_HUB: &str = "https://index.docker.io/v1/";

/// The kind of credential `registry`: the server, the user and the password (or token) of a
/// registry of images.
#[derive(Clone, Copy, Debug, Default)]
pub struct Registry;

fn text<'v>(credential: &'v Value, field: &str) -> &'v str {
    credential.get(field).and_then(Value::as_str).unwrap_or_default()
}

fn auth(user: &str, password: &str) -> String {
    STANDARD.encode(format!("{user}:{password}"))
}

impl CredentialKind for Registry {
    fn id(&self) -> &str {
        "registry"
    }

    fn caption(&self) -> String {
        "Registro de imagens".to_owned()
    }

    fn fields(&self) -> EntityInfo {
        let mut password = FieldInfo::new("password", FieldKind::Text);
        password.secret = true;
        EntityInfo::new("Registry")
            .field(FieldInfo::new("server", FieldKind::Text))
            .field(FieldInfo::new("user", FieldKind::Text))
            .field(password)
            .rule(Rule::required("user"))
            .rule(Rule::required("password"))
    }

    fn hidden(&self, credential: &Value) -> Vec<String> {
        let (user, password) = (text(credential, "user"), text(credential, "password"));
        vec![password.to_owned(), auth(user, password), format!("{user}:{password}"), self.secret(credential)]
    }

    /// The three fields as a JSON object: the step needs the server and the user too.
    fn secret(&self, credential: &Value) -> String {
        json!({
            "server": text(credential, "server"),
            "user": text(credential, "user"),
            "password": text(credential, "password"),
        })
        .to_string()
    }
}

/// A login read from a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login {
    /// The server, as a `config.json` names it; empty when the credential names none.
    pub server: String,
    /// The user.
    pub user: String,
    /// The password or token.
    pub password: String,
}

impl Login {
    /// What `Host::secret` gave for a credential: the JSON of [`Registry`], or `user:password`
    /// of a credential of kind `user-password`.
    pub fn read(secret: &str) -> Result<Self, NodeError> {
        if let Ok(value) = serde_json::from_str::<Value>(secret)
            && value.is_object()
        {
            return Ok(Self {
                server: text(&value, "server").to_owned(),
                user: text(&value, "user").to_owned(),
                password: text(&value, "password").to_owned(),
            });
        }
        match secret.split_once(':') {
            Some((user, password)) => {
                Ok(Self { server: String::new(), user: user.to_owned(), password: password.to_owned() })
            }
            None => {
                Err(NodeError::new("a credencial do registro precisa ser do tipo registry ou user-password"))
            }
        }
    }
}

/// The registry an image is in: its first part when it names a host (`ghcr.io/a/b`,
/// `localhost:5000/b`), otherwise Docker Hub.
pub fn server_of(image: &str) -> String {
    match image.split_once('/') {
        Some((first, _)) if first.contains('.') || first.contains(':') || first == "localhost" => {
            first.to_owned()
        }
        _ => DOCKER_HUB.to_owned(),
    }
}

/// A folder of configuration of the engine made for one step, with the login in it, removed when
/// dropped.
#[derive(Debug)]
pub struct Config {
    dir: PathBuf,
}

impl Config {
    /// The file of logins in it.
    pub fn file(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    /// The arguments and variables that make `engine` read this folder: `--config` for Docker,
    /// `REGISTRY_AUTH_FILE` for Podman.
    pub fn apply(&self, engine: &Engine, spec: ravenrs_process::Spec) -> ravenrs_process::Spec {
        if engine.podman { spec.env("REGISTRY_AUTH_FILE", self.file().display().to_string()) } else { spec }
    }

    /// The arguments before the subcommand: `--config <dir>` for Docker.
    pub fn global_args(&self, engine: &Engine) -> Vec<String> {
        if engine.podman { Vec::new() } else { vec!["--config".to_owned(), self.dir.display().to_string()] }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The folder Docker keeps its configuration in: `DOCKER_CONFIG`, or `.docker` in the home.
fn docker_home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DOCKER_CONFIG") {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".docker"))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A configuration with `login` for `server`, outside the build's folder, readable only by its
/// owner. The contexts and the current context of the machine's configuration come along, so
/// the engine is the same one the other steps use; nothing of it is changed.
pub fn config_for(build: &BuildScope, login: &Login, server: &str) -> Result<Config, NodeError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "raven-ci-registro-{}-{}-{}",
        std::process::id(),
        crate::engine::slug(&build.context().run),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let made =
        |e: std::io::Error| NodeError::new(format!("a configuração do registro em {}: {e}", dir.display()));
    std::fs::create_dir_all(&dir).map_err(made)?;
    let config = Config { dir: dir.clone() };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(made)?;
    }
    let mut body = json!({ "auths": { server: { "auth": auth(&login.user, &login.password) } } });
    if let Some(home) = docker_home() {
        let current = std::fs::read_to_string(home.join("config.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| value.get("currentContext").cloned());
        if let Some(current) = current {
            body["currentContext"] = current;
        }
        if home.join("contexts").is_dir() {
            copy_dir(&home.join("contexts"), &dir.join("contexts")).map_err(made)?;
        }
    }
    std::fs::write(config.file(), body.to_string()).map_err(made)?;
    build.hide(&login.password);
    build.hide(&auth(&login.user, &login.password));
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_of_an_image_is_its_host_or_docker_hub() {
        assert_eq!(server_of("ghcr.io/alexkads/site:1"), "ghcr.io");
        assert_eq!(server_of("localhost:5000/site"), "localhost:5000");
        assert_eq!(server_of("localhost/site"), "localhost");
        assert_eq!(server_of("alexkads/site"), DOCKER_HUB);
        assert_eq!(server_of("rust:1"), DOCKER_HUB);
    }

    #[test]
    fn the_log_hides_the_password_and_the_auth_of_a_registry() {
        let credential = json!({ "server": "ghcr.io", "user": "eu", "password": "s3nh4-longa" });
        let hidden = Registry.hidden(&credential);
        assert!(hidden.contains(&"s3nh4-longa".to_owned()));
        assert!(hidden.contains(&STANDARD.encode("eu:s3nh4-longa")));
        let login = Login::read(&Registry.secret(&credential)).expect("read");
        assert_eq!(
            (login.server.as_str(), login.user.as_str(), login.password.as_str()),
            ("ghcr.io", "eu", "s3nh4-longa")
        );
    }

    #[test]
    fn the_configuration_of_a_moment_has_the_login_hides_it_and_goes_when_dropped() {
        use std::collections::BTreeMap;
        use std::sync::Arc;

        use raven_ci_plugin_api::{BuildContext, BuildLog, MemoryLog};

        let log = Arc::new(MemoryLog::new());
        let context = BuildContext {
            job: "site".into(),
            number: 1,
            workspace: std::env::temp_dir().join("registro-teste-pasta"),
            run: "registro-teste".into(),
        };
        let build = BuildScope::new(context, log.clone(), BTreeMap::new());
        let login = Login { server: String::new(), user: "eu".into(), password: "s3nh4-do-registro".into() };
        let config = config_for(&build, &login, "ghcr.io").expect("a configuration");
        let file = config.file();
        let body: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).expect("the file")).expect("JSON");
        assert_eq!(body["auths"]["ghcr.io"]["auth"], STANDARD.encode("eu:s3nh4-do-registro"));
        assert!(!file.starts_with(&build.context().workspace), "outside the build's folder");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode =
                std::fs::metadata(file.parent().expect("its folder")).expect("meta").permissions().mode();
            assert_eq!(mode & 0o077, 0, "only its owner reads it");
        }
        log.line("x", "a senha s3nh4-do-registro");
        assert_eq!(log.texts(), ["a senha ****"]);
        drop(config);
        assert!(!file.exists(), "the configuration outlived the step");
    }

    #[test]
    fn a_user_password_credential_is_a_login_too() {
        let login = Login::read("eu:tok").expect("read");
        assert_eq!((login.server.as_str(), login.user.as_str(), login.password.as_str()), ("", "eu", "tok"));
        assert!(Login::read("so-um-token").is_err());
    }
}
