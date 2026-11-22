use ravenrs_secrets::{KeyFile, KeyFromEnv, KeySource, MemoryStorage, SecretError, Vault};

use crate::TOKEN;

#[test]
fn a_key_file_is_made_once_and_kept() {
    let folder = tempfile::tempdir().unwrap();
    let file = KeyFile::new(folder.path().join("chaves").join("cofre.key"));
    let first = file.key().unwrap();
    assert_eq!(*file.key().unwrap(), *first, "the same key the second time");
    let storage = MemoryStorage::new();
    let token = Vault::open(&file, storage.clone()).unwrap().put("github-token", TOKEN, "ana").unwrap();
    assert_eq!(*Vault::open(&file, storage).unwrap().reveal(&token, "ana").unwrap(), TOKEN, "after closing");
}

#[cfg(unix)]
#[test]
fn a_key_file_others_can_read_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("cofre.key");
    let file = KeyFile::new(&path);
    file.key().unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "made readable by its owner only");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(file.key(), Err(SecretError::Key(why)) if why.contains("600")));
}

#[test]
fn a_key_in_the_environment_that_is_not_set_is_an_error() {
    // Setting a variable is unsafe in this edition, and the workspace forbids it; the 64 digits are
    // checked by the unit test of the reading itself, in src/key.rs.
    let missing = KeyFromEnv("RAVENRS_SECRETS_TESTE_NENHUMA".to_owned());
    assert!(
        matches!(missing.key(), Err(SecretError::Key(why)) if why.contains("RAVENRS_SECRETS_TESTE_NENHUMA"))
    );
}
