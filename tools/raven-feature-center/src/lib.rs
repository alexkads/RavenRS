//! The feature center: every component and every feature of the RavenRS, one page each, with what
//! to try (#579). It is also how to know it all still works: `tests/suite/tour.rs` goes through every
//! demonstration, as each of the three people, in memory and in SQLite.
//!
//! It opens in more than one way, chosen by [`Opening`] from the command line or the address of the
//! page (#587):
//!
//! - `completo`, the default: the whole center, after signing in as ana, bia or caio;
//! - `vitrine`: the same, in memory, with ana filled in at the door, for the site;
//! - `componentes`: only the pages of the components, with no application framework;
//! - `playground`: the examples, to edit and run.
//!
//! The Cargo features say what goes in the build: `app` (the center, its storage and its web API),
//! `widgets` (the pages of the components) and `playground` (the examples). A build of
//! `--no-default-features --features widgets` has no `ravenrs-app*` in it.

/// The scenarios of Raven Test Studio the tool brings, one per file of `tests/e2e/`: the Studio
/// gets them when it opens the tool with `--e2e-serve`, and the tests play them.
pub const SCENARIOS: [&str; 1] = [include_str!("../tests/e2e/abrir.raventest")];

pub mod opening;
pub use opening::{Look, Mode, Opening, Speech};

pub mod versao;
pub use versao::VERSION;

/// The center tests itself, as the installer and an agent ask it to: every way it opens is
/// built with no window and must draw something a screen reader reads. Returns what it checked.
pub fn self_test() -> Result<String, String> {
    let ways: &[&[&str]] =
        &[&[], &["--modo", "vitrine"], &["--modo", "componentes"], &["--modo", "playground"]];
    for way in ways {
        let opening = Opening::from_arguments(way.iter().map(|a| (*a).to_owned()))?;
        let mut ui = ravenrs::prelude::Ui::new(view(&opening), window_theme(&opening));
        ui.set_viewport(ravenrs::prelude::Size::new(1400.0, 900.0), 1.0);
        for _ in 0..4 {
            ui.update();
        }
        let nodes = ui.access_tree().nodes.len();
        if nodes < 3 {
            return Err(format!("{way:?} opened with {nodes} accessible nodes"));
        }
    }
    Ok(format!("Autoteste: {} modos abertos sem janela", ways.len()))
}

#[cfg(feature = "app")]
pub mod acesso;
#[cfg(feature = "app")]
pub mod actions;
#[cfg(feature = "app")]
mod centro;
#[cfg(feature = "app")]
pub mod contas;
#[cfg(feature = "app")]
pub mod fechamento;
#[cfg(feature = "app")]
pub mod menu;
#[cfg(feature = "app")]
pub mod model;
#[cfg(feature = "app")]
pub mod painel;
#[cfg(feature = "app")]
pub mod registro;
#[cfg(feature = "app")]
pub mod seed;
#[cfg(all(feature = "app", not(target_arch = "wasm32")))]
pub mod servico;
#[cfg(all(feature = "app", not(target_arch = "wasm32")))]
pub mod servidor;
#[cfg(feature = "app")]
pub mod vencimentos;
#[cfg(feature = "app")]
pub mod versao_registro;
#[cfg(feature = "app")]
pub use centro::*;

#[cfg(feature = "widgets")]
mod components;
#[cfg(feature = "widgets")]
pub mod widgets;
#[cfg(feature = "widgets")]
pub use components::ComponentsShell;

use ravenrs::prelude::*;

/// The words the widgets say, in the language asked for; Portuguese, as the center, unless told.
#[cfg(feature = "widgets")]
fn widget_texts(speech: Option<Speech>) -> Labels {
    use ravenrs_app_i18n::{Catalog, Language};
    let language = match speech {
        Some(Speech::English) => Language::English,
        _ => Language::BrazilianPortuguese,
    };
    Labels::from_texts(&Catalog::builtin().texts(language))
}

/// What `opening` asks for, or a message saying why it cannot be shown in this build.
pub fn view(opening: &Opening) -> AnyView {
    let made: Option<AnyView> = match opening.mode {
        #[cfg(feature = "app")]
        Mode::Whole | Mode::Showcase if opening.counter => Some(counter_window()),
        #[cfg(feature = "app")]
        Mode::Whole | Mode::Showcase => Some(opened(opening)),
        #[cfg(feature = "widgets")]
        Mode::Components => Some(Box::new(Localized::new(
            widget_texts(opening.speech),
            ComponentsShell::new(opening.page.as_deref(), opening.embedded),
        ))),
        #[cfg(feature = "playground")]
        Mode::Playground => Some(playground(opening)),
        #[allow(unreachable_patterns)]
        _ => None,
    };
    made.unwrap_or_else(|| {
        Box::new(Alert::new(
            "Raven Feature Center",
            "Este modo não veio nesta compilação: ligue a feature que ele pede (app, widgets ou playground).",
        ))
    })
}

/// The theme the window starts with. The center itself takes the one asked for inside, where each
/// person can change it; the other modes take it here, or the dark one.
pub fn window_theme(opening: &Opening) -> Theme {
    opening.look.map_or_else(Theme::raven_dark, Look::theme)
}

/// The examples, editable: the one of `componente` alone, or all of them opening on `pagina`.
/// They live in the component gallery until the batches W1-W9 of #579 bring them here.
#[cfg(feature = "playground")]
fn playground(opening: &Opening) -> AnyView {
    use component_gallery::samples::SamplePlayground;
    match opening.component.as_deref().and_then(SamplePlayground::new) {
        Some(one) => Box::new(one),
        None => Box::new(SamplePlayground::all(opening.page.as_deref())),
    }
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            let view = match opening() {
                Ok(opening) => super::view(&opening),
                Err(why) => {
                    Box::new(ravenrs::prelude::Alert::new("Raven Feature Center", why)) as super::AnyView
                }
            };
            let theme = opening()
                .map(|opening| super::window_theme(&opening))
                .unwrap_or_else(|_| super::Theme::raven_dark());
            let options = ravenrs::web::WebOptions::new("raven-canvas").theme(theme);
            match ravenrs::web::start(view, options).await {
                Ok(app) => std::mem::forget(app),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    /// The parameters of the address of the page. Only these names are read, and none of them is
    /// an address.
    fn opening() -> Result<super::Opening, String> {
        let search = web_sys::window().and_then(|window| window.location().search().ok()).unwrap_or_default();
        let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) else {
            return Ok(super::Opening::default());
        };
        let found: Vec<(&str, String)> = ["modo", "pagina", "componente", "tema", "idioma", "incorporado"]
            .into_iter()
            .filter_map(|name| params.get(name).map(|value| (name, value)))
            .collect();
        super::Opening::from_pairs(found.iter().map(|(name, value)| (*name, value.as_str())))
    }
}
