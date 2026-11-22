//! A vault for RavenRS apps and servers.
//!
//! A secret (a token, a password, a key of another service) is sealed with ChaCha20-Poly1305 under a
//! key that never sits where the sealed records do: in a file only its owner reads, in the
//! environment, or in the system's own store. The rest of the program carries a [`SecretRef`], an
//! opaque name, so documents, exports and logs never hold the value.
//!
//! * [`Vault`] keeps secrets: [`Vault::put`], [`Vault::rotate`], [`Vault::revoke`], and the
//!   rights of each person with [`Vault::allow`] and [`Vault::deny`].
//! * [`Access`] separates using a secret ([`Vault::use_with`], which hands it to a closure) from
//!   seeing it ([`Vault::reveal`]) and from managing it. Managing does not let anyone see it.
//! * [`KeySource`] says where the key comes from: [`KeyFile`], [`KeyFromEnv`], [`FixedKey`] for
//!   tests, and `SystemKeyring` with the feature `system-keyring`.
//! * [`SecretStorage`] says where the sealed records go: [`MemoryStorage`] or [`FileStorage`].
//! * [`Redactor`] and [`redact`] keep values out of what gets written down: every value a vault
//!   hands out is taken out of the text it is given, the same way everywhere.

mod error;
mod key;
mod redact;
mod storage;
mod vault;

pub use error::{Access, SecretError};
#[cfg(feature = "system-keyring")]
pub use key::SystemKeyring;
pub use key::{FixedKey, KEY_LEN, KeyFile, KeyFromEnv, KeySource};
pub use redact::{MARK, Redactor, redact};
pub use storage::{FileStorage, MemoryStorage, Sealed, SecretStorage};
pub use vault::{SecretInfo, SecretRef, Vault};
