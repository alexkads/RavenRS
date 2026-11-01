use ravenrs_core::Color;
use ravenrs_style::{Semantic, Theme};

/// A color given either as a theme role (preferred, follows theme changes) or
/// as a literal color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    Semantic(Semantic),
    Color(Color),
}

impl Fill {
    pub fn resolve(self, theme: &Theme) -> Color {
        match self {
            Self::Semantic(role) => theme.color(role),
            Self::Color(c) => c,
        }
    }
}

impl From<Semantic> for Fill {
    fn from(s: Semantic) -> Self {
        Self::Semantic(s)
    }
}

impl From<Color> for Fill {
    fn from(c: Color) -> Self {
        Self::Color(c)
    }
}
