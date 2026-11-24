//! Each way the center opens (#587), as a person sees it: the showcase of the site, a page opened
//! from the start, only the components, the playground, a language and a theme.
#![allow(clippy::expect_used, clippy::panic)]

use raven_feature_center::{Look, Mode, Opening, Speech};
use ravenrs::core::{AccessAction, Role};
use ravenrs::prelude::*;

struct Window(Ui);

impl Window {
    fn of(view: AnyView) -> Self {
        Self::sized(view, 1400.0)
    }

    fn sized(view: AnyView, width: f32) -> Self {
        let mut ui = Ui::new(view, Theme::raven_dark());
        ui.set_viewport(Size::new(width, 900.0), 1.0);
        let mut window = Self(ui);
        let _ = window.texts();
        window
    }

    fn texts(&mut self) -> Vec<String> {
        for _ in 0..4 {
            self.0.update();
        }
        self.0
            .access_tree()
            .nodes
            .iter()
            .flat_map(|node| [node.name.clone(), node.value.clone(), node.description.clone()])
            .flatten()
            .collect()
    }

    fn says(&mut self, text: &str) -> bool {
        self.texts().iter().any(|found| found.contains(text))
    }

    fn has_button(&mut self, name: &str) -> bool {
        let _ = self.texts();
        self.0.access_tree().nodes.iter().any(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
    }

    /// The nodes that reach past `width` on the right, by role and name.
    fn past(&mut self, width: f32) -> Vec<String> {
        let _ = self.texts();
        self.0
            .access_tree()
            .nodes
            .iter()
            .filter(|n| n.bounds.width() > 0.0 && n.bounds.max_x() > width + 0.5)
            .map(|n| {
                format!("{:?} {:?} até {:.0}", n.role, n.name.as_deref().unwrap_or(""), n.bounds.max_x())
            })
            .collect()
    }

    fn press(&mut self, name: &str) {
        let _ = self.texts();
        let tree = self.0.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name}: {:?}", self.texts())).id;
        self.0.perform_action(id, AccessAction::Click);
        let _ = self.texts();
    }

    fn type_into(&mut self, field: &str, text: &str) {
        let _ = self.texts();
        let tree = self.0.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
            .unwrap_or_else(|| panic!("no field {field}: {:?}", self.texts()))
            .id;
        self.0.perform_action(id, AccessAction::Focus);
        for character in text.chars() {
            self.0.handle_event(&ravenrs::core::Event::KeyDown {
                key: ravenrs::core::Key::Character(character.to_string()),
                modifiers: ravenrs::core::Modifiers::default(),
            });
        }
        let _ = self.texts();
    }
}

fn opening(mode: Mode) -> Opening {
    Opening { mode, ..Opening::default() }
}

#[test]
fn the_showcase_fills_ana_in_says_so_and_starts_over_each_time() {
    let mut window = Window::of(raven_feature_center::view(&opening(Mode::Showcase)));
    assert!(window.says("Demonstração"), "it says it is a demonstration: {:?}", window.texts());
    assert!(window.says("ana"), "ana is filled in");
    window.press("Entrar");
    assert!(window.says("Regras") && window.says("Usuários"), "inside, as ana: {:?}", window.texts());
    window.press("Países");
    window.press("Novo");
    window.type_into("Nome", "Atlântida");
    window.press("Salvar");
    window.press("Países");
    assert!(window.says("Atlântida"), "kept while it is open: {:?}", window.texts());

    let mut again = Window::of(raven_feature_center::view(&opening(Mode::Showcase)));
    again.press("Entrar");
    again.press("Países");
    assert!(!again.says("Atlântida"), "in memory: the next one starts over, on the desktop too");
}

#[test]
fn a_page_asked_for_opens_once_inside_and_an_unknown_one_says_so() {
    let mut plain = Window::of(raven_feature_center::view(&opening(Mode::Showcase)));
    plain.press("Entrar");
    assert!(!plain.says("Mostrando Completa"), "the center does not open there by itself");
    let asked = Opening { page: Some("AuditFull_ListView".into()), ..opening(Mode::Showcase) };
    let mut window = Window::of(raven_feature_center::view(&asked));
    window.press("Entrar");
    assert!(window.says("Mostrando Completa"), "{:?}", window.texts());

    let component = Opening { page: Some("CablePlugDemo".into()), ..opening(Mode::Showcase) };
    let mut window = Window::of(raven_feature_center::view(&component));
    window.press("Entrar");
    let first = raven_feature_center::widgets::graficos::cabos::USES[0];
    assert!(window.says(first), "a component's page opens by its id: {:?}", window.texts());

    let wrong = Opening { page: Some("NaoExiste_ListView".into()), ..opening(Mode::Showcase) };
    let mut window = Window::of(raven_feature_center::view(&wrong));
    window.press("Entrar");
    assert!(window.says("NaoExiste_ListView"), "the screen says what was not found: {:?}", window.texts());
    assert!(window.says("Regras"), "and the center is still there");
}

#[test]
fn only_the_components_with_no_door_and_a_list_on_a_phone() {
    let mut window = Window::of(raven_feature_center::view(&opening(Mode::Components)));
    assert!(!window.says("Senha"), "no sign-in: {:?}", window.texts());
    for page in raven_feature_center::widgets::PAGES {
        assert!(window.has_button(page.caption), "{}: {:?}", page.caption, window.texts());
    }
    window.press("Cabos");
    assert!(window.says(raven_feature_center::widgets::graficos::cabos::USES[0]), "{:?}", window.texts());

    let mut phone = Window::sized(raven_feature_center::view(&opening(Mode::Components)), 375.0);
    assert!(phone.says("Componente"), "a list to pick from: {:?}", phone.texts());
    assert!(!phone.has_button("Cabos"), "no sidebar on a phone");
}

#[test]
fn embedded_is_only_the_page() {
    let embedded =
        Opening { page: Some("CablePlugDemo".into()), embedded: true, ..opening(Mode::Components) };
    let mut window = Window::of(raven_feature_center::view(&embedded));
    assert!(window.says(raven_feature_center::widgets::graficos::cabos::USES[0]), "{:?}", window.texts());
    assert!(!window.has_button("Diagrama de fluxo"), "no navigation around it");

    let dock = Opening { page: Some("DockAreaDemo".into()), embedded: true, ..opening(Mode::Components) };
    let mut window = Window::of(raven_feature_center::view(&dock));
    assert!(window.says("Editor de código"), "the dock opens by its id: {:?}", window.texts());
    let narrow = Opening { page: Some("RibbonDemo".into()), embedded: true, ..opening(Mode::Components) };
    let mut phone = Window::sized(raven_feature_center::view(&narrow), 375.0);
    assert!(phone.says("Página aberta: Início"), "the ribbon on a phone: {:?}", phone.texts());
    assert!(phone.says("Diga o que você deseja fazer"), "the widgets speak Portuguese: {:?}", phone.texts());
    let english = Opening { speech: Some(Speech::English), ..narrow };
    let mut window = Window::of(raven_feature_center::view(&english));
    assert!(window.says("Tell me what you want to do"), "and English when asked: {:?}", window.texts());
}

#[test]
fn the_playground_opens_on_the_example_of_a_component() {
    let one = Opening { component: Some("Button".into()), ..opening(Mode::Playground) };
    let mut window = Window::of(raven_feature_center::view(&one));
    assert!(window.says("Button"), "{:?}", window.texts());
    let all = Window::of(raven_feature_center::view(&opening(Mode::Playground))).texts();
    assert!(all.len() > 20, "every example: {all:?}");
}

#[test]
fn another_language_and_another_theme() {
    let english = Opening { speech: Some(Speech::English), ..opening(Mode::Showcase) };
    let mut window = Window::of(raven_feature_center::view(&english));
    assert!(window.has_button("Sign in"), "{:?}", window.texts());
    window.press("Sign in");
    assert!(window.says("Sign out"), "inside, in English: {:?}", window.texts());

    let light = Opening { look: Some(Look::Light), ..opening(Mode::Components) };
    assert_eq!(raven_feature_center::window_theme(&light), Theme::raven_light());
    assert_eq!(raven_feature_center::window_theme(&Opening::default()), Theme::raven_dark());
}

#[test]
fn the_news_of_the_version_fit_a_phone_with_the_dialog_open() {
    let news = Opening { page: Some("NewsDemo".into()), embedded: true, ..opening(Mode::Components) };
    let mut phone = Window::sized(raven_feature_center::view(&news), 375.0);
    assert!(phone.has_button("Detalhes de uma falha"), "{:?}", phone.texts());
    let past = phone.past(375.0);
    assert!(past.is_empty(), "past 375 pt: {past:?}");
    phone.press("Versão em uso");
    assert!(phone.has_button("Fechar"), "the news are open: {:?}", phone.texts());
    let past = phone.past(375.0);
    assert!(past.is_empty(), "past 375 pt with the news open: {past:?}");
    phone.press("Fechar");
    phone.press("Detalhes de uma falha");
    assert!(phone.has_button("Ver como atualizar à mão"), "the details are open: {:?}", phone.texts());
    let past = phone.past(375.0);
    assert!(past.is_empty(), "past 375 pt with the details open: {past:?}");
    // On a desktop the three buttons stay on one line.
    let mut desktop = Window::of(raven_feature_center::view(&news));
    let _ = desktop.texts();
    let tree = desktop.0.access_tree();
    let tops: Vec<f32> = ["Versão em uso", "Mais recente", "Detalhes de uma falha"]
        .iter()
        .map(|name| {
            let found = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
            found.unwrap_or_else(|| panic!("{name}")).bounds.y()
        })
        .collect();
    assert!(tops.windows(2).all(|w| (w[0] - w[1]).abs() < 0.5), "one line on a desktop: {tops:?}");
}
