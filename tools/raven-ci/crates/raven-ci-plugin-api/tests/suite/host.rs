use std::sync::Arc;

use raven_ci_plugin_api::testing::FakeHost;
use raven_ci_plugin_api::{CiError, Host, Permission};

#[test]
fn a_plugin_with_only_disk_gets_its_folder_and_is_denied_a_secret_on_the_audit() {
    let host: Arc<FakeHost> =
        Arc::new(FakeHost::new("cache", &[Permission::Disk]).with_secret("token", "s3cr3t"));
    let dir = host.data_dir().expect("its folder");
    assert!(dir.is_dir());
    let denied = host.secret("token").expect_err("no permission");
    assert_eq!(
        denied,
        CiError::Denied {
            plugin: "cache".into(),
            permission: Permission::Secrets,
            what: "ler o segredo token".into()
        }
    );
    assert_eq!(
        denied.to_string(),
        "o plugin cache não tem a permissão ci.secrets (ler segredos), e por isso não pôde ler o segredo token"
    );
    assert_eq!(
        host.audit(),
        [
            (Permission::Disk, "usar a pasta própria".to_owned(), true),
            (Permission::Secrets, "ler o segredo token".to_owned(), false),
        ]
    );
}

#[test]
fn a_secret_granted_is_read() {
    let host = FakeHost::new("github", &[Permission::Secrets]).with_secret("token", "s3cr3t");
    assert_eq!(host.secret("token").expect("granted"), "s3cr3t");
    assert!(matches!(host.secret("other"), Err(CiError::Unavailable(_))));
    assert!(host.database().is_err(), "the database needs disk");
}
