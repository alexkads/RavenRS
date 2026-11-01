//! Text shaping and layout for RavenRS.
//!
//! [`TextSystem`] owns Parley's font database and scratch space. It ships with
//! Inter (SIL Open Font License 1.1, see `fonts/Inter-LICENSE.txt`) embedded,
//! because browsers expose no system fonts to WebAssembly; the same bytes are used
//! on the desktop so text looks identical everywhere. System fonts are not loaded.
//!
//! Layout happens in logical pixels. The renderer rasterizes glyphs at the device
//! scale factor, so text stays sharp on HiDPI screens without re-shaping.

use std::fmt;
use std::sync::Arc;

use parley::fontique::{Blob, Collection, CollectionOptions, FamilyId, GenericFamily, SourceCache};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontWeight, Layout, LayoutContext, LineHeight,
    PositionedLayoutItem, StyleProperty,
};
use ravenrs_core::{Color, FontRef, GlyphRun, Point, PositionedGlyph, Scene, Size};

static INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
static INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");
/// A subset of Lucide icons (ISC License), see `scripts/gen-icons.py`.
static RAVEN_ICONS: &[u8] = include_bytes!("../fonts/RavenIcons.ttf");

/// The family name of the embedded icon font.
pub const ICON_FAMILY: &str = "RavenIcons";

/// The family name of the embedded default font.
pub const DEFAULT_FAMILY: &str = "Inter";

/// Visual attributes of a piece of text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextAttrs {
    /// Font size in logical pixels.
    pub size: f32,
    /// Line height in logical pixels.
    pub line_height: f32,
    /// CSS weight (400 regular, 600 semibold).
    pub weight: f32,
    /// Extra space between characters in logical pixels; may be negative.
    pub letter_spacing: f32,
    /// Font family list in CSS syntax; defaults to the embedded Inter.
    pub family: Option<String>,
}

impl Default for TextAttrs {
    fn default() -> Self {
        Self { size: 15.0, line_height: 22.0, weight: 400.0, letter_spacing: 0.0, family: None }
    }
}

/// Font database plus reusable layout scratch space. Create one per app.
pub struct TextSystem {
    fonts: FontContext,
    layouts: LayoutContext<()>,
}

impl fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextSystem").finish_non_exhaustive()
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSystem {
    /// Creates a text system with Inter registered and used for every generic family.
    pub fn new() -> Self {
        let collection = Collection::new(CollectionOptions { shared: false, system_fonts: false });
        let mut system = Self {
            fonts: FontContext { collection, source_cache: SourceCache::default() },
            layouts: LayoutContext::new(),
        };
        let mut families = Vec::new();
        for bytes in [INTER_REGULAR, INTER_SEMIBOLD] {
            families.extend(system.register_static_font(bytes));
        }
        families.dedup();
        system.register_static_font(RAVEN_ICONS);
        for generic in [GenericFamily::SansSerif, GenericFamily::SystemUi, GenericFamily::UiSansSerif] {
            system.fonts.collection.set_generic_families(generic, families.iter().copied());
        }
        system
    }

    fn register_static_font(&mut self, bytes: &'static [u8]) -> Vec<FamilyId> {
        let blob = Blob::new(Arc::new(bytes));
        self.fonts.collection.register_fonts(blob, None).into_iter().map(|(id, _)| id).collect()
    }

    /// Registers a font file (TTF/OTF/TTC) and returns the family names it provides.
    pub fn register_font(&mut self, bytes: Vec<u8>) -> Vec<String> {
        let ids: Vec<FamilyId> = self
            .fonts
            .collection
            .register_fonts(Blob::from(bytes), None)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        ids.into_iter().filter_map(|id| self.fonts.collection.family_name(id).map(str::to_owned)).collect()
    }

    /// Shapes `text`. Line breaking happens later in [`TextLayout::measure`].
    pub fn layout(&mut self, text: &str, attrs: &TextAttrs) -> TextLayout {
        let mut builder = self.layouts.ranged_builder(&mut self.fonts, text, 1.0, false);
        let family = attrs.family.as_deref().unwrap_or(DEFAULT_FAMILY);
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(family.into())));
        builder.push_default(StyleProperty::FontSize(attrs.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(attrs.weight)));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(attrs.line_height)));
        if attrs.letter_spacing != 0.0 {
            builder.push_default(StyleProperty::LetterSpacing(attrs.letter_spacing));
        }
        let layout = builder.build(text);
        TextLayout { layout, broken_at: None }
    }
}

/// Shaped text, ready to be broken into lines at any width and drawn.
pub struct TextLayout {
    layout: Layout<()>,
    /// The width limit of the last line breaking, to skip redundant work.
    broken_at: Option<Option<f32>>,
}

impl fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextLayout")
            .field("width", &self.layout.width())
            .field("height", &self.layout.height())
            .finish()
    }
}

impl TextLayout {
    /// Breaks lines at `max_width` (or never, for `None`) and returns the text's size.
    pub fn measure(&mut self, max_width: Option<f32>) -> Size {
        if self.broken_at != Some(max_width) {
            self.layout.break_all_lines(max_width);
            self.layout.align(Alignment::Start, AlignmentOptions::default());
            self.broken_at = Some(max_width);
        }
        Size::new(self.layout.width(), self.layout.height())
    }

    /// Appends the glyphs, as laid out by the last [`Self::measure`], to `scene`
    /// with their top-left corner at `origin`.
    pub fn paint(&self, origin: Point, color: Color, scene: &mut Scene) {
        for line in self.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let glyphs = glyph_run
                    .positioned_glyphs()
                    .map(|g| PositionedGlyph { id: g.id, x: origin.x + g.x, y: origin.y + g.y })
                    .collect();
                scene.push_glyphs(GlyphRun {
                    font: FontRef {
                        id: font.data.id(),
                        data: Arc::new(font.data.clone()),
                        index: font.index,
                    },
                    size: run.font_size(),
                    color,
                    glyphs,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ravenrs_core::Primitive;

    use super::*;

    #[test]
    fn shapes_with_embedded_font() {
        let mut ts = TextSystem::new();
        let mut layout = ts.layout("Welcome to RavenRS", &TextAttrs::default());
        let size = layout.measure(None);
        assert!(size.width > 100.0 && size.width < 200.0, "unexpected width {}", size.width);
        assert_eq!(size.height, 22.0);
    }

    #[test]
    fn wraps_when_narrow() {
        let mut ts = TextSystem::new();
        let mut layout = ts.layout("one two three four five six", &TextAttrs::default());
        let wide = layout.measure(None);
        let narrow = layout.measure(Some(60.0));
        assert!(narrow.height > wide.height);
        assert!(narrow.width <= 60.0 + 0.5);
    }

    #[test]
    fn semibold_is_wider_than_regular() {
        let mut ts = TextSystem::new();
        let regular = ts.layout("RavenRS", &TextAttrs::default()).measure(None).width;
        let bold =
            ts.layout("RavenRS", &TextAttrs { weight: 600.0, ..TextAttrs::default() }).measure(None).width;
        assert!(bold > regular, "semibold face should be selected ({bold} vs {regular})");
    }

    #[test]
    fn icon_font_is_registered() {
        let mut ts = TextSystem::new();
        let attrs = TextAttrs {
            family: Some(ICON_FAMILY.into()),
            size: 16.0,
            line_height: 16.0,
            ..Default::default()
        };
        // U+E151 is "search" in Lucide; any private-use glyph from the subset will do.
        let mut layout = ts.layout("\u{e151}", &attrs);
        let size = layout.measure(None);
        assert!((size.width - 16.0).abs() < 1.0, "icon glyph should be one em wide, got {}", size.width);
    }

    #[test]
    fn paints_one_glyph_per_character_offset_by_origin() {
        let mut ts = TextSystem::new();
        let mut layout = ts.layout("Raven", &TextAttrs::default());
        layout.measure(None);
        let mut scene = Scene::new();
        layout.paint(Point::new(10.0, 20.0), Color::WHITE, &mut scene);
        let Some(Primitive::Glyphs(run)) = scene.primitives().first() else {
            panic!("expected a glyph run");
        };
        assert_eq!(run.glyphs.len(), 5);
        assert!(run.glyphs[0].x >= 10.0);
        assert!(run.glyphs[0].y > 20.0, "glyph y is the baseline below the origin");
        assert!(!run.font.bytes().is_empty());
    }
}
