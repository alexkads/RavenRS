//! Themes and a field of squares, every feature (#579): the menu of the themes that come with
//! RavenRS, a menu of only some of them plus one of the application's own, the token editor
//! starting from that same theme, and a `QuadField` that stands up as the page scrolls.

use ravenrs::core::Color;
use ravenrs::prelude::*;

/// The control's name, as the session knows it. The center's own token editor is "ThemeEditor".
pub const ID: &str = "ThemesDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Temas e aparência";

/// The name of the application's own theme.
pub const FOREST: &str = "Floresta";

/// The application's own theme: the dark one, with a green accent.
pub fn forest() -> Theme {
    let mut theme = Theme::raven_dark();
    theme.name = FOREST;
    theme.set_color(Semantic::Accent, Color::hex(0x2F9E5B));
    theme
}

/// A green hill on a night sky, with a moon that comes and goes.
fn hill(u: f32, v: f32, time: f64) -> Color {
    let ridge = 0.65 - 0.15 * (u * std::f32::consts::PI * 2.0).sin();
    let moon = (u - 0.8).powi(2) + (v - 0.2).powi(2) < 0.006 + 0.002 * (time as f32).sin();
    if v > ridge {
        Color::hex(0x2F9E5B)
    } else if moon {
        Color::hex(0xF2E9C9)
    } else {
        Color::hex(0x101826)
    }
}

/// The page: the menus, the editor and the field, and a line with the theme on screen.
#[derive(Debug, Default)]
pub struct ThemesPage;

impl View for ThemesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let on = cx.theme().name;
        let menus = HStack::new((
            VStack::new((Text::new("Os que vêm com o RavenRS"), ThemePicker::new())).spacing(6.0),
            VStack::new((
                Text::new("Só claro e escuro, mais o da aplicação"),
                ThemePicker::of([Theme::raven_light(), Theme::raven_dark()]).theme(forest()),
            ))
            .spacing(6.0),
        ))
        .spacing(24.0)
        .wrap();
        cx.child(
            &VStack::new((
                Text::new("Menus de temas").style(TextStyle::Headline),
                menus,
                Text::new(format!("Tema na tela: {on}")),
                Text::new("Campo de quadrados").style(TextStyle::Headline),
                Text::new("Deitado como um chão que ondula; de pé ao passar pela tela.")
                    .foreground(Semantic::InkMuted),
                QuadField::new(48, 27, hill)
                    .height(if narrow { 160.0 } else { 240.0 })
                    .range(ScrollRange::Through),
                Text::new("Editor de tokens, a partir do tema Floresta").style(TextStyle::Headline),
                ThemeEditor::new().theme(forest()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
