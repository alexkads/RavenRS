//! The theme following the system between light and dark: each test fails if its feature goes.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn ui(theme: Theme) -> Ui {
    Ui::new(Text::new("Relatório"), theme)
}

#[test]
fn the_system_does_not_change_a_theme_that_does_not_follow_it() {
    let mut ui = ui(Theme::raven_glass());
    ui.set_system_dark(false);
    assert_eq!(ui.theme().name, Theme::raven_glass().name);
    assert_eq!(ui.system_appearance().dark(), Some(false));
}

#[test]
fn following_the_system_matches_it_now_and_on_every_change() {
    let mut ui = ui(Theme::raven_glass());
    ui.follow_system_theme(true);
    // Until the platform says what the system is, the theme stays.
    assert_eq!(ui.theme().name, Theme::raven_glass().name);
    ui.set_system_dark(true);
    assert_eq!(ui.theme().name, Theme::raven_dark().name);
    ui.set_system_dark(false);
    assert_eq!(ui.theme().name, Theme::raven_light().name);
}

#[test]
fn choosing_a_theme_by_hand_stops_following_the_system() {
    let mut ui = ui(Theme::raven_dark());
    ui.set_system_dark(true);
    ui.follow_system_theme(true);
    let (appearance, state) = (ui.system_appearance(), ui.theme_state());
    // What the theme menu does when a theme is chosen.
    appearance.set_follow(false, &state);
    state.set(Theme::raven_glass());
    ui.set_system_dark(false);
    assert!(!ui.system_appearance().follows());
    assert_eq!(ui.theme().name, Theme::raven_glass().name);
}

#[test]
fn an_application_s_own_pair_replaces_raven_s_when_following() {
    let light = Theme { name: "Morning Star", ..Theme::raven_light() };
    let dark = Theme { name: "Vigil", ..Theme::raven_dark() };
    let mut ui = ui(light);
    ui.system_themes(light, dark);
    ui.follow_system_theme(true);
    ui.set_system_dark(true);
    assert_eq!(ui.theme().name, "Vigil");
    ui.set_system_dark(false);
    assert_eq!(ui.theme().name, "Morning Star");
    // Given while following, the pair goes on at once.
    ui.system_themes(Theme::raven_glass(), dark);
    assert_eq!(ui.theme().name, Theme::raven_glass().name);
}

/// A dialog open over a full screen in glass paints a nearly opaque surface, so the screen
/// behind never crosses its text; cards in the page stay a veil (#978).
#[test]
fn a_dialog_in_glass_is_nearly_opaque_over_the_screen_behind() {
    use ravenrs_core::{Brush, Primitive, Size};
    use ravenrs_reactive::State;
    let open = State::new(true);
    let view = VStack::new((
        Text::new("Relatório de vendas por região, com bandas, campos e fórmulas atrás do diálogo"),
        Dialog::new(open.binding(), "Novo relatório").content(Text::new("Comece por um modelo pronto")),
    ));
    let glass = Theme::raven_glass();
    let mut ui = Ui::new(view, glass);
    ui.set_viewport(Size::new(900.0, 600.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let want = glass.floating_surface();
    let found = ui.scene().primitives().iter().any(|p| {
        matches!(p, Primitive::Quad(q) if matches!(q.fill, Brush::Solid(c) if c == want) && q.rect.width() > 200.0)
    });
    assert!(found, "the dialog paints the floating surface");
    assert!(want.a >= ravenrs_style::FLOATING_OPACITY, "{want:?}");
}
