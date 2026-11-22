//! The center as a product (#793): its version, its news and its self-test, as the installer and
//! the MCP server read them.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

mod accessibility;
mod cabos;
mod opening;
mod tour;

#[test]
fn the_news_speak_of_the_version_running() {
    let news = ravenrs_app_update::parse_news(include_str!("../../novidades.json")).expect("the news");
    assert_eq!(news.versao, raven_feature_center::VERSION);
}

#[test]
fn the_card_of_the_tool_matches_the_package() {
    let card: serde_json::Value = serde_json::from_str(include_str!("../../tool.json")).expect("tool.json");
    assert_eq!(card["package"], "raven-feature-center");
    assert_eq!(card["version"], raven_feature_center::VERSION);
}

#[test]
fn it_opens_every_way_with_no_window() {
    let said = raven_feature_center::self_test().expect("the self-test");
    assert!(said.contains("4 modos"), "{said}");
}
mod scenarios;
