use std::process::Command;

#[test]
fn versao_prints_the_version_of_the_news() {
    let out = Command::new(env!("CARGO_BIN_EXE_raven-ci")).arg("--versao").output().expect("the binary runs");
    assert!(out.status.success());
    let news = ravenrs_app_update::parse_news(include_str!("../../novidades.json")).expect("the news");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), news.versao);
}

#[test]
fn it_tests_itself_with_a_fake_plugin_for_every_point() {
    let out =
        Command::new(env!("CARGO_BIN_EXE_raven-ci")).arg("--self-test").output().expect("the binary runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("Autoteste: 12 plugins falsos"));
}

#[test]
fn the_card_for_agents_says_what_the_binary_does() {
    let card: serde_json::Value = serde_json::from_str(include_str!("../../tool.json")).expect("tool.json");
    assert_eq!(card["window"], false);
    assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
    let commands: Vec<&str> =
        card["commands"].as_array().expect("commands").iter().filter_map(|c| c["command"].as_str()).collect();
    for needed in
        ["raven-ci plugins listar", "raven-ci plugins info", "raven-ci plugins ligar", "raven-ci --self-test"]
    {
        assert!(commands.iter().any(|c| c.starts_with(needed)), "tool.json lists {needed}");
    }
}
