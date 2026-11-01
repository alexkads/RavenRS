//! Design tokens and themes.
//!
//! Every value here mirrors `docs/design-system/tokens.json`, the source of truth
//! that is also published as the RavenRS design system. Components never hard-code
//! colors or sizes: they ask the [`Theme`] for a semantic token.

use ravenrs_core::{Color, Insets, Point, Shadow};

/// Semantic color roles. Widgets refer to roles, never to raw colors, so every
/// component follows the active theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Semantic {
    /// `bg-000`: the canvas behind everything.
    Canvas,
    /// `bg-100`: panels and sidebars.
    Surface,
    /// `bg-200`: cards.
    SurfaceElevated,
    /// `bg-300`: control fills.
    Control,
    /// `border`: hairlines and control outlines.
    Border,
    /// `ink`: primary text and icons.
    Ink,
    /// `ink-muted`: secondary text, captions and placeholders.
    InkMuted,
    /// `accent`: primary actions and selection.
    Accent,
    /// `accent-hover`: accent fill while hovered.
    AccentHover,
    /// `accent-pressed`: accent fill while pressed.
    AccentPressed,
    /// `on-accent`: text and icons drawn on an accent fill.
    OnAccent,
    /// `focus-ring`: keyboard focus indicator.
    FocusRing,
    /// `success`: positive status.
    Success,
    /// `warning`: cautionary status.
    Warning,
    /// `danger`: errors and destructive actions.
    Danger,
    /// `scrim`: the dimmed backdrop behind dialogs and sheets, the same in every theme.
    Scrim,
}

/// The color tokens of one theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// `bg-000`: the canvas behind everything.
    pub bg_000: Color,
    /// `bg-100`: panels and sidebars.
    pub bg_100: Color,
    /// `bg-200`: cards.
    pub bg_200: Color,
    /// `bg-300`: control fills.
    pub bg_300: Color,
    /// `border`: hairlines and control outlines.
    pub border: Color,
    /// `ink`: primary text and icons.
    pub ink: Color,
    /// `ink-muted`: secondary text, captions and placeholders.
    pub ink_muted: Color,
    /// `accent`: primary actions and selection.
    pub accent: Color,
    /// `accent-hover`: accent fill while hovered.
    pub accent_hover: Color,
    /// `accent-pressed`: accent fill while pressed.
    pub accent_pressed: Color,
    /// `on-accent`: text and icons drawn on an accent fill.
    pub on_accent: Color,
    /// `focus-ring`: keyboard focus indicator.
    pub focus_ring: Color,
    /// `success`: positive status.
    pub success: Color,
    /// `warning`: cautionary status.
    pub warning: Color,
    /// `danger`: errors and destructive actions.
    pub danger: Color,
}

impl Palette {
    /// Resolves a semantic role to this palette's color.
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
            Semantic::Scrim => Color::rgba(0.0, 0.0, 0.0, 0.5),
        }
    }
}

/// Font weight on the CSS 1–1000 scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weight(pub f32);

impl Weight {
    /// Regular weight (400).
    pub const REGULAR: Self = Self(400.0);
    /// Medium weight (500).
    pub const MEDIUM: Self = Self(500.0);
    /// Semibold weight (600).
    pub const SEMIBOLD: Self = Self(600.0);
    /// Bold weight (700).
    pub const BOLD: Self = Self(700.0);
}

/// One step of the type scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    /// Font size in logical pixels.
    pub size: f32,
    /// Line height in logical pixels.
    pub line_height: f32,
    /// Font weight.
    pub weight: Weight,
    /// Extra space between characters in logical pixels; may be negative.
    pub letter_spacing: f32,
}

/// Named steps of the type scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextStyle {
    /// Large dashboard numbers (42px).
    Metric,
    /// Page and hero headings (32px).
    Display,
    /// Section titles (22px).
    Title,
    /// Emphasized headings inside content (17px).
    Headline,
    /// Default running text (15px).
    #[default]
    Body,
    /// Secondary, denser text (13px).
    Callout,
    /// Small labels and metadata (12px).
    Caption,
}

/// The type scale: one [`TypeStyle`] per [`TextStyle`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    /// Style for [`TextStyle::Metric`].
    pub metric: TypeStyle,
    /// Style for [`TextStyle::Display`].
    pub display: TypeStyle,
    /// Style for [`TextStyle::Title`].
    pub title: TypeStyle,
    /// Style for [`TextStyle::Headline`].
    pub headline: TypeStyle,
    /// Style for [`TextStyle::Body`].
    pub body: TypeStyle,
    /// Style for [`TextStyle::Callout`].
    pub callout: TypeStyle,
    /// Style for [`TextStyle::Caption`].
    pub caption: TypeStyle,
}

impl Typography {
    /// Returns the type style for a named step.
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
    TypeStyle { size, line_height, weight, letter_spacing }
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
    /// `space-1`: 4 logical pixels.
    pub s1: f32,
    /// `space-2`: 8 logical pixels.
    pub s2: f32,
    /// `space-3`: 12 logical pixels.
    pub s3: f32,
    /// `space-4`: 16 logical pixels.
    pub s4: f32,
    /// `space-5`: 20 logical pixels.
    pub s5: f32,
    /// `space-6`: 24 logical pixels.
    pub s6: f32,
    /// `space-8`: 32 logical pixels.
    pub s8: f32,
    /// `space-12`: 48 logical pixels.
    pub s12: f32,
}

/// The spacing scale shared by all themes.
pub const SPACING: Spacing =
    Spacing { s1: 4.0, s2: 8.0, s3: 12.0, s4: 16.0, s5: 20.0, s6: 24.0, s8: 32.0, s12: 48.0 };

/// Corner radii (`radius-*` tokens).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    /// `radius-sm`: small controls, in logical pixels.
    pub sm: f32,
    /// `radius-md`: buttons and inputs, in logical pixels.
    pub md: f32,
    /// `radius-lg`: cards, in logical pixels.
    pub lg: f32,
    /// `radius-xl`: sheets and large panels, in logical pixels.
    pub xl: f32,
    /// `radius-full`: pill and circle shapes.
    pub full: f32,
}

/// The corner radii shared by all themes.
pub const RADII: Radii = Radii { sm: 6.0, md: 10.0, lg: 16.0, xl: 24.0, full: 9999.0 };

/// Shadow tokens (`shadow-*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elevation {
    /// `shadow-soft`: cards and resting surfaces.
    pub soft: Shadow,
    /// `shadow-raised`: popovers, menus and dialogs.
    pub raised: Shadow,
}

/// What surfaces are made of. Glass surfaces are translucent and ask the
/// renderer for a backdrop blur where available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Material {
    /// Opaque fills.
    Solid,
    /// Translucent fills with backdrop blur where supported.
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
    /// Display name, e.g. `"Raven Dark"`.
    pub name: &'static str,
    /// Color tokens.
    pub palette: Palette,
    /// Type scale.
    pub typography: Typography,
    /// Spacing scale.
    pub spacing: Spacing,
    /// Corner radii.
    pub radii: Radii,
    /// Shadow tokens.
    pub elevation: Elevation,
    /// Surface material.
    pub material: Material,
    /// Interaction sizing and motion preferences.
    pub interaction: Interaction,
}

const INTERACTION: Interaction =
    Interaction { min_target: 40.0, focus_ring_width: 2.0, focus_ring_offset: 3.0, reduced_motion: false };

const fn shadow(y: f32, blur: f32, color: Color) -> Shadow {
    Shadow { offset: Point::new(0.0, y), blur, color }
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

    /// Light theme: near-white surfaces with the same blue-violet accent.
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
                soft: shadow(6.0, 20.0, Color::rgba(17.0 / 255.0, 24.0 / 255.0, 39.0 / 255.0, 0.08)),
                raised: shadow(12.0, 32.0, Color::rgba(17.0 / 255.0, 24.0 / 255.0, 39.0 / 255.0, 0.14)),
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

    /// Resolves a semantic color role in this theme.
    ///
    /// ```
    /// use ravenrs_style::{Semantic, Theme};
    ///
    /// let theme = Theme::raven_dark();
    /// assert_eq!(theme.color(Semantic::Accent), theme.palette.accent);
    /// ```
    pub fn color(&self, role: Semantic) -> Color {
        self.palette.get(role)
    }

    /// Returns the type style for a named step of the scale.
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
        for theme in [Theme::raven_dark(), Theme::raven_light(), Theme::raven_glass()] {
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
            assert!(p.focus_ring.contrast_ratio(p.bg_000) >= 3.0, "{}: focus ring", theme.name);
        }
    }

    #[test]
    fn semantic_lookup_matches_palette() {
        let t = Theme::raven_dark();
        assert_eq!(t.color(Semantic::Accent), Color::hex(0x4c66ee));
        assert_eq!(t.text(TextStyle::Metric).size, 42.0);
    }
}
