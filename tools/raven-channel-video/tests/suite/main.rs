//! Raven Channel Video as a product (#798): the version the installer checks and the self-test, run on
//! the binary as the installer and an agent run it.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::process::Command;

#[test]
fn versao_prints_the_version_of_the_news() {
    let out = Command::new(env!("CARGO_BIN_EXE_raven-channel-video")).arg("--versao").output().expect("the binary runs");
    assert!(out.status.success());
    let news = ravenrs_app_update::parse_news(include_str!("../../novidades.json")).expect("the news");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), news.versao);
}

#[test]
fn it_tests_itself_with_no_window() {
    let out = Command::new(env!("CARGO_BIN_EXE_raven-channel-video")).arg("--self-test").output().expect("the binary runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
