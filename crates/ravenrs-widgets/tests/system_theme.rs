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
