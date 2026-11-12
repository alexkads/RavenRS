//! Packages and their sources, read from `cargo metadata`.

use std::path::Path;

#[test]
fn finds_this_package_and_its_sources() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let packages = match ravenrs_lint::packages(&manifest) {
        Ok(packages) => packages,
        Err(error) => panic!("{error}"),
    };
    let Some(this) = packages.iter().find(|p| p.name == "ravenrs-lint") else { panic!("not listed") };
    // The binary in src/bin is inside src, so src is read once.
    assert_eq!(this.source_dirs, [Path::new(env!("CARGO_MANIFEST_DIR")).join("src")]);
    assert!(!this.uses_ravenrs);
    assert!(packages.iter().any(|p| p.name == "family-finance" && p.uses_ravenrs));
}
