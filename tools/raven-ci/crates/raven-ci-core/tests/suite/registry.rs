use std::sync::Arc;

use raven_ci_core::{Ci, Memory, Origin, PluginState};
use raven_ci_plugin_api::testing::Fake;
use raven_ci_plugin_api::{Permission, Point};
use serde_json::json;

fn state(ci: &Ci, name: &str) -> PluginState {
    ci.registry().info(name).expect("known").state
}

#[test]
fn a_plugin_that_wants_another_version_is_refused_with_why_and_the_rest_start() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::Scm).named("git").version("1.4.0")));
    ci.add_plugin(Arc::new(Fake::new(Point::StatusReporter).named("github").requires("git", "^2")));
    ci.add_plugin(Arc::new(Fake::new(Point::Notifier).named("email")));
    let changes = ci.apply_plugins().expect("applies");
    assert_eq!(state(&ci, "git"), PluginState::On);
    assert_eq!(state(&ci, "email"), PluginState::On);
    assert_eq!(
        state(&ci, "github"),
        PluginState::Refused(
            "precisa de git a partir da 2.0.0 e abaixo da 3.0.0, e o instalado é 1.4.0".into()
        )
    );
    assert!(changes.iter().any(|c| c.starts_with("github recusado")), "{changes:?}");
}

#[test]
fn who_depends_on_a_refused_plugin_is_refused_saying_of_whom() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::Scm).named("git").requires("ssh", "^1")));
    ci.add_plugin(Arc::new(Fake::new(Point::BranchSource).named("github").after("git")));
    ci.apply_plugins().expect("applies");
    assert_eq!(state(&ci, "git"), PluginState::Refused("precisa de ssh, que não está instalado".into()));
    assert_eq!(state(&ci, "github"), PluginState::Refused("precisa de git, que ficou de fora".into()));
}

#[test]
fn two_plugins_that_need_each_other_are_refused_by_the_cycle() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::Scm).named("a").after("b")));
    ci.add_plugin(Arc::new(Fake::new(Point::Trigger).named("b").after("a")));
    ci.apply_plugins().expect("applies");
    assert_eq!(state(&ci, "a"), PluginState::Refused("depende em ciclo de a, b".into()));
    assert_eq!(state(&ci, "b"), PluginState::Refused("depende em ciclo de a, b".into()));
}

#[test]
fn switching_off_a_plugin_stops_those_that_need_it_in_order_without_a_restart() {
    let ci = Ci::in_memory().expect("a core");
    let git = Fake::new(Point::Scm).named("git");
    let github = Fake::new(Point::StatusReporter).named("github").after("git");
    let (git_probe, github_probe) = (git.probe(), github.probe());
    ci.add_plugin(Arc::new(git));
    ci.add_plugin(Arc::new(github));
    ci.apply_plugins().expect("applies");
    assert_eq!((git_probe.started(), github_probe.started()), (1, 1));

    let changes = ci.disable_plugin("git").expect("switches off");
    assert_eq!(
        changes[..2],
        ["github desligado".to_owned(), "git desligado".to_owned()],
        "the one that needs it first"
    );
    assert_eq!(state(&ci, "git"), PluginState::Off);
    assert_eq!(state(&ci, "github"), PluginState::Refused("precisa de git, que está desligado".into()));
    assert_eq!((git_probe.stopped(), github_probe.stopped()), (1, 1));
    assert!(
        ci.registry().info("github").expect("known").enabled,
        "github stays switched on, waiting for git"
    );

    ci.enable_plugin("git").expect("switches on");
    assert_eq!(state(&ci, "github"), PluginState::On, "it comes back with git");
    assert_eq!((git_probe.started(), github_probe.started()), (2, 2));
    assert!(
        matches!(ci.disable_plugin("nao-existe"), Err(e) if e.to_string() == "não há plugin com o nome nao-existe")
    );
}

#[test]
fn a_migration_runs_once_however_many_times_the_plugin_comes_back() {
    let ci = Ci::in_memory().expect("a core");
    let fake = Fake::new(Point::Publisher)
        .named("artefatos")
        .permission(Permission::Disk)
        .migration("1-tabela", "CREATE TABLE guardados (nome TEXT); INSERT INTO guardados VALUES ('um');");
    let probe = fake.probe();
    ci.add_plugin(Arc::new(fake));
    ci.apply_plugins().expect("applies");
    for _ in 0..3 {
        ci.disable_plugin("artefatos").expect("off");
        ci.enable_plugin("artefatos").expect("on");
    }
    let path = probe.host().expect("on").database().expect("its database");
    let conn = rusqlite::Connection::open(path).expect("opens");
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM guardados", [], |row| row.get(0)).expect("counts");
    assert_eq!(rows, 1, "the insert ran once");
}

#[test]
fn a_plugin_with_migrations_and_no_disk_is_refused() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::Publisher).named("sem-disco").migration("1", "SELECT 1;")));
    ci.apply_plugins().expect("applies");
    assert!(matches!(state(&ci, "sem-disco"), PluginState::Refused(why) if why.contains("ci.disk")));
    let audit = ci.store().audit_of("sem-disco").expect("audit");
    assert_eq!((audit[0].permission, audit[0].granted), (Permission::Disk, false));
}

#[test]
fn the_configuration_is_checked_against_what_the_plugin_describes() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::StatusReporter).named("github").configured()));
    ci.apply_plugins().expect("applies");
    assert!(
        matches!(state(&ci, "github"), PluginState::Refused(why) if why.starts_with("a configuração context:")),
        "it needs its context before it starts"
    );
    let wrong =
        ci.configure_plugin("github", json!({ "context": "ci", "every": "cinco" })).expect_err("wrong");
    assert_eq!(wrong.to_string(), "a configuração every: tem que ser um número inteiro");
    ci.configure_plugin("github", json!({ "context": "raven-ci/build", "every": 5 })).expect("right");
    assert_eq!(state(&ci, "github"), PluginState::On);
    let host = ci.registry().info("github").expect("known");
    assert_eq!(host.settings, json!({ "context": "raven-ci/build", "every": 5 }));
}

#[test]
fn a_permission_taken_away_is_denied_and_on_the_audit() {
    let ci = Ci::in_memory().expect("a core");
    let fake =
        Fake::new(Point::Scm).named("git").permission(Permission::Disk).permission(Permission::Secrets);
    let probe = fake.probe();
    ci.add_plugin(Arc::new(fake));
    ci.apply_plugins().expect("applies");
    ci.registry().revoke("git", Permission::Secrets, true).expect("taken away");
    let view = ci.registry().info("git").expect("known");
    assert_eq!(view.declared, [Permission::Secrets, Permission::Disk]);
    assert_eq!(view.granted, [Permission::Disk]);
    let host = probe.host().expect("started again");
    assert!(host.data_dir().is_ok());
    assert!(host.secret("token").is_err());
    let audit = ci.store().audit_of("git").expect("audit");
    assert_eq!(audit.iter().map(|a| a.granted).collect::<Vec<_>>(), [true, false]);
    assert_eq!(audit[1].what, "ler o segredo token");
}

#[test]
fn a_plugin_that_fails_to_start_is_refused_and_takes_those_that_need_it() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::Scm).named("git").failing_to_start("o git não está instalado")));
    ci.add_plugin(Arc::new(Fake::new(Point::BranchSource).named("github").after("git")));
    ci.apply_plugins().expect("applies");
    assert_eq!(state(&ci, "git"), PluginState::Refused("não ligou: o git não está instalado".into()));
    assert_eq!(state(&ci, "github"), PluginState::Refused("precisa de git, que não ligou".into()));
}

#[test]
fn the_list_says_where_each_plugin_came_from_what_it_offers_and_its_memory() {
    let ci = Ci::in_memory().expect("a core");
    ci.add_plugin(Arc::new(Fake::new(Point::Trigger)));
    ci.apply_plugins().expect("applies");
    let view = ci.registry().info("fake-trigger").expect("known");
    assert_eq!(view.origin, Origin::Embedded);
    assert_eq!(view.points, [Point::Trigger]);
    assert_eq!(view.memory, Memory::Shared);
    assert_eq!(view.version.as_deref(), Some("1.0.0"));
    let json = serde_json::to_value(&view).expect("as JSON");
    assert_eq!(json["state"], json!({ "state": "on" }));
}

#[test]
fn a_core_in_memory_takes_the_folders_of_its_plugins_with_it() {
    let ci = Ci::in_memory().expect("a core");
    let fake = Fake::new(Point::Scm).named("git").permission(Permission::Disk);
    let probe = fake.probe();
    ci.add_plugin(Arc::new(fake));
    ci.apply_plugins().expect("applies");
    let dir = probe.host().expect("on").data_dir().expect("its folder");
    assert!(dir.is_dir());
    let other = Ci::in_memory().expect("another core");
    drop(ci);
    drop(probe);
    assert!(!dir.exists(), "gone with its core");
    drop(other);
}
