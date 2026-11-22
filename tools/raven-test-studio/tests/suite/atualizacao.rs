//! What `--atualizacao` answers, the pane among those who ask, from what the updater found:
//! a newer version, the same one, or no answer from the network.

use raven_test_studio::atualizacao::Answer;
use ravenrs_app_update::{Decision, InstallKind, parse_news};

fn news(version: &str) -> ravenrs_app_update::News {
    parse_news(&format!(
        r#"{{"versao":"{version}","titulo":"Fica ao lado do relógio","novidades":["Tray"],"por_que_atualizar":"p","issues":[641]}}"#
    ))
    .expect("news")
}

#[test]
fn a_newer_version_comes_with_its_news_and_how_to_update() {
    let n = news("9.9.9");
    let answer =
        Answer::of(Ok((n.clone(), Decision::Announce { news: n, start: false })), InstallKind::Installed);
    let json = answer.json();
    assert_eq!(json["newer"], true);
    assert_eq!(json["latest"], "9.9.9");
    assert_eq!(json["installed"], raven_test_studio::VERSION);
    assert_eq!(json["news"]["title"], "Fica ao lado do relógio");
    assert_eq!(json["news"]["items"][0], "Tray");
    assert_eq!(json["updates_itself"], true);
    assert!(answer.text().contains("--atualizar"), "{}", answer.text());
}

#[test]
fn the_same_version_is_no_news_and_a_checkout_updates_with_git() {
    let answer =
        Answer::of(Ok((news(raven_test_studio::VERSION), Decision::Nothing)), InstallKind::Development);
    let json = answer.json();
    assert_eq!(json["newer"], false);
    assert!(json["news"].is_null());
    assert_eq!(json["updates_itself"], false);
    assert!(answer.text().contains("é a versão mais nova"));

    let n = news("9.9.9");
    let newer =
        Answer::of(Ok((n.clone(), Decision::Announce { news: n, start: false })), InstallKind::Development);
    assert!(newer.text().contains("git pull"), "{}", newer.text());
}

#[test]
fn no_answer_from_the_network_says_why() {
    let answer = Answer::of(Err("sem rede".into()), InstallKind::Installed);
    assert_eq!(answer.json()["error"], "sem rede");
    assert_eq!(answer.json()["newer"], false);
    assert!(answer.text().contains("sem rede"));
}
