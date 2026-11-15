//! How one node or one line looks apart from the theme: colors written as `#rrggbb`, so they
//! go to a file and come back as they are.

use ravenrs_core::Color;
use serde::{Deserialize, Serialize};

/// What a node shows apart from the theme. Every part left out is the theme's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FlowStyle {
    /// Inside the outline, as `#rrggbb` or `#rrggbbaa`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// The outline's color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// The outline's width, in units of the diagram.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f32>,
    /// The words' color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The title under the outline instead of inside it, as small round or diamond shapes
    /// carry it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub title_below: bool,
    /// A sign drawn large in the middle of the outline, such as `✕` or `+`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FlowStyle {
    /// The theme's look.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filled with `color` (`#rrggbb`).
    pub fn fill(mut self, color: impl Into<String>) -> Self {
        self.fill = Some(color.into());
        self
    }

    /// Outlined in `color`, `width` units wide.
    pub fn stroke(mut self, color: impl Into<String>, width: f32) -> Self {
        self.stroke = Some(color.into());
        self.stroke_width = Some(width);
        self
    }

    /// Only the outline's width, in the theme's color.
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = Some(width);
        self
    }

    /// Words in `color`.
    pub fn text(mut self, color: impl Into<String>) -> Self {
        self.text = Some(color.into());
        self
    }

    /// The title under the outline.
    pub fn title_below(mut self) -> Self {
        self.title_below = true;
        self
    }

    /// A sign drawn large in the middle.
    pub fn marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = Some(marker.into());
        self
    }

    /// Whether it is all the theme's.
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }

    /// The fill as a color, when it is one.
    pub fn fill_color(&self) -> Option<Color> {
        self.fill.as_deref().and_then(parse_color)
    }

    /// The outline's color, when it is one.
    pub fn stroke_color(&self) -> Option<Color> {
        self.stroke.as_deref().and_then(parse_color)
    }

    /// The words' color, when it is one.
    pub fn text_color(&self) -> Option<Color> {
        self.text.as_deref().and_then(parse_color)
    }
}

/// A color written `#rgb`, `#rrggbb` or `#rrggbbaa`; anything else is none.
pub fn parse_color(text: &str) -> Option<Color> {
    let hex = text.trim().strip_prefix('#')?;
    let digit = |i: usize, n: usize| u8::from_str_radix(hex.get(i..i + n)?, 16).ok();
    let (r, g, b, a) = match hex.len() {
        3 => {
            let one = |i| digit(i, 1).map(|v| v * 17);
            (one(0)?, one(1)?, one(2)?, 255)
        }
        6 => (digit(0, 2)?, digit(2, 2)?, digit(4, 2)?, 255),
        8 => (digit(0, 2)?, digit(2, 2)?, digit(4, 2)?, digit(6, 2)?),
        _ => return None,
    };
    let f = |v: u8| f32::from(v) / 255.0;
    Some(Color::rgba(f(r), f(g), f(b), f(a)))
}

/// A color written `#rrggbb` (with `aa` when it is not opaque).
pub fn color_hex(color: Color) -> String {
    let [r, g, b, a] = color.to_array();
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if a >= 0.999 {
        format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b), byte(a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_read_and_written_as_hex() {
        assert_eq!(parse_color("#fff"), Some(Color::rgba(1.0, 1.0, 1.0, 1.0)));
        assert_eq!(parse_color("#ff000080").map(|c| c.to_array()[3] > 0.49), Some(true));
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#12345"), None);
        let c = parse_color("#3a7bd5").expect("a color");
        assert_eq!(color_hex(c), "#3a7bd5");
        let style = FlowStyle::new().fill("#3a7bd5").stroke("#000", 3.0).title_below().marker("✕");
        assert!(!style.is_plain() && FlowStyle::new().is_plain());
        assert_eq!(style.fill_color(), Some(c));
    }
}
