//! `Catalog`, every feature: the texts that come with the framework, by language and key, what a
//! language still misses, a text of its own told from one that falls back on English, and texts
//! put on top from JSON, from another catalog or one by one.

use ravenrs::prelude::*;
use ravenrs_app_i18n::Catalog;
use ravenrs_app_ui::Session;

use super::{button, layout};

/// The control's name, as the session knows it.
pub const ID: &str = "CatalogDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Catálogo de textos";

const PT: &str = "pt-BR";

/// A catalog an application would bring: a text in two languages, and one only in English.
fn own() -> Catalog {
    Catalog::new().with(PT, "center.bye", "Até logo!").with("en", "center.bye", "See you!").with(
        "en",
        "center.only_english",
        "Only in English",
    )
}

/// What a catalog has, as the screen lists it.
fn described(catalog: &Catalog) -> Vec<String> {
    let languages: Vec<&str> = catalog.languages().collect();
    let missing = catalog.missing(PT);
    let shown = |key: &str| {
        let own = catalog.own_text(PT, key).unwrap_or("nenhum");
        let fallback = catalog.text(PT, key).unwrap_or("nenhum");
        format!("{key}: próprio do pt-BR {own}; com o inglês por trás {fallback}")
    };
    vec![
        format!("Idiomas: {}", languages.join(", ")),
        format!("Chaves em pt-BR: {}; em inglês: {}", catalog.keys(PT).count(), catalog.keys("en").count()),
        format!(
            "Faltam em pt-BR: {}{}",
            missing.len(),
            missing.first().map_or_else(String::new, |key| format!(", a primeira é {key}"))
        ),
        shown("ui.refresh"),
        shown("center.hello"),
        shown("center.bye"),
        shown("center.only_english"),
    ]
}

/// The screen.
struct CatalogScreen;

impl View for CatalogScreen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let catalog = cx.use_state(Catalog::builtin);
        let said = cx.use_state(|| "Este é o catálogo que vem com o framework.".to_owned());
        let act = |caption: &str, change: fn(&mut Catalog) -> String| {
            let (catalog, said) = (catalog.clone(), said.clone());
            button(caption, move || {
                let mut changed = catalog.get();
                said.set(change(&mut changed));
                catalog.set(changed);
            })
        };
        let buttons = vec![
            act("Juntar o JSON do centro", |catalog| match catalog.merge_json(PT, crate::model::TEXTS) {
                Ok(()) => "O JSON do centro trocou ui.refresh e trouxe center.hello.".to_owned(),
                Err(why) => format!("O JSON não serve: {why}"),
            }),
            act("Juntar o JSON errado", |catalog| match catalog.merge_json(PT, r#"{ "ui.save": 1 }"#) {
                Ok(()) => "Juntou.".to_owned(),
                Err(why) => format!("Recusado, nada mudou: {why}"),
            }),
            act("Juntar o catálogo próprio", |catalog| {
                catalog.merge(&own());
                "O catálogo próprio trouxe center.bye nos dois idiomas e um texto só em inglês.".to_owned()
            }),
            act("Pôr um texto", |catalog| {
                catalog.set(PT, "ui.refresh", "Ler de novo");
                "ui.refresh agora diz Ler de novo.".to_owned()
            }),
            act("Recomeçar", |catalog| {
                *catalog = Catalog::builtin();
                "De volta ao catálogo do framework.".to_owned()
            }),
        ];
        let intro = "Os textos das telas vêm de um catálogo por idioma e chave, com o inglês por trás. \
                     Junte textos e veja o que muda em cada chave.";
        let view = layout(cx, intro, buttons, described(&catalog.get()), said.get());
        cx.child(view.as_ref())
    }
}

/// Registers the screen with the session.
pub fn register(session: &Session) {
    session.register_control(ID, || Box::new(CatalogScreen) as AnyView);
}
