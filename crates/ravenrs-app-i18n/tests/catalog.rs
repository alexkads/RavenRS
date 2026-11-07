//! The catalog: every language has every text, and what an application gives goes over it.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use ravenrs_app_i18n::{Catalog, FALLBACK, Language, Texts, format, model};

#[test]
fn every_text_that_comes_with_it_is_in_every_language() {
    let catalog = Catalog::builtin();
    for language in Language::ALL {
        assert_eq!(catalog.missing(language.code()), Vec::<String>::new(), "{language}");
        let extra: Vec<_> =
            catalog.keys(language.code()).filter(|key| catalog.own_text(FALLBACK, key).is_none()).collect();
        assert!(extra.is_empty(), "{language} has keys English does not: {extra:?}");
    }
}

#[test]
fn the_placeholders_are_the_same_in_every_language() {
    let catalog = Catalog::builtin();
    let holes = |text: &str| (0..10).filter(|i| text.contains(&format!("{{{i}}}"))).collect::<Vec<_>>();
    // A date is written in another order, and with other parts, in each language.
    const ANY_PARTS: &[&str] = &["widgets.short_date"];
    for key in catalog.keys(FALLBACK).filter(|key| !ANY_PARTS.contains(key)) {
        let english = holes(catalog.own_text(FALLBACK, key).unwrap());
        for language in Language::ALL {
            assert_eq!(
                holes(catalog.own_text(language.code(), key).unwrap()),
                english,
                "{key} in {language}"
            );
        }
    }
}

#[test]
fn a_language_without_a_key_falls_back_to_english_and_then_to_the_key() {
    let catalog = Catalog::new().with("en", "shop.cart", "Cart").with("pt-BR", "shop.pay", "Pagar");
    let texts = catalog.texts(Language::BrazilianPortuguese);
    assert_eq!(texts.get("shop.pay"), "Pagar");
    assert_eq!(texts.get("shop.cart"), "Cart");
    assert_eq!(texts.get("shop.nobody"), "shop.nobody");
    let spanish = Texts::of(catalog.with("es", "shop.cart", "Carrito"), "es");
    assert_eq!(spanish.get("shop.cart"), "Carrito");
    assert_eq!(spanish.language(), "es");
}

#[test]
fn json_replaces_and_adds_and_says_what_is_wrong() {
    let mut catalog = Catalog::builtin();
    catalog.merge_json("pt-BR", r#"{ "ui.save": "Gravar", "shop.pay": "Pagar" }"#).expect("reads");
    let texts = catalog.texts(Language::BrazilianPortuguese);
    assert_eq!(texts.get("ui.save"), "Gravar");
    assert_eq!(texts.get("ui.cancel"), "Cancelar");
    assert_eq!(texts.get("shop.pay"), "Pagar");

    let error = catalog.merge_json("pt-BR", r#"{ "ui.cancel": "Desistir", "ui.next": 3 }"#).unwrap_err();
    assert!(error.0.contains("`ui.next` is not text"), "{error}");
    assert_eq!(catalog.texts(Language::BrazilianPortuguese).get("ui.cancel"), "Cancelar", "nothing was put");
    assert!(catalog.merge_json("pt-BR", "[1]").is_err());
    assert!(catalog.merge_json("pt-BR", "{").is_err());
}

#[test]
fn merging_catalogs_puts_the_other_on_top() {
    let mut mine = Catalog::new().with("en", "a", "1").with("en", "b", "2");
    mine.merge(&Catalog::new().with("en", "b", "two").with("pt-BR", "a", "um"));
    assert_eq!(mine.text("en", "b"), Some("two"));
    assert_eq!(mine.text("pt-BR", "a"), Some("um"));
    assert_eq!(mine.text("pt-BR", "b"), Some("two"));
    assert_eq!(mine.languages().collect::<Vec<_>>(), ["en", "pt-BR"]);
}

#[test]
fn format_fills_the_numbered_holes() {
    assert_eq!(format("{0} cannot be below {1}", &["Age", "18"]), "Age cannot be below 18");
    assert_eq!(format("{1}{0}{1}", &["a", "b"]), "bab");
    assert_eq!(format("{2} and {x} and {", &["a"]), "{2} and {x} and {");
    let texts = Texts::new(Language::English);
    assert_eq!(texts.format("rules.too_high", &["Age", "99"]), "Age cannot be above 99");
    assert_eq!(texts.char("format.decimal", '?'), '.');
    assert_eq!(Texts::new(Language::BrazilianPortuguese).char("format.decimal", '?'), ',');
    assert_eq!(texts.char("nothing", '?'), '?');
    let written = ravenrs_app_i18n::format_with("{1}-{0}-{7}", |index, out| match index {
        0 => {
            out.push('a');
            true
        }
        1 => {
            out.push_str("bb");
            true
        }
        _ => false,
    });
    assert_eq!(written, "bb-a-{7}");
}

#[test]
fn languages_go_by_their_codes() {
    for language in Language::ALL {
        assert_eq!(Language::from_code(language.code()), Some(*language));
    }
    assert_eq!(Language::from_code("pt_br"), Some(Language::BrazilianPortuguese));
    assert_eq!(Language::from_code("PT"), Some(Language::BrazilianPortuguese));
    assert_eq!(Language::from_code("en-US"), Some(Language::English));
    assert_eq!(Language::from_code("es"), None);
    assert_eq!(Language::BrazilianPortuguese.to_string(), "Português (Brasil)");
}

#[test]
fn the_translations_of_the_model_are_json() {
    for language in Language::ALL {
        for json in [model::security(*language), model::reports(*language)].into_iter().flatten() {
            serde_json::from_str::<serde_json::Value>(json).expect("reads");
        }
    }
    assert!(model::security(Language::English).is_none());
}
