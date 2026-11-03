//! Keeping passwords so that what is kept does not give them away.

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

/// The longest password taken, in bytes, so a huge one cannot be used to keep the machine busy.
const MOST_BYTES: usize = 1024;

/// What is kept for a password: Argon2id, with its parameters and a salt of its own, in the usual
/// text form. A new v4 UUID is 122 random bits from the system.
pub fn hash_password(password: &str) -> Result<String, String> {
    if password.len() > MOST_BYTES {
        return Err("the password is too long".to_owned());
    }
    let salt = SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes()).map_err(|e| e.to_string())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}

/// Whether this password is the one this was made from. Something unreadable matches nothing.
pub fn verify_password(password: &str, hash: &str) -> bool {
    if password.len() > MOST_BYTES {
        return false;
    }
    PasswordHash::new(hash)
        .is_ok_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}
