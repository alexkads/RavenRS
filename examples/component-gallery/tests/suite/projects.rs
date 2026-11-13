//! Every example, downloaded as a project from the playground, builds with Cargo.
//!
//! It runs Cargo once over all of them, against the crates of this checkout, so it takes a few
//! minutes: `cargo test -p component-gallery --test suite projects:: -- --ignored`, which
//! `scripts/check.sh --full` runs.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::Path;
use std::process::Command;

use component_gallery::samples::{NAMES, sample_code, sample_project};

#[test]
#[ignore = "builds every example as a project of its own; scripts/check.sh --full runs it"]
fn every_example_downloads_as_a_project_that_builds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let place = Path::new(env!("CARGO_TARGET_TMPDIR")).join("downloaded-projects");
    let _ = std::fs::remove_dir_all(&place);
    let mut members = Vec::new();
    for name in NAMES {
        let code = sample_code(name).unwrap_or_else(|| panic!("{name} has no code"));
        let project = sample_project(name, &code).local_crates(&root);
        let folder = place.join(project.crate_name());
        for file in project.files().unwrap_or_else(|e| panic!("{name}: {e}")) {
            let path = folder.join(&file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file.contents).unwrap();
        }
        members.push(format!("{:?}", project.crate_name()));
    }
    std::fs::write(
        place.join("Cargo.toml"),
        format!("[workspace]\nresolver = \"3\"\nmembers = [{}]\n", members.join(", ")),
    )
    .unwrap();
    // The versions this checkout was tested with, so nothing is fetched.
    std::fs::copy(root.join("Cargo.lock"), place.join("Cargo.lock")).unwrap();
    let target = std::env::var("CARGO_TARGET_DIR").map_or_else(|_| root.join("target"), Into::into);
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["check", "--workspace", "--all-targets", "--offline", "--quiet", "--keep-going"])
        .current_dir(&place)
        .env("CARGO_TARGET_DIR", Path::new(&target).join("downloaded-projects"))
        .status()
        .expect("cargo runs");
    assert!(status.success(), "a downloaded project does not build; see above, under {}", place.display());
}
