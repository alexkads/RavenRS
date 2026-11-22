use ravenrs_secrets::{
    Access, FileStorage, FixedKey, MemoryStorage, SecretError, SecretRef, SecretStorage, Vault,
};

use crate::{TOKEN, vault};

#[test]
fn the_file_holds_no_value_and_another_key_opens_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let mine = Vault::open(&FixedKey([1; 32]), FileStorage::new(folder.path()).unwrap()).unwrap();
    let token = mine.put("github-token", TOKEN, "ana").unwrap();
    let file = std::fs::read_to_string(folder.path().join(format!("{}.json", token.id()))).unwrap();
    assert!(!file.contains(TOKEN), "the value is not in the file");
    assert!(!file.contains("umToken"), "not even a piece of it");
    assert_eq!(*mine.reveal(&token, "ana").unwrap(), TOKEN);

    let other = Vault::open(&FixedKey([2; 32]), FileStorage::new(folder.path()).unwrap()).unwrap();
    assert_eq!(other.reveal(&token, "ana").unwrap_err(), SecretError::Tampered);
}

#[test]
fn changing_the_kind_or_the_version_in_the_record_breaks_it() {
    let storage = MemoryStorage::new();
    let vault = Vault::open(&FixedKey([1; 32]), storage.clone()).unwrap();
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    let kept = storage.load(token.id()).unwrap().unwrap();

    let mut kind = kept.clone();
    kind.kind = "outro-tipo".to_owned();
    storage.save(token.id(), &kind).unwrap();
    assert_eq!(vault.reveal(&token, "ana").unwrap_err(), SecretError::Tampered);

    let mut version = kept;
    version.version = 2;
    storage.save(token.id(), &version).unwrap();
    assert_eq!(vault.reveal(&token, "ana").unwrap_err(), SecretError::Tampered);
}

#[test]
fn using_is_not_seeing() {
    let vault = vault(1);
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    vault.allow(&token, "ana", "agente-ci", Access::Use).unwrap();
    let length = vault.use_with(&token, "agente-ci", |value| value.len()).unwrap();
    assert_eq!(length, TOKEN.len(), "the agent can use it");
    assert_eq!(vault.reveal(&token, "agente-ci").unwrap_err(), SecretError::Denied(Access::Reveal));
    assert_eq!(vault.use_with(&token, "bia", |_| ()).unwrap_err(), SecretError::Denied(Access::Use));
    assert!(vault.may(&token, "agente-ci", Access::Use).unwrap());
    assert!(!vault.may(&token, "agente-ci", Access::Administer).unwrap());
}

#[test]
fn administering_is_not_seeing_either() {
    let vault = vault(1);
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    vault.allow(&token, "ana", "bia", Access::Administer).unwrap();
    assert_eq!(vault.reveal(&token, "bia").unwrap_err(), SecretError::Denied(Access::Reveal));
    assert_eq!(vault.rotate(&token, "ghp_outroTokenDeTeste456", "bia").unwrap(), 2, "but she can rotate it");
}

#[test]
fn a_revoked_secret_fails_the_next_use() {
    let vault = vault(1);
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    vault.use_with(&token, "ana", |_| ()).unwrap();
    vault.revoke(&token, "ana").unwrap();
    assert_eq!(vault.use_with(&token, "ana", |_| ()).unwrap_err(), SecretError::Revoked);
    assert_eq!(vault.reveal(&token, "ana").unwrap_err(), SecretError::Revoked);
    assert!(vault.describe(&token).unwrap().revoked, "what refers to it can say why");
}

#[test]
fn rotation_raises_the_version_and_the_new_value_holds() {
    let vault = vault(1);
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    assert_eq!(vault.describe(&token).unwrap().version, 1);
    let new = "ghp_tokenNovoDepoisDaRotacao789";
    assert_eq!(vault.rotate(&token, new, "ana").unwrap(), 2);
    assert_eq!(*vault.reveal(&token, "ana").unwrap(), new);
    let log = vault.redactor().redact(&format!("antes {TOKEN}, depois {new}"));
    assert!(!log.contains(TOKEN) && !log.contains(new), "both stay out: {log}");
}

#[test]
fn only_an_administrator_manages_and_the_last_one_stays() {
    let vault = vault(1);
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    assert_eq!(
        vault.allow(&token, "bia", "bia", Access::Reveal).unwrap_err(),
        SecretError::Denied(Access::Administer)
    );
    assert_eq!(vault.revoke(&token, "bia").unwrap_err(), SecretError::Denied(Access::Administer));
    assert_eq!(
        vault.deny(&token, "ana", "ana", Access::Administer).unwrap_err(),
        SecretError::Denied(Access::Administer),
        "a secret is never left with nobody to manage it"
    );
    vault.allow(&token, "ana", "bia", Access::Use).unwrap();
    vault.deny(&token, "ana", "bia", Access::Use).unwrap();
    assert_eq!(vault.use_with(&token, "bia", |_| ()).unwrap_err(), SecretError::Denied(Access::Use));
}

#[test]
fn the_reference_holds_nothing_of_the_value() {
    let vault = vault(1);
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    let written = token.to_string();
    assert!(written.starts_with("secret:") && !written.contains(TOKEN));
    assert_eq!(SecretRef::parse(&written), Some(token.clone()));
    assert_eq!(SecretRef::parse("secret:../../etc/passwd"), None, "a reference cannot name a path");
    let exported = serde_json::to_string(&serde_json::json!({ "credential": token })).unwrap();
    assert!(!exported.contains(TOKEN), "{exported}");
    assert!(!format!("{vault:?}").contains(TOKEN));
    assert_eq!(
        vault.describe(&SecretRef::parse("secret:nao-existe").unwrap()).unwrap_err(),
        SecretError::NotFound
    );
}
