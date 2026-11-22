//! The Studio's menu beside the clock (#643): how its own test went, running it again from the
//! menu with the window closed, and an update that is ready.

use std::time::{Duration, Instant};

use raven_test_studio::tray::{RUN_SELF_TEST, TrayLink};
use raven_test_studio::{app_in_tray, versao};
use ravenrs::core::{TrayEntry, TrayMenu};
use ravenrs::prelude::*;
use ravenrs_app_update::{News, TRAY_UPDATE, UpdateStatus};
use ravenrs_e2e::E2e;

fn title(entries: &[TrayEntry]) -> String {
    match entries.first() {
        Some(TrayEntry::Status { title, .. }) => title.clone(),
        other => panic!("the menu starts with the state, not {other:?}"),
    }
}

/// Builds the window again, as the platform does after a choice of the menu.
fn build(e2e: &mut E2e) {
    e2e.ui_mut().ui_waker().wake();
    e2e.advance(0.0);
}

#[test]
fn the_menu_says_the_self_test_runs_it_again_and_offers_the_update() {
    let link = TrayLink::new(TrayMenu::default());
    let updater = versao::idle();
    updater.set_status(UpdateStatus::Ready(News {
        versao: "9.9.9".into(),
        data: None,
        titulo: "t".into(),
        importante: false,
        novidades: vec![],
        por_que_atualizar: "p".into(),
        contribuidores: vec![],
        issues: vec![],
    }));
    let mut studio = E2e::new(app_in_tray(link.clone(), updater), Theme::raven_dark());
    let entries = link.menu().entries();
    assert_eq!(title(&entries), "Autoteste ainda não rodou");
    assert!(entries.contains(&TrayEntry::item(RUN_SELF_TEST, "Rodar autoteste")));
    assert!(
        entries.contains(
            &TrayEntry::item(TRAY_UPDATE, "Reiniciar para atualizar").detail("Versão 9.9.9 pronta")
        ),
        "{entries:?}"
    );

    link.choose(RUN_SELF_TEST);
    build(&mut studio);
    let running = link.menu().entries();
    assert_eq!(title(&running), "Rodando o autoteste…");
    assert!(
        running.contains(&TrayEntry::item(RUN_SELF_TEST, "Rodar autoteste").enabled(false)),
        "{running:?}"
    );

    let started = Instant::now();
    while title(&link.menu().entries()) == "Rodando o autoteste…" {
        assert!(started.elapsed() < Duration::from_secs(120), "the self-test never ended");
        std::thread::sleep(Duration::from_millis(50));
        studio.advance(0.05);
    }
    let done = link.menu().entries();
    assert_eq!(title(&done), "Autoteste: 18 de 18 passos passaram");
    assert!(matches!(&done[0], TrayEntry::Status { detail: Some(d), .. } if d == "passou"), "{done:?}");
}
