//! Where the key comes from. It never sits with the sealed records: someone who copies those gets
//! nothing they can read.

use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{OsRng, rand_core::RngCore};
use zeroize::Zeroizing;

use crate::vault::SecretError;

/// The length of a key, in bytes.
pub const KEY_LEN: usize = 32;

/// Gives the key of a vault.
pub trait KeySource {
    /// The key, made the first time when the source can keep it.
    fn key(&self) -> Result<Zeroizing<[u8; KEY_LEN]>, SecretError>;
}

fn fresh() -> Zeroizing<[u8; KEY_LEN]> {
    let mut key = Zeroizing::new([0_u8; KEY_LEN]);
    OsRng.fill_bytes(key.as_mut());
    key
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Reads 64 hexadecimal digits as a key.
fn from_hex(text: &str) -> Result<Zeroizing<[u8; KEY_LEN]>, SecretError> {
    let text = text.trim();
    let wrong = || SecretError::Key(format!("a key is {} hexadecimal digits", KEY_LEN * 2));
    if text.len() != KEY_LEN * 2 || !text.is_ascii() {
        return Err(wrong());
    }
    let mut key = Zeroizing::new([0_u8; KEY_LEN]);
    for (index, byte) in key.iter_mut().enumerate() {
        let pair = text.get(index * 2..index * 2 + 2).ok_or_else(wrong)?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| wrong())?;
    }
    Ok(key)
}

/// A key given as it is, for tests.
#[derive(Clone)]
pub struct FixedKey(pub [u8; KEY_LEN]);

impl std::fmt::Debug for FixedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FixedKey(•••)")
    }
}

impl KeySource for FixedKey {
    fn key(&self) -> Result<Zeroizing<[u8; KEY_LEN]>, SecretError> {
        Ok(Zeroizing::new(self.0))
    }
}

/// A key in an environment variable, as 64 hexadecimal digits: for a server, where the variable
/// comes from what starts it (a systemd credential, a container secret).
#[derive(Clone, Debug)]
pub struct KeyFromEnv(pub String);

impl KeySource for KeyFromEnv {
    fn key(&self) -> Result<Zeroizing<[u8; KEY_LEN]>, SecretError> {
        let text = Zeroizing::new(
            std::env::var(&self.0)
                .map_err(|_| SecretError::Key(format!("the variable {} is not set", self.0)))?,
        );
        from_hex(&text)
    }
}

/// A key in a file only its owner reads, made the first time with a random key. On Linux and macOS a
/// file that others may read is refused, since then the key is not kept apart.
#[derive(Clone, Debug)]
pub struct KeyFile(pub PathBuf);

impl KeyFile {
    /// The key in this file.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

#[cfg(unix)]
fn only_mine(path: &Path) -> Result<(), SecretError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).map_err(|e| SecretError::Key(e.to_string()))?.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(SecretError::Key(format!(
            "{} can be read by others ({:o}); it has to be 600",
            path.display(),
            mode & 0o777
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn only_mine(_path: &Path) -> Result<(), SecretError> {
    // On Windows a file under the user's profile is the user's; its ACL is not checked here.
    Ok(())
}

#[cfg(unix)]
fn write_mine(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    file.write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_mine(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(text.as_bytes())
}

impl KeySource for KeyFile {
    fn key(&self) -> Result<Zeroizing<[u8; KEY_LEN]>, SecretError> {
        let path = &self.0;
        if !path.exists() {
            if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty()) {
                std::fs::create_dir_all(folder).map_err(|e| SecretError::Key(e.to_string()))?;
            }
            let key = fresh();
            let text = Zeroizing::new(to_hex(key.as_ref()));
            write_mine(path, &text).map_err(|e| SecretError::Key(e.to_string()))?;
            return Ok(key);
        }
        only_mine(path)?;
        let text =
            Zeroizing::new(std::fs::read_to_string(path).map_err(|e| SecretError::Key(e.to_string()))?);
        from_hex(&text)
    }
}

/// The key in the system's own store (Keychain on macOS, Credential Manager on Windows, Secret
/// Service on Linux), made the first time. Asking for it may show the system's prompt.
#[cfg(feature = "system-keyring")]
#[derive(Clone, Debug)]
pub struct SystemKeyring {
    /// Who it belongs to, as the app's identifier.
    pub service: String,
    /// Which key of that app.
    pub account: String,
}

#[cfg(feature = "system-keyring")]
impl KeySource for SystemKeyring {
    fn key(&self) -> Result<Zeroizing<[u8; KEY_LEN]>, SecretError> {
        let entry =
            keyring::Entry::new(&self.service, &self.account).map_err(|e| SecretError::Key(e.to_string()))?;
        match entry.get_password() {
            Ok(text) => from_hex(&Zeroizing::new(text)),
            Err(keyring::Error::NoEntry) => {
                let key = fresh();
                let text = Zeroizing::new(to_hex(key.as_ref()));
                entry.set_password(&text).map_err(|e| SecretError::Key(e.to_string()))?;
                Ok(key)
            }
            Err(other) => Err(SecretError::Key(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_goes_both_ways_and_refuses_what_is_not_a_key() {
        let key = fresh();
        assert_eq!(*from_hex(&to_hex(key.as_ref())).unwrap_or_else(|e| panic!("{e}")), *key);
        for wrong in ["", "abc", &"g".repeat(64), &"0".repeat(63), &"0".repeat(65)] {
            assert!(from_hex(wrong).is_err(), "{wrong}");
        }
    }
}
