//! A theme's tokens by name, to change one at a time and to keep what changed as text.

use ravenrs_core::Color;
use serde_json::{Map, Value, json};

use crate::{Palette, Radii, Semantic, TextStyle, Theme, Typography};

/// The least contrast for a window to count as readable, well under what the design system asks for.
const LEAST_CONTRAST: f32 = 3.0;
/// The sizes of text that still fit a window.
const TEXT_SIZES: std::ops::RangeInclusive<f32> = 9.0..=64.0;
/// The sizes a radius can take when it comes from text.
const RADIUS_RANGE: std::ops::RangeInclusive<f64> = 0.0..=64.0;
/// The sizes text can take when it comes from text.
const SIZE_RANGE: std::ops::RangeInclusive<f64> = 8.0..=96.0;

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Semantic {
    /// The colors a theme chooses. The others follow from these.
    pub const TOKENS: [Self; 15] = [
        Self::Canvas,
        Self::Surface,
        Self::SurfaceElevated,
        Self::Control,
        Self::Border,
        Self::Ink,
        Self::InkMuted,
        Self::Accent,
        Self::AccentHover,
        Self::AccentPressed,
        Self::OnAccent,
        Self::FocusRing,
        Self::Success,
        Self::Warning,
        Self::Danger,
    ];

    /// Its name in `docs/design-system/tokens.json`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Canvas => "bg-000",
            Self::Surface => "bg-100",
            Self::SurfaceElevated => "bg-200",
            Self::Control => "bg-300",
            Self::Border => "border",
            Self::Ink => "ink",
            Self::InkMuted => "ink-muted",
            Self::Accent => "accent",
            Self::AccentHover => "accent-hover",
            Self::AccentPressed => "accent-pressed",
            Self::OnAccent => "on-accent",
            Self::FocusRing => "focus-ring",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
            Self::Scrim => "scrim",
            Self::Chart1 => "chart-1",
            Self::Chart2 => "chart-2",
            Self::Chart3 => "chart-3",
            Self::Chart4 => "chart-4",
            Self::Chart5 => "chart-5",
            Self::ChartGrid => "chart-grid",
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Palette {
    /// Changes the color of a role. Roles that follow from others, as in charts, stay as they are.
    pub fn set(&mut self, role: Semantic, color: Color) {
        match role {
            Semantic::Canvas => self.bg_000 = color,
            Semantic::Surface => self.bg_100 = color,
            Semantic::SurfaceElevated => self.bg_200 = color,
            Semantic::Control => self.bg_300 = color,
            Semantic::Border => self.border = color,
            Semantic::Ink => self.ink = color,
            Semantic::InkMuted => self.ink_muted = color,
            Semantic::Accent => self.accent = color,
            Semantic::AccentHover => self.accent_hover = color,
            Semantic::AccentPressed => self.accent_pressed = color,
            Semantic::OnAccent => self.on_accent = color,
            Semantic::FocusRing => self.focus_ring = color,
            Semantic::Success => self.success = color,
            Semantic::Warning => self.warning = color,
            Semantic::Danger => self.danger = color,
            _ => {}
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Radii {
    /// The radii a theme chooses, by name. `full` is not one: it only means a pill.
    pub const TOKENS: [&'static str; 4] = ["sm", "md", "lg", "xl"];

    /// A radius by name.
    pub fn get(&self, token: &str) -> Option<f32> {
        match token {
            "sm" => Some(self.sm),
            "md" => Some(self.md),
            "lg" => Some(self.lg),
            "xl" => Some(self.xl),
            _ => None,
        }
    }

    /// Changes a radius by name, if there is one.
    pub fn set(&mut self, token: &str, radius: f32) {
        match token {
            "sm" => self.sm = radius,
            "md" => self.md = radius,
            "lg" => self.lg = radius,
            "xl" => self.xl = radius,
            _ => {}
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TextStyle {
    /// Every step of the scale, largest first.
    pub const TOKENS: [Self; 7] =
        [Self::Metric, Self::Display, Self::Title, Self::Headline, Self::Body, Self::Callout, Self::Caption];

    /// Its name in `docs/design-system/tokens.json`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Display => "display",
            Self::Title => "title",
            Self::Headline => "headline",
            Self::Body => "body",
            Self::Callout => "callout",
            Self::Caption => "caption",
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Typography {
    /// Changes the size of a step, with its line height in the same proportion.
    pub fn set_size(&mut self, style: TextStyle, size: f32) {
        let step = match style {
            TextStyle::Metric => &mut self.metric,
            TextStyle::Display => &mut self.display,
            TextStyle::Title => &mut self.title,
            TextStyle::Headline => &mut self.headline,
            TextStyle::Body => &mut self.body,
            TextStyle::Callout => &mut self.callout,
            TextStyle::Caption => &mut self.caption,
        };
        let proportion = step.line_height / step.size.max(1.0);
        step.size = size;
        step.line_height = (size * proportion).round();
    }
}

/// A color as `#rrggbb`, or `#rrggbbaa` when it lets something through.
fn to_hex(color: Color) -> String {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (byte(color.r), byte(color.g), byte(color.b), byte(color.a));
    if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
}

fn from_hex(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(Color::hex(value)),
        8 => Some(Color::hex_alpha(value)),
        _ => None,
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Theme {
    /// The themes that come with RavenRS.
    pub const fn builtin() -> [Self; 3] {
        [Self::raven_dark(), Self::raven_light(), Self::raven_glass()]
    }

    /// The built-in theme this one started from, by its name; itself when it did not.
    pub fn base(&self) -> Self {
        Self::builtin().into_iter().find(|theme| theme.name == self.name).unwrap_or(*self)
    }

    /// What differs from [`base`](Self::base), in the shape of [`to_tokens_json`](Self::to_tokens_json).
    fn changes(&self) -> (Map<String, Value>, Map<String, Value>, Map<String, Value>) {
        let base = self.base();
        let colors = Semantic::TOKENS
            .into_iter()
            .filter(|role| to_hex(self.palette.get(*role)) != to_hex(base.palette.get(*role)))
            .map(|role| (role.token().to_owned(), json!(to_hex(self.palette.get(role)))))
            .collect();
        let radii = Radii::TOKENS
            .into_iter()
            .filter(|token| self.radii.get(token) != base.radii.get(token))
            .map(|token| (token.to_owned(), json!(self.radii.get(token))))
            .collect();
        let sizes = TextStyle::TOKENS
            .into_iter()
            .filter(|style| self.typography.get(*style).size != base.typography.get(*style).size)
            .map(|style| (style.token().to_owned(), json!(self.typography.get(style).size)))
            .collect();
        (colors, radii, sizes)
    }

    /// Whether someone can read a window in this theme, and so find their way out of it. A theme
    /// kept from another time that is not goes unused: a person can make anything while editing,
    /// and should not open to a blank window because of it.
    pub fn is_readable(&self) -> bool {
        let palette = &self.palette;
        let flat = |color: Color| palette.bg_000.lerp(color.with_alpha(1.0), color.a);
        let grounds = [palette.bg_000, palette.bg_100, palette.bg_200];
        let legible = |ink: Color, ground: Color| flat(ink).contrast_ratio(flat(ground)) >= LEAST_CONTRAST;
        grounds.iter().all(|ground| legible(palette.ink, *ground) && legible(palette.ink_muted, *ground))
            && legible(palette.on_accent, palette.accent)
            && TextStyle::TOKENS.iter().all(|style| TEXT_SIZES.contains(&self.typography.get(*style).size))
    }

    /// How many tokens differ from [`base`](Self::base).
    pub fn changed_tokens(&self) -> usize {
        let (colors, radii, sizes) = self.changes();
        colors.len() + radii.len() + sizes.len()
    }

    /// What differs from [`base`](Self::base) as JSON, with the same names as
    /// `docs/design-system/tokens.json`, for keeping or handing over.
    pub fn to_tokens_json(&self) -> String {
        let (colors, radii, sizes) = self.changes();
        let tokens = json!({ "base": self.name, "color": colors, "radius": radii, "type": sizes });
        serde_json::to_string_pretty(&tokens).unwrap_or_default()
    }

    /// A theme from what [`to_tokens_json`](Self::to_tokens_json) wrote. Anything it does not
    /// recognize, or a size too far out to read, refuses the whole text, which may come from anywhere.
    pub fn from_tokens_json(text: &str) -> Result<Self, String> {
        let tokens: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let name = tokens.get("base").and_then(Value::as_str).ok_or("no base theme")?;
        let mut theme = Self::builtin()
            .into_iter()
            .find(|theme| theme.name == name)
            .ok_or_else(|| format!("unknown base theme: {name}"))?;
        let part = |key: &str| tokens.get(key).and_then(Value::as_object).cloned().unwrap_or_default();
        for (token, value) in part("color") {
            let role = Semantic::TOKENS
                .into_iter()
                .find(|role| role.token() == token)
                .ok_or_else(|| format!("unknown color: {token}"))?;
            let color = value.as_str().and_then(from_hex).ok_or_else(|| format!("bad color: {token}"))?;
            theme.palette.set(role, color);
        }
        for (token, value) in part("radius") {
            if !Radii::TOKENS.contains(&token.as_str()) {
                return Err(format!("unknown radius: {token}"));
            }
            let radius = value
                .as_f64()
                .filter(|r| RADIUS_RANGE.contains(r))
                .ok_or_else(|| format!("bad radius: {token}"))?;
            theme.radii.set(&token, radius as f32);
        }
        for (token, value) in part("type") {
            let style = TextStyle::TOKENS
                .into_iter()
                .find(|style| style.token() == token)
                .ok_or_else(|| format!("unknown text size: {token}"))?;
            let size = value
                .as_f64()
                .filter(|s| SIZE_RANGE.contains(s))
                .ok_or_else(|| format!("bad text size: {token}"))?;
            theme.typography.set_size(style, size as f32);
        }
        Ok(theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_token_is_read_and_changed_by_its_name() {
        for role in Semantic::TOKENS {
            let mut palette = Theme::raven_dark().palette;
            let color = Color::hex(0x123456);
            palette.set(role, color);
            assert_eq!(palette.get(role), color, "{}", role.token());
        }
        let mut radii = Theme::raven_dark().radii;
        for token in Radii::TOKENS {
            radii.set(token, 3.0);
            assert_eq!(radii.get(token), Some(3.0));
        }
    }

    #[test]
    fn what_changed_goes_to_text_and_comes_back() {
        let mut theme = Theme::raven_light();
        assert_eq!(theme.changed_tokens(), 0);
        theme.palette.set(Semantic::Accent, Color::hex(0xff0066));
        theme.palette.set(Semantic::Surface, Color::hex_alpha(0x11223344));
        theme.radii.set("md", 4.0);
        theme.typography.set_size(TextStyle::Body, 16.0);
        assert_eq!(theme.changed_tokens(), 4);
        let again = Theme::from_tokens_json(&theme.to_tokens_json()).expect("reads");
        assert_eq!(again.name, "Raven Light");
        assert_eq!(to_hex(again.palette.accent), "#ff0066");
        assert_eq!(to_hex(again.palette.bg_100), "#11223344");
        assert_eq!(again.radii.md, 4.0);
        assert_eq!(again.typography.body.size, 16.0);
        assert_eq!(again.changed_tokens(), 4);
    }

    #[test]
    fn the_themes_that_come_with_it_are_readable_and_a_blank_one_is_not() {
        for theme in Theme::builtin() {
            assert!(theme.is_readable(), "{}", theme.name);
        }
        let mut blank = Theme::raven_dark();
        blank.palette.ink = Color::hex_alpha(0x00000000);
        assert!(!blank.is_readable());
        let mut same = Theme::raven_light();
        same.palette.ink = same.palette.bg_100;
        assert!(!same.is_readable());
        let mut huge = Theme::raven_dark();
        huge.typography.set_size(TextStyle::Body, 96.0);
        assert!(!huge.is_readable());
    }

    #[test]
    fn a_line_keeps_its_proportion_to_its_size() {
        let mut theme = Theme::raven_dark();
        let before = theme.typography.body;
        theme.typography.set_size(TextStyle::Body, before.size * 2.0);
        assert_eq!(theme.typography.body.line_height, (before.line_height * 2.0).round());
    }

    #[test]
    fn text_with_anything_strange_is_refused_whole() {
        for bad in [
            "not json",
            "{}",
            r##"{"base":"Other"}"##,
            r##"{"base":"Raven Dark","color":{"nothing":"#000000"}}"##,
            r##"{"base":"Raven Dark","color":{"ink":"black"}}"##,
            r##"{"base":"Raven Dark","radius":{"md":9000}}"##,
            r##"{"base":"Raven Dark","radius":{"full":1}}"##,
            r##"{"base":"Raven Dark","type":{"body":1}}"##,
        ] {
            assert!(Theme::from_tokens_json(bad).is_err(), "{bad}");
        }
    }
}
