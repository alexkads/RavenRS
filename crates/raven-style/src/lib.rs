//! Design tokens and themes.
//!
//! Every value here mirrors `docs/design-system/tokens.json`, the source of truth
//! that is also published as the RavenRS design system. Components never hard-code
//! colors or sizes: they ask the [`Theme`] for a semantic token.

use raven_core::{Color, Insets, Point, Shadow};

/// Semantic color roles. Widgets refer to roles, never to raw colors, so every
/// component follows the active theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Semantic {
    /// `bg-000`: the canvas behind everything.
    Canvas,
    /// `bg-100`: panels and sidebars.
    Surface,
    /// `bg-200`: cards.
    SurfaceElevated,
    /// `bg-300`: control fills.
    Control,
    Border,
    Ink,
    InkMuted,
    Accent,
    AccentHover,
    AccentPressed,
    OnAccent,
    FocusRing,
    Success,
    Warning,
    Danger,
}

/// The color tokens of one theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub bg_000: Color,
    pub bg_100: Color,
    pub bg_200: Color,
    pub bg_300: Color,
    pub border: Color,
    pub ink: Color,
    pub ink_muted: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub on_accent: Color,
    pub focus_ring: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
}

impl Palette {
    pub fn get(&self, role: Semantic) -> Color {
        match role {
            Semantic::Canvas => self.bg_000,
            Semantic::Surface => self.bg_100,
            Semantic::SurfaceElevated => self.bg_200,
            Semantic::Control => self.bg_300,
            Semantic::Border => self.border,
            Semantic::Ink => self.ink,
            Semantic::InkMuted => self.ink_muted,
            Semantic::Accent => self.accent,
            Semantic::AccentHover => self.accent_hover,
            Semantic::AccentPressed => self.accent_pressed,
            Semantic::OnAccent => self.on_accent,
            Semantic::FocusRing => self.focus_ring,
            Semantic::Success => self.success,
            Semantic::Warning => self.warning,
            Semantic::Danger => self.danger,
        }
    }
}

/// Font weight on the CSS 1–1000 scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weight(pub f32);

impl Weight {
    pub const REGULAR: Self = Self(400.0);
    pub const MEDIUM: Self = Self(500.0);
    pub const SEMIBOLD: Self = Self(600.0);
    pub const BOLD: Self = Self(700.0);
}

/// One step of the type scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    pub size: f32,
    pub line_height: f32,
    pub weight: Weight,
    pub letter_spacing: f32,
}

/// Named steps of the type scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextStyle {
    Metric,
    Display,
    Title,
    Headline,
    #[default]
    Body,
    Callout,
    Caption,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    pub metric: TypeStyle,
    pub display: TypeStyle,
    pub title: TypeStyle,
    pub headline: TypeStyle,
    pub body: TypeStyle,
    pub callout: TypeStyle,
    pub caption: TypeStyle,
}

impl Typography {
    pub fn get(&self, style: TextStyle) -> TypeStyle {
        match style {
            TextStyle::Metric => self.metric,
            TextStyle::Display => self.display,
            TextStyle::Title => self.title,
            TextStyle::Headline => self.headline,
            TextStyle::Body => self.body,
            TextStyle::Callout => self.callout,
            TextStyle::Caption => self.caption,
        }
    }
}

const fn ts(size: f32, line_height: f32, weight: Weight, letter_spacing: f32) -> TypeStyle {
    TypeStyle {
        size,
        line_height,
        weight,
        letter_spacing,
    }
}

/// The type scale shared by all themes (Inter, 400 and 600).
pub const TYPOGRAPHY: Typography = Typography {
    metric: ts(42.0, 48.0, Weight::SEMIBOLD, -0.5),
    display: ts(32.0, 38.0, Weight::SEMIBOLD, -0.3),
    title: ts(22.0, 28.0, Weight::SEMIBOLD, 0.0),
    headline: ts(17.0, 24.0, Weight::SEMIBOLD, 0.0),
    body: ts(15.0, 22.0, Weight::REGULAR, 0.0),
    callout: ts(13.0, 18.0, Weight::REGULAR, 0.0),
    caption: ts(12.0, 16.0, Weight::SEMIBOLD, 0.4),
};

/// The 4px spacing grid (`space-*` tokens).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub s1: f32,
    pub s2: f32,
    pub s3: f32,
    pub s4: f32,
    pub s5: f32,
    pub s6: f32,
    pub s8: f32,
    pub s12: f32,
}

pub const SPACING: Spacing = Spacing {
    s1: 4.0,
    s2: 8.0,
    s3: 12.0,
    s4: 16.0,
    s5: 20.0,
    s6: 24.0,
    s8: 32.0,
    s12: 48.0,
};

/// Corner radii (`radius-*` tokens).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub full: f32,
}

pub const RADII: Radii = Radii {
    sm: 6.0,
    md: 10.0,
    lg: 16.0,
    xl: 24.0,
    full: 9999.0,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elevation {
    pub soft: Shadow,
    pub raised: Shadow,
}

/// What surfaces are made of. Glass surfaces are translucent and ask the
/// renderer for a backdrop blur where available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    Solid,
    Glass,
}

/// Interaction sizing and motion preferences that components must respect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interaction {
    /// Minimum control height for pointer input.
    pub min_target: f32,
    /// Width of the keyboard focus ring.
    pub focus_ring_width: f32,
    /// Gap between a control and its focus ring.
    pub focus_ring_offset: f32,
    /// When true, components skip non-essential motion.
    pub reduced_motion: bool,
}

/// A complete theme: palette plus the shared scales.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub palette: Palette,
    pub typography: Typography,
    pub spacing: Spacing,
    pub radii: Radii,
    pub elevation: Elevation,
    pub material: Material,
    pub interaction: Interaction,
}

const INTERACTION: Interaction = Interaction {
    min_target: 40.0,
    focus_ring_width: 2.0,
    focus_ring_offset: 3.0,
    reduced_motion: false,
};

const fn shadow(y: f32, blur: f32, color: Color) -> Shadow {
    Shadow {
        offset: Point::new(0.0, y),
        blur,
        color,
    }
}

impl Theme {
    /// The default theme: deep neutrals with a single blue-violet accent.
    pub const fn raven_dark() -> Self {
        Self {
            name: "Raven Dark",
            palette: Palette {
                bg_000: Color::hex(0x0b0d12),
                bg_100: Color::hex(0x12151c),
                bg_200: Color::hex(0x181c25),
                bg_300: Color::hex(0x222734),
                border: Color::hex(0x2a3040),
                ink: Color::hex(0xeef1f7),
                ink_muted: Color::hex(0x9aa3b5),
                accent: Color::hex(0x4c66ee),
                accent_hover: Color::hex(0x5f78f5),
                accent_pressed: Color::hex(0x3e55d6),
                on_accent: Color::hex(0xffffff),
                focus_ring: Color::hex(0x9db0ff),
                success: Color::hex(0x3fb984),
                warning: Color::hex(0xe8a23a),
                danger: Color::hex(0xef5b5b),
            },
            typography: TYPOGRAPHY,
            spacing: SPACING,
            radii: RADII,
            elevation: Elevation {
                soft: shadow(8.0, 24.0, Color::rgba(0.0, 0.0, 0.0, 0.35)),
                raised: shadow(16.0, 40.0, Color::rgba(0.0, 0.0, 0.0, 0.45)),
            },
            material: Material::Solid,
            interaction: INTERACTION,
        }
    }

    pub const fn raven_light() -> Self {
        Self {
            name: "Raven Light",
            palette: Palette {
                bg_000: Color::hex(0xf4f5f8),
                bg_100: Color::hex(0xffffff),
                bg_200: Color::hex(0xffffff),
                bg_300: Color::hex(0xebedf2),
                border: Color::hex(0xdadde5),
                ink: Color::hex(0x12151c),
                ink_muted: Color::hex(0x5a6375),
                accent: Color::hex(0x3f57db),
                accent_hover: Color::hex(0x4a62e3),
                accent_pressed: Color::hex(0x3348bf),
                on_accent: Color::hex(0xffffff),
                focus_ring: Color::hex(0x3f57db),
                success: Color::hex(0x1f8a5b),
                warning: Color::hex(0xb26a00),
                danger: Color::hex(0xc93434),
            },
            typography: TYPOGRAPHY,
            spacing: SPACING,
            radii: RADII,
            elevation: Elevation {
                soft: shadow(
                    6.0,
                    20.0,
                    Color::rgba(17.0 / 255.0, 24.0 / 255.0, 39.0 / 255.0, 0.08),
                ),
                raised: shadow(
                    12.0,
                    32.0,
                    Color::rgba(17.0 / 255.0, 24.0 / 255.0, 39.0 / 255.0, 0.14),
                ),
            },
            material: Material::Solid,
            interaction: INTERACTION,
        }
    }

    /// Translucent surfaces over a deep canvas. Until the compositor's backdrop
    /// blur lands, glass surfaces render as their translucent fill.
    pub const fn raven_glass() -> Self {
        Self {
            name: "Raven Glass",
            palette: Palette {
                bg_000: Color::hex(0x0a0e1a),
                bg_100: Color::hex_alpha(0xffffff0f),
                bg_200: Color::hex_alpha(0xffffff1a),
                bg_300: Color::hex_alpha(0xffffff24),
                border: Color::hex_alpha(0xffffff2e),
                ink: Color::hex(0xf5f7ff),
                ink_muted: Color::hex(0xb4bdd3),
                accent: Color::hex(0x4c66ee),
                accent_hover: Color::hex(0x5f78f5),
                accent_pressed: Color::hex(0x3e55d6),
                on_accent: Color::hex(0xffffff),
                focus_ring: Color::hex(0x9db0ff),
                success: Color::hex(0x3fb984),
                warning: Color::hex(0xe8a23a),
                danger: Color::hex(0xef5b5b),
            },
            typography: TYPOGRAPHY,
            spacing: SPACING,
            radii: RADII,
            elevation: Elevation {
                soft: shadow(8.0, 32.0, Color::rgba(0.0, 0.0, 0.0, 0.30)),
                raised: shadow(16.0, 48.0, Color::rgba(0.0, 0.0, 0.0, 0.40)),
            },
            material: Material::Glass,
            interaction: INTERACTION,
        }
    }

    pub fn color(&self, role: Semantic) -> Color {
        self.palette.get(role)
    }

    pub fn text(&self, style: TextStyle) -> TypeStyle {
        self.typography.get(style)
    }

    /// Padding of a card (`space-6`).
    pub fn card_padding(&self) -> Insets {
        Insets::uniform(self.spacing.s6)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::raven_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_over(fg: Color, bg: Color) -> Color {
        bg.lerp(fg.with_alpha(1.0), fg.a)
    }

    /// The design system promises 4.5:1 for text on its grounds in every theme.
    #[test]
    fn text_contrast_meets_wcag_aa() {
        for theme in [
            Theme::raven_dark(),
            Theme::raven_light(),
            Theme::raven_glass(),
        ] {
            let p = theme.palette;
            let card = opaque_over(p.bg_200, p.bg_000);
            for (name, fg, bg) in [
                ("ink/canvas", p.ink, p.bg_000),
                ("ink/card", p.ink, card),
                ("ink-muted/canvas", p.ink_muted, p.bg_000),
                ("ink-muted/card", p.ink_muted, card),
                ("on-accent/accent", p.on_accent, p.accent),
            ] {
                let ratio = fg.contrast_ratio(bg);
                assert!(ratio >= 4.5, "{}: {name} is {ratio:.2}:1", theme.name);
            }
            assert!(
                p.focus_ring.contrast_ratio(p.bg_000) >= 3.0,
                "{}: focus ring",
                theme.name
            );
        }
    }

    #[test]
    fn semantic_lookup_matches_palette() {
        let t = Theme::raven_dark();
        assert_eq!(t.color(Semantic::Accent), Color::hex(0x4c66ee));
        assert_eq!(t.text(TextStyle::Metric).size, 42.0);
    }
}
