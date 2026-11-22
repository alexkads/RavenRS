//! The plugin `credentials` of Raven CI: the kinds of credential.
//!
//! # Its role
//!
//! The core keeps credentials sealed in its vault by name; what a credential holds depends on its
//! kind, and so does what a plugin gets when it asks for it and what a log must hide. This plugin
//! brings the three kinds the reference pipelines need:
//!
//! | Kind | Fields | What `Host::secret` gives | Hidden in a log |
//! |---|---|---|---|
//! | `token` | `token` | the token | the token |
//! | `user-password` | `user`, `password` | `user:password`, as `ravenrs-git` and HTTP basic take it | the password and `user:password` |
//! | `ssh-key` | `private_key`, `passphrase` | the private key | the key, each of its lines, and the passphrase |
//!
//! # How to use it
//!
//! ```text
//! echo '{"token": "ghp_…"}' | raven-ci credencial criar github --tipo token
//! ```
//!
//! and a job names it: `{"scm": {"id": "git", "credential": "github", …}}`.

use std::sync::Arc;

use raven_ci_plugin_api::{CiPlugin, CredentialKind, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest, Rule};
use serde_json::Value;

/// The plugin `credentials`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CredentialsPlugin;

impl Module for CredentialsPlugin {
    fn name(&self) -> &str {
        "credentials"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for CredentialsPlugin {
    fn summary(&self) -> String {
        "Os tipos de credencial: token, usuário e senha, e chave SSH.".to_owned()
    }

    fn credential_kinds(&self) -> Vec<Arc<dyn CredentialKind>> {
        vec![Arc::new(Token), Arc::new(UserPassword), Arc::new(SshKey)]
    }
}

fn secret(name: &str, kind: FieldKind) -> FieldInfo {
    let mut field = FieldInfo::new(name, kind);
    field.secret = true;
    field
}

fn text<'v>(credential: &'v Value, field: &str) -> &'v str {
    credential.get(field).and_then(Value::as_str).unwrap_or_default()
}

/// A token: of GitHub, a PAT of Azure DevOps, of a registry.
struct Token;

impl CredentialKind for Token {
    fn id(&self) -> &str {
        "token"
    }

    fn caption(&self) -> String {
        "Token".to_owned()
    }

    fn fields(&self) -> EntityInfo {
        EntityInfo::new("Token").field(secret("token", FieldKind::Text)).rule(Rule::required("token"))
    }

    fn hidden(&self, credential: &Value) -> Vec<String> {
        vec![text(credential, "token").to_owned()]
    }
}

/// A user and a password.
struct UserPassword;

impl CredentialKind for UserPassword {
    fn id(&self) -> &str {
        "user-password"
    }

    fn caption(&self) -> String {
        "Usuário e senha".to_owned()
    }

    fn fields(&self) -> EntityInfo {
        EntityInfo::new("UserPassword")
            .field(FieldInfo::new("user", FieldKind::Text))
            .field(secret("password", FieldKind::Text))
            .rule(Rule::required("user"))
            .rule(Rule::required("password"))
    }

    fn hidden(&self, credential: &Value) -> Vec<String> {
        vec![text(credential, "password").to_owned(), self.secret(credential)]
    }

    fn secret(&self, credential: &Value) -> String {
        format!("{}:{}", text(credential, "user"), text(credential, "password"))
    }
}

/// A private SSH key, with its passphrase when it has one.
struct SshKey;

impl CredentialKind for SshKey {
    fn id(&self) -> &str {
        "ssh-key"
    }

    fn caption(&self) -> String {
        "Chave SSH".to_owned()
    }

    fn fields(&self) -> EntityInfo {
        EntityInfo::new("SshKey")
            .field(secret("private_key", FieldKind::LongText))
            .field(secret("passphrase", FieldKind::Text))
            .rule(Rule::required("private_key"))
    }

    fn hidden(&self, credential: &Value) -> Vec<String> {
        let key = text(credential, "private_key");
        // A key leaks line by line as often as whole: each line long enough to tell is hidden too.
        let mut hidden: Vec<String> = key
            .lines()
            .map(str::trim)
            .filter(|line| line.len() >= 16 && !line.starts_with("-----"))
            .map(str::to_owned)
            .collect();
        hidden.push(key.to_owned());
        hidden.push(text(credential, "passphrase").to_owned());
        hidden
    }
}
