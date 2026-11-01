/// A non-premultiplied RGBA color with components in `0.0..=1.0`, in the sRGB
/// color space (the space design tokens are written in).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    /// Red component, `0.0..=1.0`.
    pub r: f32,
    /// Green component, `0.0..=1.0`.
    pub g: f32,
    /// Blue component, `0.0..=1.0`.
    pub b: f32,
    /// Alpha (opacity), `0.0` transparent to `1.0` opaque.
    pub a: f32,
}

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
    /// Opaque black.
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// Opaque white.
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);

    /// Builds a color from sRGB components and alpha, each in `0.0..=1.0`.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Builds an opaque color from sRGB components in `0.0..=1.0`.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }

    /// Builds an opaque color from `0xRRGGBB`.
    pub const fn hex(rgb: u32) -> Self {
        Self::rgb(
            ((rgb >> 16) & 0xff) as f32 / 255.0,
            ((rgb >> 8) & 0xff) as f32 / 255.0,
            (rgb & 0xff) as f32 / 255.0,
        )
    }

    /// Builds a color from `0xRRGGBBAA`.
    pub const fn hex_alpha(rgba: u32) -> Self {
        Self::rgba(
            ((rgba >> 24) & 0xff) as f32 / 255.0,
            ((rgba >> 16) & 0xff) as f32 / 255.0,
            ((rgba >> 8) & 0xff) as f32 / 255.0,
            (rgba & 0xff) as f32 / 255.0,
        )
    }

    /// Returns the same color with alpha replaced by `a` (`0.0..=1.0`).
    pub const fn with_alpha(self, a: f32) -> Self {
        Self::rgba(self.r, self.g, self.b, a)
    }

    /// Component-wise interpolation in sRGB space, `t` clamped to `0..=1`.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::rgba(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }

    /// Converts to linear-light RGB (alpha unchanged), as needed by `*Srgb` render targets.
    pub fn to_linear(self) -> [f32; 4] {
        fn channel(c: f32) -> f32 {
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }
        [channel(self.r), channel(self.g), channel(self.b), self.a]
    }

    /// Returns the sRGB components as `[r, g, b, a]`, without linearization.
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let [r, g, b, _] = self.to_linear();
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// WCAG contrast ratio between two opaque colors (1.0 to 21.0).
    pub fn contrast_ratio(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(Color::hex(0xff0000), Color::rgb(1.0, 0.0, 0.0));
        let c = Color::hex_alpha(0xffffff80);
        assert!((c.a - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn contrast_black_white_is_21() {
        assert!((Color::BLACK.contrast_ratio(Color::WHITE) - 21.0).abs() < 0.01);
    }

    #[test]
    fn lerp_clamps() {
        assert_eq!(Color::BLACK.lerp(Color::WHITE, 2.0), Color::WHITE);
    }
}
