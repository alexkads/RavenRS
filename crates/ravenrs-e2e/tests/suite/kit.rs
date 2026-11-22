//! The components of `ravenrs-widgets-kit` in the testability contract (#738): tabs, steps,
//! sections and files are found by what they are, and what opens does it by the action.

use ravenrs_core::{AccessAction, Size};
use ravenrs_e2e::E2e;
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{Disclosure, Tabs, Text, TextArea, VStack, View, ViewExt};

fn session(view: impl View + 'static) -> E2e {
    E2e::new(view, Theme::raven_dark()).viewport(Size::new(1000.0, 760.0), 1.0)
}

#[test]
fn a_tab_is_chosen_by_its_label_wherever_it_is() {
    let tab = State::new(0_usize);
    let names = State::new(vec!["Geral", "Cobrança", "Equipe"]);
    let (t, n) = (tab.clone(), names.clone());
    let view = ravenrs_widgets::view_fn(move || {
        n.get().into_iter().fold(Tabs::new(t.binding()), |tabs, name| tabs.item(name))
    });
    let mut e2e = session(view);
    e2e.by_test_id("tab:Equipe").perform(AccessAction::Select).unwrap();
    assert_eq!(tab.get(), 2);
    // The tabs change order: the same id still finds "Equipe", now first.
    names.set(vec!["Equipe", "Geral", "Cobrança"]);
    e2e.advance(0.1);
    e2e.by_test_id("tab:Geral").click().unwrap();
    assert_eq!(tab.get(), 1);
    assert_eq!(e2e.by_test_id("tab:Equipe").node().unwrap().name.as_deref(), Some("Equipe"));
}

#[test]
fn a_disclosure_opens_and_closes_by_the_action_and_a_text_area_takes_its_text() {
    let notes = State::new(String::new());
    let view = VStack::new((
        Disclosure::new("Detalhes", Text::new("Mais sobre o pedido.")).test_id("detalhes"),
        TextArea::new(notes.binding()).test_id("notas"),
    ));
    let mut e2e = session(view);
    e2e.by_test_id("detalhes").expand().unwrap();
    assert_eq!(e2e.by_test_id("detalhes").node().unwrap().expanded, Some(true));
    e2e.by_test_id("detalhes").collapse().unwrap();
    assert_eq!(e2e.by_test_id("detalhes").node().unwrap().expanded, Some(false));
    e2e.by_test_id("notas").set_value("duas\nlinhas").unwrap();
    assert_eq!(notes.get(), "duas\nlinhas");
}
