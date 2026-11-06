//! The look of the app: lime for money that grows on a bright ground, the default, and on a
//! lively graphite for whoever prefers it dark. The rest of the shared scales come from RavenRS.

use ravenrs::prelude::*;
use ravenrs::style::Palette;

/// The default: near-white with a touch of lime, and lime for what matters on a screen.
pub fn light() -> Theme {
    Theme {
        name: "Raven Finanças Claro",
        palette: Palette {
            bg_000: Color::hex(0xf4f9e6),
            bg_100: Color::hex(0xffffff),
            bg_200: Color::hex(0xffffff),
            bg_300: Color::hex(0xe9f3d2),
            border: Color::hex(0xd8e6ba),
            ink: Color::hex(0x15230d),
            ink_muted: Color::hex(0x4c5c3d),
            accent: Color::hex(0x7ac70c),
            accent_hover: Color::hex(0x8bd61e),
            accent_pressed: Color::hex(0x6ab006),
            on_accent: Color::hex(0x142204),
            focus_ring: Color::hex(0x4d7c0f),
            success: Color::hex(0x15803d),
            warning: Color::hex(0xb45309),
            danger: Color::hex(0xc81e3a),
        },
        ..Theme::raven_light()
    }
}

/// Medium grays rather than near-black, so that it does not feel like the middle of the night.
pub fn dark() -> Theme {
    Theme {
        name: "Raven Finanças Escuro",
        palette: Palette {
            bg_000: Color::hex(0x2c2f33),
            bg_100: Color::hex(0x34373c),
            bg_200: Color::hex(0x3c4046),
            bg_300: Color::hex(0x4a4f55),
            border: Color::hex(0x585e65),
            ink: Color::hex(0xf6f8f2),
            ink_muted: Color::hex(0xc9cfcc),
            accent: Color::hex(0xb2ea3c),
            accent_hover: Color::hex(0xc3f25c),
            accent_pressed: Color::hex(0x9ad42a),
            on_accent: Color::hex(0x1c2b06),
            focus_ring: Color::hex(0xd4f57f),
            success: Color::hex(0x5ee08f),
            warning: Color::hex(0xffc94d),
            danger: Color::hex(0xff9c92),
        },
        ..Theme::raven_dark()
    }
}

/// Translucent surfaces with the same lime, over the same graphite.
pub fn glass() -> Theme {
    let raven = Theme::raven_glass();
    let dark = dark().palette;
    Theme {
        name: "Raven Finanças Vidro",
        palette: Palette {
            bg_000: dark.bg_000,
            ink: dark.ink,
            ink_muted: dark.ink_muted,
            accent: dark.accent,
            accent_hover: dark.accent_hover,
            accent_pressed: dark.accent_pressed,
            on_accent: dark.on_accent,
            focus_ring: dark.focus_ring,
            success: dark.success,
            warning: dark.warning,
            danger: dark.danger,
            ..raven.palette
        },
        ..raven
    }
}

/// The theme selected on this computer, or light on the first launch.
pub fn remembered() -> Theme {
    read_theme(&crate::paths::home().join("theme"))
}

fn read_theme(path: &std::path::Path) -> Theme {
    match std::fs::read_to_string(path).as_deref() {
        Ok("dark") => dark(),
        Ok("glass") => glass(),
        _ => light(),
    }
}

/// Remembers the theme picked in Ajustes for the next launch.
pub fn remember(theme: Theme) {
    let name = if theme.name == dark().name {
        "dark"
    } else if theme.name == glass().name {
        "glass"
    } else {
        "light"
    };
    let folder = crate::paths::home();
    if let Err(error) =
        std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(folder.join("theme"), name))
    {
        tracing::warn!(%error, "could not save the selected theme");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_saved_themes_and_defaults_to_light() {
        let path = std::env::temp_dir().join(format!("raven-theme-{}", uuid::Uuid::new_v4()));
        assert_eq!(read_theme(&path).name, light().name);
        for (saved, expected) in [("dark", dark()), ("light", light()), ("glass", glass())] {
            std::fs::write(&path, saved).unwrap();
            assert_eq!(read_theme(&path).name, expected.name);
        }
        std::fs::write(&path, "invalid").unwrap();
        assert_eq!(read_theme(&path).name, light().name);
        std::fs::remove_file(path).unwrap();
    }

    fn over(color: Color, ground: Color) -> Color {
        ground.lerp(color.with_alpha(1.0), color.a)
    }

    /// The same promise the themes of RavenRS make: 4.5:1 for text on its grounds.
    #[test]
    fn text_stays_legible_in_each_theme() {
        for theme in [light(), dark(), glass()] {
            let p = theme.palette;
            let (card, control) = (over(p.bg_200, p.bg_000), over(p.bg_300, p.bg_000));
            for (name, ink, ground) in [
                ("ink/canvas", p.ink, p.bg_000),
                ("ink/card", p.ink, card),
                ("muted/canvas", p.ink_muted, p.bg_000),
                ("muted/card", p.ink_muted, card),
                ("muted/control", p.ink_muted, control),
                ("on-accent/accent", p.on_accent, p.accent),
                ("success/card", p.success, card),
                ("warning/card", p.warning, card),
                ("danger/card", p.danger, card),
            ] {
                let ratio = ink.contrast_ratio(ground);
                assert!(ratio >= 4.5, "{}: {name} is {ratio:.2}:1", theme.name);
            }
            assert!(p.focus_ring.contrast_ratio(p.bg_000) >= 3.0, "{}: focus ring", theme.name);
        }
    }

    /// The switcher tells them apart by name.
    #[test]
    fn each_theme_has_its_own_name() {
        assert_ne!(light().name, dark().name);
        assert_ne!(dark().name, glass().name);
    }
}
