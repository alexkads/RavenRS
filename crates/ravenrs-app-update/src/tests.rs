//! The rules of updating, with no network: the decision, the status bar, the
//! updater's messages, the lock, and an installer that does not compile.

use std::path::{Path, PathBuf};

use super::installer::*;
use super::*;

/// An app installed by the installer, for the tests.
static TEST_APP: InstalledApp = InstalledApp {
    name: "Raven Teste",
    package: "raven-teste",
    home_dir: ".raven-teste-da-atualizacao",
    bundle_id: "dev.ravenrs.raven-teste",
    icons: "examples/raven-teste/assets",
    mac_category: "public.app-category.productivity",
    linux_categories: "Office;",
    source: "https://example.invalid/raven-teste/master.tar.gz",
    env_prefix: "RT",
    installed_env: "RAVEN_TESTE_INSTALLED",
    news_urls: &["https://example.invalid/raven-teste/novidades.json"],
    history_urls: &["https://example.invalid/raven-teste/historico-de-novidades.json"],
    installer_url: "https://example.invalid/raven-teste/install.cmd",
    issues_url: "https://example.invalid/raven-teste/issues/",
    version: "0.1.0",
    installed: true,
    history: r#"[{"versao":"0.1.0","data":"2026-11-01","titulo":"A primeira","novidades":[],"por_que_atualizar":"p"}]"#,
};

/// Another app, which must keep to its own folders.
static OTHER_APP: InstalledApp = InstalledApp {
    name: "Outro Ávido",
    package: "outro-app",
    home_dir: ".outro-app",
    bundle_id: "dev.ravenrs.outro-app",
    icons: "examples/outro-app/assets",
    mac_category: "public.app-category.utilities",
    linux_categories: "Utility;",
    source: "https://example.invalid/outro/master.tar.gz",
    env_prefix: "OA",
    installed_env: "OUTRO_APP_INSTALLED",
    news_urls: &[],
    history_urls: &[],
    installer_url: "https://example.invalid/outro/install.cmd",
    issues_url: "https://example.invalid/outro/issues",
    version: "1.0.0",
    installed: false,
    history: "[]",
};

fn news(version: &str) -> News {
    News {
        versao: version.to_owned(),
        data: None,
        titulo: "Algo novo".to_owned(),
        importante: false,
        novidades: vec![],
        por_que_atualizar: "Porque sim".to_owned(),
        contribuidores: Vec::new(),
        issues: Vec::new(),
    }
}

#[test]
fn versions_compare_as_numbers() {
    assert!(is_newer("0.1.10", "0.1.9"));
    assert!(is_newer("1.0.0", "0.9.9"));
    assert!(!is_newer("0.1.0", "0.1.0"));
    assert!(!is_newer("0.0.9", "0.1.0"));
    assert!(!is_newer("nova", "0.1.0"));
}

#[test]
fn broken_news_is_no_news() {
    assert_eq!(parse_news("<html>404</html>"), None);
    assert_eq!(parse_news(r#"{"versao":"x","titulo":"a","novidades":[],"por_que_atualizar":"b"}"#), None);
}

#[test]
fn an_installed_copy_starts_compiling_by_itself() {
    let decision =
        decide(InstallKind::Installed, "0.1.0", Some(news("0.2.0")), &Memory::default(), false, 1000);
    assert_eq!(decision, Decision::Announce { news: news("0.2.0"), start: true });
}

#[test]
fn nothing_starts_by_itself_in_development_or_while_busy() {
    for (kind, busy) in [(InstallKind::Development, false), (InstallKind::Installed, true)] {
        let decision = decide(kind, "0.1.0", Some(news("0.2.0")), &Memory::default(), busy, 1000);
        assert_eq!(decision, Decision::Announce { news: news("0.2.0"), start: false });
    }
}

#[test]
fn a_failed_version_waits_a_day() {
    let memory = Memory { failed: Some("0.2.0".to_owned()), when: Some(1000) };
    let soon = 1000 + RETRY_AFTER.as_secs() - 1;
    let later = 1000 + RETRY_AFTER.as_secs();
    let start_at =
        |now| match decide(InstallKind::Installed, "0.1.0", Some(news("0.2.0")), &memory, false, now) {
            Decision::Announce { start, .. } => start,
            Decision::Nothing => panic!("should announce"),
        };
    assert!(!start_at(soon));
    assert!(start_at(later));
    // Another version is not held back by this one's failure.
    let other = decide(InstallKind::Installed, "0.1.0", Some(news("0.3.0")), &memory, false, soon);
    assert!(matches!(other, Decision::Announce { start: true, .. }));
}

#[test]
fn no_announcement_when_up_to_date_or_offline() {
    let quiet = |n| decide(InstallKind::Installed, "0.2.0", n, &Memory::default(), false, 0);
    assert_eq!(quiet(None), Decision::Nothing);
    assert_eq!(quiet(Some(news("0.2.0"))), Decision::Nothing);
    assert_eq!(quiet(Some(news("0.1.0"))), Decision::Nothing);
}

#[test]
fn a_thread_that_died_stops_the_waiting_and_says_so() {
    let updater = Updater::idle(&TEST_APP);
    let (send, receive) = channel::<Message>();
    *updater.inbox.borrow_mut() = Some(receive);
    updater.set_status(UpdateStatus::Compiling(news("0.2.0")));
    updater.poll();
    assert!(updater.is_running(), "still alive: nothing to read yet");
    drop(send);
    updater.poll();
    assert!(!updater.is_running());
    assert!(matches!(updater.status(), UpdateStatus::Failed(..)));
}

#[test]
fn the_status_bar_says_what_the_other_app_says() {
    let n = news("0.2.0");
    let say = |status: &UpdateStatus| notice(status, "0.1.0", Some("compilando"), None);
    use NoticeAction::*;

    assert_eq!(say(&UpdateStatus::Quiet), None);

    let available = say(&UpdateStatus::Available(n.clone())).unwrap();
    assert!(available.text.starts_with("Versão 0.2.0 disponível — "));
    assert_eq!(available.actions, [News, Update, Later]);
    assert!(!available.important && !available.busy);

    let compiling = say(&UpdateStatus::Compiling(n.clone())).unwrap();
    assert!(compiling.text.contains("em segundo plano — compilando. O app continua funcionando."));
    assert!(compiling.busy);
    assert_eq!(compiling.actions, [News]);
    let starting = notice(&UpdateStatus::Compiling(n.clone()), "0.1.0", None, None).unwrap();
    assert!(starting.text.contains("— começando."));

    let ready = say(&UpdateStatus::Ready(n.clone())).unwrap();
    assert!(ready.text.contains("entra na próxima vez que o app abrir"));
    assert_eq!(ready.actions, [Reopen]);

    let failed = say(&UpdateStatus::Failed(n.clone(), "sem disco.".to_owned())).unwrap();
    assert!(failed.text.contains("a 0.1.0 continua funcionando."));
    assert!(!failed.text.contains("sem disco"), "the reason is in the details, where it is not cut");
    assert_eq!(failed.actions, [Retry, Details, Close]);

    let checking = say(&UpdateStatus::Checking).unwrap();
    assert!(checking.busy && checking.actions.is_empty());
    assert!(say(&UpdateStatus::UpToDate).unwrap().text.contains("mais recente (0.1.0)"));
    assert_eq!(say(&UpdateStatus::NoAnswer("sem rede".to_owned())).unwrap().actions, [CheckAgain, Close]);
}

#[test]
fn only_an_important_version_stands_out() {
    let important = News { importante: true, ..news("0.2.0") };
    let say = |status| notice(&status, "0.1.0", None, None).unwrap();
    let offered = say(UpdateStatus::Available(important.clone()));
    assert!(offered.important && offered.text.starts_with("Atualização importante: versão 0.2.0"));
    assert!(say(UpdateStatus::Ready(important.clone())).important);
    assert!(!say(UpdateStatus::Compiling(important.clone())).important);
    assert!(!say(UpdateStatus::Failed(important, String::new())).important);
}

#[test]
fn later_hides_that_version_for_the_session_and_nothing_under_way() {
    let n = news("0.2.0");
    let hide = Some("0.2.0");
    assert_eq!(notice(&UpdateStatus::Available(n.clone()), "0.1.0", None, hide), None);
    assert_eq!(notice(&UpdateStatus::Failed(n.clone(), String::new()), "0.1.0", None, hide), None);
    assert!(notice(&UpdateStatus::Available(news("0.3.0")), "0.1.0", None, hide).is_some());
    assert!(notice(&UpdateStatus::Compiling(n.clone()), "0.1.0", None, hide).is_some());
    assert!(notice(&UpdateStatus::Ready(n), "0.1.0", None, hide).is_some());
}

#[test]
fn dismissing_hides_a_version_and_clears_an_answer_and_asking_brings_it_back() {
    let updater = Updater::idle(&TEST_APP);
    updater.set_status(UpdateStatus::Available(news("0.2.0")));
    updater.dismiss();
    assert_eq!(updater.notice(), None);
    assert!(matches!(updater.status(), UpdateStatus::Available(_)), "the version is still known");
    updater.ask();
    assert_eq!(updater.status(), UpdateStatus::Checking);
    assert!(updater.dismissed.borrow().is_none());

    updater.set_status(UpdateStatus::UpToDate);
    updater.dismiss();
    assert_eq!(updater.status(), UpdateStatus::Quiet);
}

#[test]
fn asking_does_not_get_in_the_way_of_an_update_under_way() {
    let updater = Updater::idle(&TEST_APP);
    for status in [UpdateStatus::Compiling(news("0.2.0")), UpdateStatus::Ready(news("0.2.0"))] {
        updater.set_status(status.clone());
        updater.ask();
        assert_eq!(updater.status(), status);
    }
}

fn answer(updater: &Updater, decision: Decision, failure: Option<&str>) {
    let (send, receive) = channel();
    *updater.inbox.borrow_mut() = Some(receive);
    send.send(Message::Checked { decision, failure: failure.map(str::to_owned), history: Vec::new() })
        .unwrap();
    updater.poll();
}

#[test]
fn only_a_check_someone_asked_for_answers_when_there_is_nothing_new() {
    let updater = Updater::idle(&TEST_APP);
    answer(&updater, Decision::Nothing, None);
    assert_eq!(updater.status(), UpdateStatus::Quiet, "the hourly look says nothing");
    answer(&updater, Decision::Nothing, Some("sem rede"));
    assert_eq!(updater.status(), UpdateStatus::Quiet, "nor does it complain about the network");

    updater.set_status(UpdateStatus::Checking);
    answer(&updater, Decision::Nothing, None);
    assert_eq!(updater.status(), UpdateStatus::UpToDate);

    updater.set_status(UpdateStatus::Checking);
    answer(&updater, Decision::Nothing, Some("sem rede"));
    assert_eq!(updater.status(), UpdateStatus::NoAnswer("sem rede".to_owned()));
}

#[test]
fn it_looks_again_on_the_hour_unless_something_is_under_way() {
    let hour = CHECK_EVERY;
    let minute = Duration::from_secs(60);
    assert!(check_is_due(None, &UpdateStatus::Quiet));
    assert!(!check_is_due(Some(minute), &UpdateStatus::Quiet));
    assert!(check_is_due(Some(hour), &UpdateStatus::Quiet));
    assert!(check_is_due(Some(hour), &UpdateStatus::Available(news("0.2.0"))));
    assert!(!check_is_due(Some(hour), &UpdateStatus::Compiling(news("0.2.0"))));
    assert!(!check_is_due(Some(hour), &UpdateStatus::Ready(news("0.2.0"))));
    assert!(!check_is_due(Some(hour), &UpdateStatus::Checking));
}

#[test]
fn a_test_updater_never_looks_by_itself() {
    let updater = Updater::idle(&TEST_APP);
    updater.connect(|| {});
    updater.poll();
    assert!(!updater.is_running());
}

#[test]
fn the_steps_come_from_the_lines_the_installer_announces() {
    assert_eq!(installer_stage("> baixando o codigo"), Some("baixando o codigo".to_owned()));
    assert_eq!(installer_stage("   Compiling ravenrs v27.1.1"), None);
    assert_eq!(installer_stage("> "), None);
    assert_eq!(installer_stage("> compilando (a primeira vez demora)"), Some("compilando".to_owned()));
    let log = "\n> baixando o codigo\nFrom github\n\n> compilando\n   Compiling x\nwarning: y\n";
    assert_eq!(last_stage(log).as_deref(), Some("compilando"));
    assert_eq!(last_stage("nada anunciado\n"), None);
}

#[test]
fn a_broken_history_entry_is_skipped() {
    let text = r#"[{"versao":"0.1.0","titulo":"t","novidades":[],"por_que_atualizar":"p"},{"nada":1}]"#;
    assert_eq!(parse_history(text).len(), 1);
    assert!(parse_history("<html>erro</html>").is_empty());
}

#[test]
fn a_check_asked_for_before_the_window_exists_runs_when_it_does() {
    let updater = Updater::idle(&TEST_APP);
    updater.check();
    assert!(!updater.is_running(), "nothing could wake the UI yet");
    assert!(updater.check_when_connected.get());
    // The thread starts here and asks the network; only whether it started matters.
    updater.connect(|| {});
    assert!(updater.is_running());
    assert!(!updater.check_when_connected.get());
}

#[test]
fn a_stale_lock_does_not_hold_updates_back() {
    let dir = std::env::temp_dir().join(format!("ff-lock-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let lock = dir.join("trava");
    assert!(!is_locked(&lock, 100));
    std::fs::write(&lock, "100").unwrap_or_else(|e| panic!("{e}"));
    assert!(is_locked(&lock, 200));
    assert!(!is_locked(&lock, 100 + LOCK_EXPIRES.as_secs()));
    let _ = std::fs::remove_file(&lock);
}

#[test]
fn a_lock_whose_log_went_quiet_was_left_by_a_closed_window() {
    let (quiet, began) = (LOG_QUIET_FOR.as_secs(), 1_000);
    assert!(lock_is_live(began, None, began + 60), "just begun, no log yet");
    assert!(!lock_is_live(began, None, began + quiet + 1), "never wrote a line");
    assert!(lock_is_live(began, Some(began + 3_000), began + 3_000 + 60), "compiling, still writing");
    assert!(!lock_is_live(began, Some(began + 600), began + 600 + quiet + 1), "stopped writing");
    assert!(!lock_is_live(began, Some(began - 50), began + quiet + 1), "that log is the last run's");
    assert!(
        !lock_is_live(began, Some(began + 9_000), began + LOCK_EXPIRES.as_secs()),
        "never past the limit"
    );
}

#[test]
fn looking_again_does_not_erase_a_failure_on_the_same_version() {
    let updater = Updater::idle(&TEST_APP);
    let failed = UpdateStatus::Failed(news("0.2.0"), "sem disco.".to_owned());
    updater.set_status(failed.clone());
    answer(&updater, Decision::Announce { news: news("0.2.0"), start: false }, None);
    assert_eq!(updater.status(), failed);
}

#[test]
fn a_copy_built_by_hand_only_mentions_the_version() {
    static BY_HAND: InstalledApp = InstalledApp { installed: false, ..TEST_APP };
    let updater = Updater::idle(&BY_HAND);
    answer(&updater, Decision::Announce { news: news("0.2.0"), start: true }, None);
    assert_eq!(updater.status(), UpdateStatus::Development(news("0.2.0")));
}

#[test]
fn the_timeline_joins_what_is_known_with_the_newest_on_top() {
    let told_by_repository = News { titulo: "do repositório".to_owned(), ..news("0.2.0") };
    let joined = join_history([
        vec![news("0.3.0")],
        vec![told_by_repository.clone(), news("0.1.10")],
        vec![news("0.1.9"), news("0.1.10")],
    ]);
    let versions: Vec<&str> = joined.iter().map(|n| n.versao.as_str()).collect();
    assert_eq!(versions, ["0.3.0", "0.2.0", "0.1.10", "0.1.9"]);
    assert_eq!(joined[1].titulo, "do repositório");

    let dated =
        News { data: Some("2026-11-01".to_owned()), titulo: "do histórico".to_owned(), ..news("0.2.0") };
    let joined = join_history([vec![told_by_repository], vec![dated]]);
    assert_eq!(joined[0].titulo, "do repositório");
    assert_eq!(joined[0].data.as_deref(), Some("2026-11-01"), "the announcement has no date of its own");

    let updater = Updater::idle(&TEST_APP);
    updater.set_status(UpdateStatus::Available(news("99.0.0")));
    let timeline = updater.timeline();
    assert_eq!(timeline[0].versao, "99.0.0");
    assert!(timeline.iter().any(|n| n.versao == TEST_APP.version));
}

// The identity.

#[test]
fn each_app_keeps_to_its_own_folders_and_addresses() {
    let (mine, other) = (&TEST_APP, &OTHER_APP);
    assert!(mine.home().ends_with(".raven-teste-da-atualizacao"));
    assert!(other.home().ends_with(".outro-app"));
    assert_ne!(mine.installed_binary(), other.installed_binary());
    assert_ne!(mine.previous_version(), other.previous_version());
    assert_ne!(mine.installer_log(), other.installer_log());
    let binary = other.installed_binary().unwrap_or_default().display().to_string();
    assert!(binary.contains("outro-app") && !binary.contains("raven-teste"), "{binary}");
    assert_eq!(other.windows_dir(), "OutroAvido");
    assert_eq!(other.ascii_name(), "Outro Avido");
    assert_eq!(mine.issue_url(42), "https://example.invalid/raven-teste/issues/42");
    assert_eq!(other.install_kind(), InstallKind::Development);
    assert_eq!(mine.install_kind(), InstallKind::Installed);
}

#[test]
fn the_command_by_hand_downloads_the_app_s_installer_to_a_file() {
    for windows in [true, false] {
        let command = OTHER_APP.install_command_on(windows);
        assert!(command.contains(OTHER_APP.installer_url), "{command}");
        assert!(!command.contains("| sh") && !command.contains("| iex"), "never piped into a shell");
    }
    assert_eq!(TEST_APP.install_command(), TEST_APP.install_command_on(cfg!(windows)));
}

// The installer made from the template.

fn installer_of(app: &InstalledApp) -> String {
    app.render_installer().unwrap_or_else(|problem| panic!("{problem}"))
}

#[test]
fn every_mark_of_the_template_is_filled() {
    let text = installer_of(&OTHER_APP);
    let left: Vec<&str> = text
        .split('@')
        .skip(1)
        .step_by(2)
        .filter(|mark| !mark.is_empty() && mark.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .collect();
    assert!(left.is_empty(), "marks left: {left:?}");
    for value in [
        "Outro Ávido",
        "Outro Avido",
        "OutroAvido",
        "outro-app",
        ".outro-app",
        "dev.ravenrs.outro-app",
        "examples/outro-app/assets/icon.icns",
        r"examples\outro-app\assets\icon.ico",
        "OA_SEM_PAUSA",
        "OA_CASA",
        "OUTRO_APP_INSTALLED=1",
        "public.app-category.utilities",
        "Categories=Utility;",
        "https://example.invalid/outro/master.tar.gz",
    ] {
        assert!(text.contains(value), "{value}");
    }
    assert!(!text.contains("raven-teste") && !text.contains("Raven Teste"));
}

#[test]
fn the_installer_made_is_whole_and_a_cut_one_is_not() {
    let text = installer_of(&TEST_APP);
    assert!(installer_is_whole(&text));
    assert!(!installer_is_whole(&text[..text.len() / 2]));
    assert!(!installer_is_whole("<html>error</html>"));
    assert!(!installer_is_whole(""));
}

#[test]
fn the_installer_announces_steps_on_every_system() {
    let installer = installer_of(&TEST_APP);
    assert!(installer.contains("printf '\\n> %s\\n'"), "sh");
    assert!(installer.contains("Write-Host \"> compilando"), "PowerShell");
}

#[test]
fn macos_lists_one_app_with_its_new_icon() {
    let text = installer_of(&TEST_APP);
    // In the Applications folder, the previous version would show as a second app.
    assert!(text.contains(r#"ANTERIOR="$CASA/anterior/$NOME.app""#));
    // macOS keeps icons by path, so it has to be told the app changed.
    assert!(text.contains("lsregister"));
    // A version that changes on every install is what makes it read the icon again.
    assert!(text.contains("<key>CFBundleVersion</key><string>$(date"));
}

#[test]
fn gnome_finds_the_icon_through_the_window_id() {
    let installer = installer_of(&TEST_APP);
    let id = TEST_APP.package;
    assert!(installer.contains(&format!("share/applications/{id}.desktop")), "the file's name is the id");
    assert!(installer.contains(&format!("StartupWMClass={id}")));
    assert!(installer.contains(&format!("Icon={id}")));
    assert!(installer.contains(&format!("hicolor/256x256/apps/{id}.png")));
}

#[test]
fn an_update_recompiles_what_changed() {
    // Without -m the files keep their commit's date, older than the last build, so
    // Cargo skipped the crates that changed and the update failed to compile.
    let text = installer_of(&TEST_APP);
    assert_eq!(text.matches("tar -xzmf").count(), 2, "on both sides of the installer");
    assert!(!text.contains("tar -xzf"));
}

#[test]
fn a_field_that_would_change_the_script_is_refused() {
    let broken = [
        InstalledApp { name: "Raven \"Teste\"", ..TEST_APP },
        InstalledApp { name: "Raven $HOME", ..TEST_APP },
        InstalledApp { package: "Raven_Teste", ..TEST_APP },
        InstalledApp { home_dir: "raven-teste", ..TEST_APP },
        InstalledApp { home_dir: ".a/b", ..TEST_APP },
        InstalledApp { env_prefix: "rt", ..TEST_APP },
        InstalledApp { source: "https://example.invalid/a b.tar.gz", ..TEST_APP },
        InstalledApp { linux_categories: "Office", ..TEST_APP },
        InstalledApp { name: "", ..TEST_APP },
    ];
    for app in broken {
        assert!(app.render_installer().is_err(), "{app:?}");
    }
}

#[test]
fn the_committed_copy_must_be_the_one_made() {
    let dir = scratch("copia");
    let file = dir.join("install.cmd");
    assert!(check_installer(&TEST_APP, &file).is_err(), "missing");
    std::fs::write(&file, installer_of(&TEST_APP)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(check_installer(&TEST_APP, &file), Ok(()));
    let problem = check_installer(&OTHER_APP, &file).err().unwrap_or_default();
    assert!(problem.contains(WRITE_INSTALLER_ENV), "{problem}");
    let _ = std::fs::remove_dir_all(dir);
}

// The three release files.

fn release(dir: &Path, cargo_version: &str, news: &str, history: &str) {
    let write =
        |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap_or_else(|e| panic!("{e}"));
    write("Cargo.toml", &format!("[package]\nname = \"a\"\nversion = \"{cargo_version}\"\n"));
    write("novidades.json", news);
    write("historico-de-novidades.json", history);
}

const NEWS: &str = r#"{"versao":"0.2.0","titulo":"t","novidades":["a"],"por_que_atualizar":"p","contribuidores":["Ana"],"issues":[7]}"#;

fn dated(news: &str) -> String {
    format!(
        "[{}, {{\"versao\":\"0.1.0\",\"titulo\":\"t\",\"novidades\":[],\"por_que_atualizar\":\"p\"}}]",
        news.replacen('{', "{\"data\":\"2026-11-15\",", 1)
    )
}

#[test]
fn the_release_files_agree_or_say_what_to_fix() {
    let dir = scratch("versao");
    release(&dir, "0.2.0", NEWS, &dated(NEWS));
    assert_eq!(check_release_files(&dir), Ok(()));
    assert_release_files_agree(&dir);

    let fails = |cargo: &str, news: &str, history: &str, says: &str| {
        release(&dir, cargo, news, history);
        let problem = check_release_files(&dir).err().unwrap_or_default();
        assert!(problem.contains(says), "{problem:?} should say {says:?}");
    };
    fails("0.2.1", NEWS, &dated(NEWS), "raise novidades.json together with Cargo.toml");
    let nobody = NEWS.replace(r#""contribuidores":["Ana"]"#, r#""contribuidores":[]"#);
    fails("0.2.0", &nobody, &dated(&nobody), "contribuidores");
    let blank = NEWS.replace(r#"["Ana"]"#, r#"["  "]"#);
    fails("0.2.0", &blank, &dated(&blank), "contribuidores");
    let no_issues = NEWS.replace(r#""issues":[7]"#, r#""issues":[]"#);
    fails("0.2.0", &no_issues, &dated(&no_issues), "issues");
    let older_history = dated(&NEWS.replace("0.2.0", "0.1.9"));
    fails("0.2.0", NEWS, &older_history, "the history starts with this version's entry");
    fails("0.2.0", NEWS, &format!("[{NEWS}]"), "the history records the day");
    fails("0.2.0", NEWS, "[]", "empty history");
    fails("0.2.0", "<html>404</html>", &dated(NEWS), "novidades.json");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[should_panic(expected = "raise novidades.json together with Cargo.toml")]
fn the_assertion_fails_when_they_disagree() {
    let dir = scratch("versao-panico");
    release(&dir, "0.3.0", NEWS, &dated(NEWS));
    assert_release_files_agree(&dir);
}

// Running an installer that does not compile, on sh.

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ravenrs-app-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// A whole installer for `sh` that "compiles" a binary answering `answers` and
/// moves it in the way the real one does on Linux, keeping the previous as `.anterior`.
#[cfg(unix)]
fn fake_installer(binary: &Path, answers: &str) -> String {
    format!(
        r#":<<"::FIM-DO-CMD"
exit /b 1
::FIM-DO-CMD
: <<'#==FIM-POWERSHELL=='
#==POWERSHELL==
throw "só para sh"
#==FIM-POWERSHELL==
{{
set -eu
[ "${{RT_SEM_PAUSA:-}}" = 1 ] || exit 3
printf '\n> %s\n' "compilando (falso)"
INSTALADO="{binary}"
mkdir -p "$(dirname "$INSTALADO")"
printf '#!/bin/sh\necho {answers}\n' > "$INSTALADO.novo"
chmod 755 "$INSTALADO.novo"
if [ -f "$INSTALADO" ]; then cp -p "$INSTALADO" "$INSTALADO.anterior"; fi
mv -f "$INSTALADO.novo" "$INSTALADO"
# Fim do instalador falso.
}}
"#,
        binary = binary.display()
    )
}

#[cfg(unix)]
fn installed_at(root: &Path, answers: &str) -> PathBuf {
    let binary = root.join("bin").join(TEST_APP.package);
    let _ = std::fs::create_dir_all(binary.parent().unwrap_or(root));
    let script = format!("#!/bin/sh\necho {answers}\n");
    std::fs::write(&binary, script).unwrap_or_else(|e| panic!("{e}"));
    let _ = std::process::Command::new("chmod").arg("755").arg(&binary).status();
    binary
}

#[cfg(unix)]
fn answer_of(binary: &Path) -> String {
    let output =
        std::process::Command::new(binary).arg("--versao").output().unwrap_or_else(|e| panic!("{e}"));
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[cfg(unix)]
fn setup(home: &Path, binary: &Path, installer: String) -> Setup<'static> {
    Setup {
        home: home.to_owned(),
        binary: Some(binary.to_owned()),
        no_pause_env: TEST_APP.no_pause_env(),
        fetch: Box::new(move || Ok(installer.clone())),
    }
}

#[cfg(unix)]
#[test]
fn an_installed_copy_finds_the_new_version_and_swaps_only_after_it_answers() {
    let root = scratch("troca");
    let home = root.join(TEST_APP.home_dir);
    let binary = installed_at(&root, "0.1.38");
    let now = seconds_now();
    let decision = decide(
        InstallKind::Installed,
        "0.1.38",
        Some(news("0.1.39")),
        &read_memory(&home.join(MEMORY)),
        is_locked(&home.join(LOCK), now),
        now,
    );
    assert_eq!(decision, Decision::Announce { news: news("0.1.39"), start: true });

    let stages = std::sync::Mutex::new(Vec::new());
    let result =
        run_installer(&setup(&home, &binary, fake_installer(&binary, "0.1.39")), "0.1.39", &|stage| {
            stages.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(stage);
        });
    assert_eq!(result, Ok(()));
    assert_eq!(answer_of(&binary), "0.1.39");
    let mut previous = binary.clone().into_os_string();
    previous.push(".anterior");
    assert_eq!(answer_of(Path::new(&previous)), "0.1.38", "the previous version is kept");
    assert!(
        stages.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&"compilando".to_owned())
    );
    assert!(!home.join(LOCK).exists(), "the lock goes when it ends");
    assert_eq!(read_memory(&home.join(MEMORY)), Memory::default());
    assert!(home.join("install.cmd").exists(), "it ran from a file, never from a pipe");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn a_version_that_did_not_answer_waits_a_day_by_the_clock() {
    let root = scratch("falha");
    let home = root.join(TEST_APP.home_dir);
    let binary = installed_at(&root, "0.1.38");
    // It "installs" a binary that still says the old version.
    let result = run_installer(&setup(&home, &binary, fake_installer(&binary, "0.1.38")), "0.1.39", &|_| {});
    assert_eq!(result, Err("o instalado responde 0.1.38, e não 0.1.39.".to_owned()));
    let memory = read_memory(&home.join(MEMORY));
    assert_eq!(memory.failed.as_deref(), Some("0.1.39"));
    let when = memory.when.unwrap_or_default();
    let starts =
        |now| match decide(InstallKind::Installed, "0.1.38", Some(news("0.1.39")), &memory, false, now) {
            Decision::Announce { start, .. } => start,
            Decision::Nothing => panic!("should announce"),
        };
    assert!(!starts(when + 3600), "an hour later it only announces");
    assert!(!starts(when + RETRY_AFTER.as_secs() - 1));
    assert!(starts(when + RETRY_AFTER.as_secs()), "a day later it tries by itself");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn an_installer_cut_in_half_is_refused_and_nothing_is_touched() {
    let root = scratch("metade");
    let home = root.join(TEST_APP.home_dir);
    let binary = installed_at(&root, "0.1.38");
    let whole = fake_installer(&binary, "0.1.39");
    let half = whole[..whole.len() / 2].to_owned();
    let result = run_installer(&setup(&home, &binary, half), "0.1.39", &|_| {});
    assert!(result.as_ref().is_err_and(|reason| reason.contains("incompleto")), "{result:?}");
    assert_eq!(answer_of(&binary), "0.1.38");
    assert!(!home.join("install.cmd").exists(), "the cut installer was never written");
    assert!(!home.join(LOCK).exists());
    let mut previous = binary.into_os_string();
    previous.push(".anterior");
    assert!(!Path::new(&previous).exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asking_now_with_nowhere_to_ask_says_why_and_decides_nothing() {
    // No address to read: the answer is the reason, as the window's check would show it.
    let answer = check_now(&OTHER_APP);
    assert_eq!(answer.err().as_deref(), Some("a resposta não veio completa."));
}

#[test]
fn the_menu_beside_the_clock_says_the_update_and_installs_it() {
    use ravenrs_core::TrayEntry;
    let n = news("0.2.0");
    assert!(tray_entries(&UpdateStatus::Quiet, None).is_empty());
    assert!(tray_entries(&UpdateStatus::UpToDate, None).is_empty());

    let ready = tray_entries(&UpdateStatus::Ready(n.clone()), None);
    assert_eq!(
        ready[0],
        TrayEntry::item(TRAY_UPDATE, "Reiniciar para atualizar").detail("Versão 0.2.0 pronta")
    );
    assert_eq!(ready[1], TrayEntry::item(TRAY_NEWS, "Novidades da versão 0.2.0"));
    assert!(TRAY_NEWS.starts_with(ravenrs_core::OPENS_WINDOW), "the news open the window");

    let compiling = tray_entries(&UpdateStatus::Compiling(n.clone()), Some("compilando"));
    assert_eq!(
        compiling[0],
        TrayEntry::status("Atualizando para a versão 0.2.0…", Some("compilando".into()))
    );

    assert_eq!(
        tray_choice(&UpdateStatus::Available(n.clone()), TRAY_UPDATE),
        Some(TrayChoice::Install(n.clone()))
    );
    assert_eq!(
        tray_choice(&UpdateStatus::Failed(n.clone(), "x".into()), TRAY_UPDATE),
        Some(TrayChoice::Install(n.clone()))
    );
    assert_eq!(tray_choice(&UpdateStatus::Ready(n.clone()), TRAY_UPDATE), Some(TrayChoice::Reopen));
    assert_eq!(tray_choice(&UpdateStatus::Ready(n.clone()), TRAY_NEWS), Some(TrayChoice::News));
    // While compiling there is nothing to choose, and the app's own ids are not the update's.
    assert_eq!(tray_choice(&UpdateStatus::Compiling(n.clone()), TRAY_UPDATE), None);
    assert_eq!(tray_choice(&UpdateStatus::Ready(n), "run"), None);
    assert_eq!(Updater::idle(&TEST_APP).tray_select(TRAY_UPDATE), None, "nothing to install");
}
