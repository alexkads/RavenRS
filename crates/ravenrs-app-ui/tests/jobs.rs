//! An action whose work runs away from the screen, declared with no module and no screen of
//! its own, driven the way people use it.
#![allow(clippy::expect_used)]

use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use ravenrs_app::{Action, BusinessObject, ObjectSpace};
use ravenrs_app_ui::{App, Language};
use ravenrs_core::{AccessAction, AccessNode, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

#[derive(BusinessObject)]
#[object(caption = "Pastas", navigation = "Disco")]
struct Folder {
    #[field(key)]
    id: i64,
    #[field(caption = "Nome", default_property)]
    name: String,
    #[field(caption = "Tamanho")]
    size: i64,
}

fn nodes(ui: &mut Ui) -> Vec<AccessNode> {
    ui.access_tree().nodes
}

fn says(ui: &mut Ui, text: &str) -> bool {
    nodes(ui).iter().any(|n| n.name.as_deref() == Some(text) || n.value.as_deref() == Some(text))
}

fn press(ui: &mut Ui, name: &str) {
    let id = nodes(ui)
        .into_iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no button {name}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    settle(ui);
}

fn settle(ui: &mut Ui) {
    ui.update();
    ui.update();
}

/// Lets the screen be woken until this is true, as the window would.
fn wait_for(ui: &mut Ui, what: &str, done: impl Fn(&mut Ui) -> bool) {
    let start = Instant::now();
    while !done(ui) {
        assert!(start.elapsed() < Duration::from_secs(5), "never: {what}");
        std::thread::sleep(Duration::from_millis(2));
        settle(ui);
    }
}

fn app(gate: Arc<Barrier>, stop: bool) -> Ui {
    let view = App::new("Raven Sweep")
        .language(Language::BrazilianPortuguese)
        .entity::<Folder>()
        .action(move || {
            let gate = gate.clone();
            Action::background("Scan", "Varrer", move |job| {
                gate.wait();
                for (n, name) in ["Caches", "Logs"].into_iter().enumerate() {
                    job.progress((n + 1) as f32 / 2.0, format!("lendo {name}"));
                    let mut folder = ravenrs_app::Object::from_values("Folder", Default::default());
                    folder.set("id", (n + 1) as f64);
                    folder.set("name", name);
                    folder.set("size", 1024.0);
                    job.save(folder);
                }
                gate.wait();
                while stop && !job.cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(())
            })
            .target_entity("Folder")
        })
        .into_view();
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    settle(&mut ui);
    ui
}

#[test]
fn the_list_fills_in_as_the_work_hands_records_back_and_the_window_shows_how_far_it_got() {
    let gate = Arc::new(Barrier::new(2));
    let mut ui = app(gate.clone(), false);
    assert!(!says(&mut ui, "Caches"));
    press(&mut ui, "Varrer");
    let progress = |ui: &mut Ui| nodes(ui).into_iter().filter(|n| n.role == Role::ProgressIndicator).count();
    assert_eq!(progress(&mut ui), 1, "the window shows the work running");
    assert!(says(&mut ui, "Cancelar"));
    gate.wait();
    gate.wait();
    wait_for(&mut ui, "the records show", |ui| says(ui, "Caches") && says(ui, "Logs"));
    wait_for(&mut ui, "the work ends", |ui| progress(ui) == 0);
    assert!(!says(&mut ui, "Cancelar"));
    assert!(ui.next_wake().is_none(), "nothing keeps the screen awake once it ended");
}

#[test]
fn cancel_asks_the_work_to_stop_and_the_action_is_off_until_it_does() {
    let gate = Arc::new(Barrier::new(2));
    let mut ui = app(gate.clone(), true);
    press(&mut ui, "Varrer");
    gate.wait();
    gate.wait();
    let scan = nodes(&mut ui)
        .into_iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Varrer"))
        .expect("still shown");
    assert!(scan.disabled, "it does not run twice at once");
    press(&mut ui, "Cancelar");
    wait_for(&mut ui, "the work stops", |ui| !says(ui, "Cancelar") && !says(ui, "Parando…"));
    wait_for(&mut ui, "what it handed back stays", |ui| says(ui, "Logs"));
}

#[test]
fn a_space_of_its_own_sees_what_was_kept() {
    // What the work hands back is committed, not only shown.
    let database = ravenrs_app::MemoryDatabase::new(vec![Folder::entity_info()]).expect("valid");
    let kept = database.clone();
    let view = App::new("Raven Sweep")
        .entity::<Folder>()
        .storage(move || kept.space())
        .action(|| {
            Action::background("Scan", "Scan", |job| {
                let mut folder = ravenrs_app::Object::from_values("Folder", Default::default());
                folder.set("id", 1.0);
                folder.set("name", "Caches");
                folder.set("size", 1.0);
                job.save(folder);
                Ok(())
            })
            .target_entity("Folder")
        })
        .into_view();
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    settle(&mut ui);
    press(&mut ui, "Scan");
    wait_for(&mut ui, "the record shows", |ui| says(ui, "Caches"));
    let found = database.space().find("Folder", &ravenrs_app::Value::Number(1.0)).expect("reads");
    assert!(found.is_some());
}
