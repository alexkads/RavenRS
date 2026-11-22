//! The news of each version, as every RavenRS app shows them (#744): the version at the foot of
//! the window opens them on the version running.

use ravenrs::prelude::*;
use ravenrs_e2e::E2e;

#[test]
fn the_version_in_the_status_bar_opens_the_news_of_the_version_running() {
    let mut studio =
        E2e::new(raven_test_studio::app(), Theme::raven_dark()).viewport(Size::new(1440.0, 900.0), 1.0);
    let version = format!("Raven Test Studio · versão {}", raven_test_studio::VERSION);
    studio.by_text(version).click().expect("the version is a button");
    let screen = studio.snapshot();
    for said in ["Novidades", "Instalada", "Quem contribuiu", "Issues resolvidas"] {
        assert!(screen.contains(said), "{said}:\n{screen}");
    }
    // The issues this version closed, as its news say them.
    let news = ravenrs_app_update::parse_news(include_str!("../../novidades.json")).expect("the news");
    assert_eq!(news.versao, raven_test_studio::VERSION);
    for issue in news.issues {
        assert!(screen.contains(&format!("#{issue}")), "#{issue}:\n{screen}");
    }
    studio.press(ravenrs::core::Key::Escape);
    assert!(!studio.snapshot().contains("Instalada"));
}
