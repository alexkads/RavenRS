//! Words of the screens and of the components, from the catalog and from what the application gives.
#![allow(clippy::expect_used, clippy::panic)]

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind};
use ravenrs_app_ui::{App, Language, Texts, Words};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

fn account() -> EntityInfo {
    EntityInfo::new("Account")
        .navigation("Cadastros")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
}

fn names(ui: &mut Ui) -> Vec<String> {
    ui.update();
    ui.access_tree().nodes.iter().filter_map(|node| node.name.clone()).collect()
}

#[test]
fn the_words_of_each_language_come_from_the_catalog() {
    assert_eq!(Words::pt_br().save, Texts::new(Language::BrazilianPortuguese).get("ui.save"));
    assert_eq!(Words::default().save, "Save");
    assert_eq!(Words::pt_br().thousands, '.');
    assert_eq!(Words::pt_br().rules.decimal, ',');
}

#[test]
fn texts_replace_words_of_the_screens_and_of_the_components() {
    let window = App::new("Contas")
        .language(Language::BrazilianPortuguese)
        .entity_info(account())
        .texts("pt-BR", r#"{ "ui.new": "Incluir"}"#)
        .build()
        .expect("builds");
    let mut ui = Ui::new(window, Theme::raven_dark());
    let shown = names(&mut ui);
    assert!(shown.iter().any(|name| name == "Incluir"), "{shown:?}");
    assert!(!shown.iter().any(|name| name == "Novo"), "{shown:?}");
}

#[test]
fn texts_that_do_not_read_say_so_before_any_screen() {
    let Err(error) = App::new("Contas").entity_info(account()).texts("pt-BR", r#"{ "ui.new": 1 }"#).build()
    else {
        panic!("it built");
    };
    assert!(error.0.contains("`ui.new` is not text"), "{}", error.0);
}
