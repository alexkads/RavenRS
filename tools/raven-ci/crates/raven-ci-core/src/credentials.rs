//! Credentials with a name, sealed in a vault: what a job names (`"credential": "github"`) and a
//! plugin reads with `Host::secret`.
//!
//! # How it works
//!
//! - **The vault** is `ravenrs-secrets`: ChaCha20-Poly1305 records in `cofre/` in the folder of
//!   data, under a key in `cofre.chave` (only its owner reads it) or, when it is set, in the
//!   variable `RAVEN_CI_CHAVE` (64 hexadecimal digits), as a service manager hands secrets over.
//! - **The index** is the table `ci_credentials` of the core's file: the name, the kind, who made
//!   it and when, and the id of its record in the vault. No value ever goes there.
//! - **What is sealed** for a credential is what was filled in its form, with what its
//!   [`CredentialKind`](raven_ci_plugin_api::CredentialKind) says a plugin gets (`secret`) and a
//!   log hides (`hidden`), worked out when it was saved, so reading it needs no plugin.
//! - **Saving a name again** rotates its record: the old value stays out of every log.
//! - **Deleting** revokes the record and forgets the name.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::CiError;
use ravenrs_secrets::{FileStorage, KeyFile, KeyFromEnv, KeySource, SecretRef, Vault};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::host::{Secrets, ServerLog};
use crate::store::{Store, db};

/// The folder of the vault's records, in the folder of data.
pub const VAULT_DIR: &str = "cofre";
/// The file of the vault's key, in the folder of data.
pub const KEY_FILE: &str = "cofre.chave";
/// The variable that holds the key instead, as 64 hexadecimal digits.
pub const KEY_VARIABLE: &str = "RAVEN_CI_CHAVE";
/// Who the core is to the vault.
const OWNER: &str = "raven-ci";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ci_credentials (
    name TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    secret TEXT NOT NULL,
    created_by TEXT,
    created_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL
);
";

/// A credential as it is listed: never its value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CredentialInfo {
    /// What jobs name it by.
    pub name: String,
    /// Its kind, by the id of a plugin's kind: `token`.
    pub kind: String,
    /// Who made it.
    pub created_by: Option<String>,
    /// When it was made, on the core's clock.
    pub created_ms: i64,
    /// When its value last changed.
    pub updated_ms: i64,
}

/// What is sealed for one credential.
#[derive(Serialize, Deserialize)]
struct Sealed {
    fields: Value,
    secret: String,
    hidden: Vec<String>,
}

/// The vault of the core and its index.
pub struct Credentials {
    vault: Mutex<Vault<FileStorage>>,
    store: Arc<Store>,
    log: Arc<ServerLog>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials").finish_non_exhaustive()
    }
}

fn vault_error(e: impl std::fmt::Display) -> CiError {
    CiError::failed(format!("o cofre: {e}"))
}

/// Checks a name of a credential: letters, numbers, `-`, `_` and `.`.
fn checked(name: &str) -> Result<&str, CiError> {
    let fine = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if fine {
        Ok(name)
    } else {
        Err(CiError::settings("nome", format!("{name:?} não serve: use letras, números, '-', '_' e '.'")))
    }
}

impl Credentials {
    /// The vault of the folder of data `dir`, made the first time.
    pub(crate) fn open(dir: &Path, store: Arc<Store>, log: Arc<ServerLog>) -> Result<Self, CiError> {
        store.db().execute_batch(SCHEMA).map_err(db)?;
        let storage = FileStorage::new(dir.join(VAULT_DIR)).map_err(vault_error)?;
        let source: Box<dyn KeySource> = if std::env::var_os(KEY_VARIABLE).is_some() {
            Box::new(KeyFromEnv(KEY_VARIABLE.to_owned()))
        } else {
            Box::new(KeyFile::new(dir.join(KEY_FILE)))
        };
        let vault = Vault::open(source.as_ref(), storage).map_err(vault_error)?;
        Ok(Self { vault: Mutex::new(vault), store, log })
    }

    fn vault(&self) -> std::sync::MutexGuard<'_, Vault<FileStorage>> {
        self.vault.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn reference(&self, name: &str) -> Result<Option<(SecretRef, String)>, CiError> {
        let found: Option<(String, String)> = self
            .store
            .db()
            .query_row("SELECT secret, kind FROM ci_credentials WHERE name = ?1", params![name], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()
            .map_err(db)?;
        Ok(found.and_then(|(text, kind)| SecretRef::parse(&text).map(|secret| (secret, kind))))
    }

    /// Seals `fields` of the kind `kind` as `name`, with what a plugin gets (`secret`) and a log
    /// hides (`hidden`); rotates it when the name is there already.
    pub(crate) fn save(
        &self,
        name: &str,
        kind: &str,
        fields: &Value,
        secret: String,
        hidden: Vec<String>,
        by: Option<&str>,
    ) -> Result<(), CiError> {
        let name = checked(name)?;
        let sealed = serde_json::to_string(&Sealed { fields: fields.clone(), secret, hidden })
            .map_err(|e| CiError::failed(e.to_string()))?;
        let now = self.store.now_ms();
        match self.reference(name)? {
            Some((secret, was)) if was == kind => {
                self.vault().rotate(&secret, &sealed, OWNER).map_err(vault_error)?;
                self.store
                    .db()
                    .execute("UPDATE ci_credentials SET updated_ms = ?2 WHERE name = ?1", params![name, now])
                    .map_err(db)?;
            }
            Some((_, was)) => {
                return Err(CiError::settings(
                    "tipo",
                    format!("a credencial {name} é do tipo {was}; apague-a antes de criá-la como {kind}"),
                ));
            }
            None => {
                let secret = self.vault().put(kind, &sealed, OWNER).map_err(vault_error)?;
                self.store
                    .db()
                    .execute(
                        "INSERT INTO ci_credentials (name, kind, secret, created_by, created_ms, updated_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                        params![name, kind, secret.to_string(), by, now],
                    )
                    .map_err(db)?;
            }
        }
        Ok(())
    }

    /// Every credential, by name, with no value.
    pub fn list(&self) -> Result<Vec<CredentialInfo>, CiError> {
        let db = self.store.db();
        let mut stmt = db
            .prepare(
                "SELECT name, kind, created_by, created_ms, updated_ms FROM ci_credentials ORDER BY name",
            )
            .map_err(crate::store::db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CredentialInfo {
                    name: row.get(0)?,
                    kind: row.get(1)?,
                    created_by: row.get(2)?,
                    created_ms: row.get(3)?,
                    updated_ms: row.get(4)?,
                })
            })
            .map_err(crate::store::db)?;
        rows.collect::<Result<_, _>>().map_err(crate::store::db)
    }

    /// Revokes `name` and forgets it.
    pub(crate) fn delete(&self, name: &str) -> Result<(), CiError> {
        let (secret, _) = self
            .reference(name)?
            .ok_or_else(|| CiError::unavailable(format!("não há a credencial {name}")))?;
        self.vault().revoke(&secret, OWNER).map_err(vault_error)?;
        self.store.db().execute("DELETE FROM ci_credentials WHERE name = ?1", params![name]).map_err(db)?;
        Ok(())
    }

    fn sealed(&self, name: &str) -> Option<Sealed> {
        let (secret, _) = self.reference(name).ok()??;
        let text = self.vault().reveal(&secret, OWNER).ok()?;
        serde_json::from_str(&text).ok()
    }
}

impl Secrets for Credentials {
    fn get(&self, name: &str) -> Option<String> {
        let sealed = self.sealed(name)?;
        self.log.add_hidden(&sealed.secret);
        Some(sealed.secret)
    }

    fn hidden(&self, name: &str) -> Vec<String> {
        self.sealed(name).map(|sealed| sealed.hidden).unwrap_or_default()
    }
}

/// The secrets a core reads: its vault, or what it was given, as a test gives.
pub(crate) struct Either {
    pub vault: Option<Arc<Credentials>>,
    pub given: Arc<dyn Secrets>,
}

impl Secrets for Either {
    fn get(&self, name: &str) -> Option<String> {
        self.vault.as_ref().and_then(|vault| vault.get(name)).or_else(|| self.given.get(name))
    }

    fn hidden(&self, name: &str) -> Vec<String> {
        let mut hidden = self.vault.as_ref().map(|vault| vault.hidden(name)).unwrap_or_default();
        hidden.extend(self.given.hidden(name));
        hidden
    }
}
