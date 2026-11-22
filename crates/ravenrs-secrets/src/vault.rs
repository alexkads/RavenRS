//! The vault: sealing, the rights of each person, rotation and revocation.

use std::collections::BTreeMap;
use std::fmt;

use chacha20poly1305::aead::{Aead, KeyInit, OsRng, Payload, rand_core::RngCore};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{Access, SecretError};
use crate::key::KeySource;
use crate::redact::Redactor;
use crate::storage::{Sealed, SecretStorage};

/// The name of a secret, which is all that documents, exports and logs carry. It shows as
/// `secret:<id>` and holds nothing of the value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SecretRef(String);

impl SecretRef {
    /// Its id.
    pub fn id(&self) -> &str {
        &self.0
    }

    /// Reads `secret:<id>`, as [`Display`](fmt::Display) writes it.
    pub fn parse(text: &str) -> Option<Self> {
        let id = text.strip_prefix("secret:")?;
        let fine = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        fine.then(|| Self(id.to_owned()))
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "secret:{}", self.0)
    }
}

/// What may be said about a secret without its value.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SecretInfo {
    /// What it is, as `github-token`.
    pub kind: String,
    /// Raised by each rotation, from 1.
    pub version: u32,
    /// Whether it was revoked.
    pub revoked: bool,
}

/// Keeps secrets sealed in `S`, under a key from a [`KeySource`].
pub struct Vault<S: SecretStorage> {
    cipher: ChaCha20Poly1305,
    storage: S,
    redactor: Redactor,
}

impl<S: SecretStorage> fmt::Debug for Vault<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vault").field("redactor", &self.redactor).finish_non_exhaustive()
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok()).collect()
}

/// What the cipher binds a value to: changing the id, the kind or the version in the record makes
/// it fail to open.
fn bound(id: &str, kind: &str, version: u32) -> Vec<u8> {
    format!("ravenrs-secrets/1|{id}|{kind}|{version}").into_bytes()
}

impl<S: SecretStorage> Vault<S> {
    /// A vault over `storage`, with the key `source` gives.
    pub fn open(source: &dyn KeySource, storage: S) -> Result<Self, SecretError> {
        let key = source.key()?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key.as_ref()));
        Ok(Self { cipher, storage, redactor: Redactor::new() })
    }

    /// What keeps the values this vault handed out out of text. Give it to whatever writes logs.
    pub fn redactor(&self) -> Redactor {
        self.redactor.clone()
    }

    fn seal(&self, id: &str, kind: &str, version: u32, value: &str) -> Result<(String, String), SecretError> {
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let aad = bound(id, kind, version);
        let cipher = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: value.as_bytes(), aad: &aad })
            .map_err(|_| SecretError::Key("sealing failed".to_owned()))?;
        Ok((to_hex(&nonce), to_hex(&cipher)))
    }

    fn unseal(&self, id: &str, sealed: &Sealed) -> Result<Zeroizing<String>, SecretError> {
        let nonce = from_hex(&sealed.nonce).filter(|nonce| nonce.len() == 12).ok_or(SecretError::Tampered)?;
        let cipher = from_hex(&sealed.cipher).ok_or(SecretError::Tampered)?;
        let aad = bound(id, &sealed.kind, sealed.version);
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(&nonce), Payload { msg: &cipher, aad: &aad })
            .map_err(|_| SecretError::Tampered)?;
        String::from_utf8(plain).map(Zeroizing::new).map_err(|_| SecretError::Tampered)
    }

    fn load(&self, secret: &SecretRef) -> Result<Sealed, SecretError> {
        self.storage.load(secret.id())?.ok_or(SecretError::NotFound)
    }

    fn check(sealed: &Sealed, who: &str, access: Access) -> Result<(), SecretError> {
        let given = sealed.grants.get(who).is_some_and(|rights| rights.contains(&access));
        if given { Ok(()) } else { Err(SecretError::Denied(access)) }
    }

    /// Keeps `value` as a new secret of kind `kind`. `owner` may use, reveal and administer it.
    pub fn put(&self, kind: &str, value: &str, owner: &str) -> Result<SecretRef, SecretError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (nonce, cipher) = self.seal(&id, kind, 1, value)?;
        let grants =
            BTreeMap::from([(owner.to_owned(), vec![Access::Use, Access::Reveal, Access::Administer])]);
        let sealed = Sealed { kind: kind.to_owned(), version: 1, nonce, cipher, revoked: false, grants };
        self.storage.save(&id, &sealed)?;
        Ok(SecretRef(id))
    }

    /// What may be said about `secret` without its value.
    pub fn describe(&self, secret: &SecretRef) -> Result<SecretInfo, SecretError> {
        let sealed = self.load(secret)?;
        Ok(SecretInfo { kind: sealed.kind, version: sealed.version, revoked: sealed.revoked })
    }

    /// Whether `who` may do `access` with `secret`.
    pub fn may(&self, secret: &SecretRef, who: &str, access: Access) -> Result<bool, SecretError> {
        Ok(Self::check(&self.load(secret)?, who, access).is_ok())
    }

    /// Hands the value to `run`, for `who` who may use it, and keeps it out of text from now on.
    pub fn use_with<T>(
        &self,
        secret: &SecretRef,
        who: &str,
        run: impl FnOnce(&str) -> T,
    ) -> Result<T, SecretError> {
        let sealed = self.load(secret)?;
        if sealed.revoked {
            return Err(SecretError::Revoked);
        }
        Self::check(&sealed, who, Access::Use)?;
        let value = self.unseal(secret.id(), &sealed)?;
        self.redactor.add(&value);
        Ok(run(&value))
    }

    /// The value, for `who` who may reveal it. It is wiped from memory when dropped.
    pub fn reveal(&self, secret: &SecretRef, who: &str) -> Result<Zeroizing<String>, SecretError> {
        let sealed = self.load(secret)?;
        if sealed.revoked {
            return Err(SecretError::Revoked);
        }
        Self::check(&sealed, who, Access::Reveal)?;
        let value = self.unseal(secret.id(), &sealed)?;
        self.redactor.add(&value);
        Ok(value)
    }

    /// Replaces the value, for `who` who administers it, and returns the new version. The old value
    /// stays out of text, since logs from before may still hold it.
    pub fn rotate(&self, secret: &SecretRef, value: &str, who: &str) -> Result<u32, SecretError> {
        let sealed = self.load(secret)?;
        if sealed.revoked {
            return Err(SecretError::Revoked);
        }
        Self::check(&sealed, who, Access::Administer)?;
        let old = self.unseal(secret.id(), &sealed)?;
        self.redactor.add(&old);
        let version = sealed.version + 1;
        let (nonce, cipher) = self.seal(secret.id(), &sealed.kind, version, value)?;
        self.storage.save(secret.id(), &Sealed { version, nonce, cipher, ..sealed })?;
        Ok(version)
    }

    /// Revokes `secret`, for `who` who administers it: from now on using or revealing it fails
    /// with [`SecretError::Revoked`]. The record stays, so what still refers to it says why.
    pub fn revoke(&self, secret: &SecretRef, who: &str) -> Result<(), SecretError> {
        let sealed = self.load(secret)?;
        Self::check(&sealed, who, Access::Administer)?;
        self.storage.save(secret.id(), &Sealed { revoked: true, ..sealed })
    }

    /// Lets `person` do `access` with `secret`, for `who` who administers it.
    pub fn allow(
        &self,
        secret: &SecretRef,
        who: &str,
        person: &str,
        access: Access,
    ) -> Result<(), SecretError> {
        let mut sealed = self.load(secret)?;
        Self::check(&sealed, who, Access::Administer)?;
        let rights = sealed.grants.entry(person.to_owned()).or_default();
        if !rights.contains(&access) {
            rights.push(access);
            rights.sort();
        }
        self.storage.save(secret.id(), &sealed)
    }

    /// Takes `access` from `person`, for `who` who administers it. Nobody can take the last right
    /// to administer it, so a secret is never left with no one to manage it.
    pub fn deny(
        &self,
        secret: &SecretRef,
        who: &str,
        person: &str,
        access: Access,
    ) -> Result<(), SecretError> {
        let mut sealed = self.load(secret)?;
        Self::check(&sealed, who, Access::Administer)?;
        let administrators =
            sealed.grants.values().filter(|rights| rights.contains(&Access::Administer)).count();
        let theirs = sealed.grants.get(person).is_some_and(|rights| rights.contains(&Access::Administer));
        if access == Access::Administer && theirs && administrators == 1 {
            return Err(SecretError::Denied(Access::Administer));
        }
        if let Some(rights) = sealed.grants.get_mut(person) {
            rights.retain(|right| *right != access);
            if rights.is_empty() {
                sealed.grants.remove(person);
            }
        }
        self.storage.save(secret.id(), &sealed)
    }
}
