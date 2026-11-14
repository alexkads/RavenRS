//! Text shaping and layout for RavenRS.
//!
//! [`TextSystem`] owns Parley's font database and scratch space. It ships with
//! Inter (SIL Open Font License 1.1, see `fonts/Inter-LICENSE.txt`) embedded,
//! because browsers expose no system fonts to WebAssembly; the same bytes are used
//! on the desktop so text looks identical everywhere. System fonts are not loaded.
//!
//! Layout happens in logical pixels. The renderer rasterizes glyphs at the device
//! scale factor, so text stays sharp on HiDPI screens without re-shaping.

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

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
/// Fira Mono (SIL Open Font License), for code.
#[cfg(feature = "mono")]
static FIRA_MONO: &[u8] = include_bytes!("../fonts/FiraMono-Medium.ttf");

/// Every built-in font, in the order of [`builtin_font_number`]: new ones go at the end.
#[cfg(not(feature = "mono"))]
const BUILTIN: [&[u8]; 3] = [INTER_REGULAR, INTER_SEMIBOLD, RAVEN_ICONS];
#[cfg(feature = "mono")]
const BUILTIN: [&[u8]; 4] = [INTER_REGULAR, INTER_SEMIBOLD, RAVEN_ICONS, FIRA_MONO];

/// The family name of the embedded icon font.
pub const ICON_FAMILY: &str = "RavenIcons";

/// The family name of the embedded default font.
pub const DEFAULT_FAMILY: &str = "Inter";

/// The family name of the embedded monospaced font, with the `mono` feature. It is also
/// what the generic `monospace` family resolves to.
#[cfg(feature = "mono")]
pub const MONO_FAMILY: &str = "Fira Mono";

/// Which built-in font these bytes are, as a number that does not change between
/// runs: what a recording stores instead of the font itself.
pub fn builtin_font_number(bytes: &[u8]) -> Option<u8> {
    BUILTIN.iter().position(|font| *font == bytes).map(|i| i as u8)
}

/// A built-in font by its [`builtin_font_number`], for drawing shaped glyphs back.
pub fn builtin_font(number: u8) -> Option<FontRef> {
    static FONTS: OnceLock<[FontRef; BUILTIN.len()]> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        BUILTIN.map(|bytes| {
            let blob = Blob::new(Arc::new(bytes));
            FontRef { id: blob.id(), data: Arc::new(blob), index: 0 }
        })
    });
    fonts.get(usize::from(number)).cloned()
}

/// The embedded Inter font file, regular or semibold, for documents that embed
/// the font they were measured with.
pub fn inter_font(semibold: bool) -> &'static [u8] {
    if semibold { INTER_SEMIBOLD } else { INTER_REGULAR }
}

/// Folds a character to what searches compare: lowercase, without accents, so
/// "regiao" finds "Região". Folding can change how many characters there are.
pub fn fold(c: char) -> impl Iterator<Item = char> {
    c.to_lowercase().map(|c| match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        // A number formatted with a non-breaking space is found with a plain one.
        '\u{a0}' => ' ',
        other => other,
    })
}

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

/// Work a [`TextSystem`] has done since it was created.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextStats {
    /// Times text was shaped, the costly part of showing it.
    pub shapes: u64,
}

/// Layouts kept per generation; two generations are alive, so up to twice this.
const CACHE_GENERATION: usize = 8192;
/// Longer texts are rare and take the most memory, so they are shaped each time.
const CACHE_LONGEST_TEXT: usize = 4096;

/// What identifies a shaped text: the text, its attributes and, once cut to fit, the
/// width. Hashed with fixed keys, because the same run must do the same work every time.
fn cache_key(text: &str, attrs: &TextAttrs, fit: Option<f32>) -> u64 {
    let mut hash = std::hash::DefaultHasher::new();
    text.hash(&mut hash);
    attrs.size.to_bits().hash(&mut hash);
    attrs.line_height.to_bits().hash(&mut hash);
    attrs.weight.to_bits().hash(&mut hash);
    attrs.letter_spacing.to_bits().hash(&mut hash);
    attrs.family.hash(&mut hash);
    fit.map(f32::to_bits).hash(&mut hash);
    hash.finish()
}

#[derive(Clone)]
struct CachedLayout {
    /// Checked on every hit: a hash can collide, and wrong text would be a visible bug.
    text: Box<str>,
    attrs: TextAttrs,
    layout: TextLayout,
}

/// Recently shaped text. Rebuilding a screen shows almost the same text as before, so
/// most lookups hit. What was not used for a whole generation goes away.
#[derive(Default)]
struct LayoutCache {
    current: HashMap<u64, CachedLayout>,
    previous: HashMap<u64, CachedLayout>,
}

impl LayoutCache {
    fn get(&mut self, key: u64, text: &str, attrs: &TextAttrs) -> Option<TextLayout> {
        let matches = |entry: &CachedLayout| &*entry.text == text && entry.attrs == *attrs;
        if let Some(entry) = self.current.get(&key).filter(|entry| matches(entry)) {
            return Some(entry.layout.clone());
        }
        let entry = self.previous.get(&key).filter(|entry| matches(entry))?.clone();
        let layout = entry.layout.clone();
        self.insert(key, entry);
        Some(layout)
    }

    fn put(&mut self, key: u64, text: &str, attrs: &TextAttrs, layout: &TextLayout) {
        if text.len() <= CACHE_LONGEST_TEXT {
            let entry = CachedLayout { text: text.into(), attrs: attrs.clone(), layout: layout.clone() };
            self.insert(key, entry);
        }
    }

    fn insert(&mut self, key: u64, entry: CachedLayout) {
        if self.current.len() >= CACHE_GENERATION {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current.insert(key, entry);
    }

    fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
    }
}

/// Font database plus reusable layout scratch space. Create one per app.
pub struct TextSystem {
    fonts: FontContext,
    layouts: LayoutContext<()>,
    stats: TextStats,
    cache: LayoutCache,
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
            stats: TextStats::default(),
            cache: LayoutCache::default(),
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
        #[cfg(feature = "mono")]
        {
            let mono = system.register_static_font(FIRA_MONO);
            for generic in [GenericFamily::Monospace, GenericFamily::UiMonospace] {
                system.fonts.collection.set_generic_families(generic, mono.iter().copied());
            }
        }
        system
    }

    fn register_static_font(&mut self, bytes: &'static [u8]) -> Vec<FamilyId> {
        let blob = Blob::new(Arc::new(bytes));
        self.fonts.collection.register_fonts(blob, None).into_iter().map(|(id, _)| id).collect()
    }

    /// Registers a font file (TTF/OTF/TTC) and returns the family names it provides.
    pub fn register_font(&mut self, bytes: Vec<u8>) -> Vec<String> {
        // A new font can change what any text resolves to.
        self.cache.clear();
        let ids: Vec<FamilyId> = self
            .fonts
            .collection
            .register_fonts(Blob::from(bytes), None)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        ids.into_iter().filter_map(|id| self.fonts.collection.family_name(id).map(str::to_owned)).collect()
    }

    /// What this system has done so far.
    pub fn stats(&self) -> TextStats {
        self.stats
    }

    /// Shapes `text`. Line breaking happens later in [`TextLayout::measure`].
    pub fn layout(&mut self, text: &str, attrs: &TextAttrs) -> TextLayout {
        let key = cache_key(text, attrs, None);
        if let Some(layout) = self.cache.get(key, text, attrs) {
            return layout;
        }
        let layout = self.shape(text, attrs);
        self.cache.put(key, text, attrs, &layout);
        layout
    }

    fn shape(&mut self, text: &str, attrs: &TextAttrs) -> TextLayout {
        self.stats.shapes += 1;
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
        TextLayout { layout, broken_at: None, align: TextAlign::Start }
    }

    /// Shapes `text` on one line, cut at a character boundary and ended with
    /// `…` so it fits `max_width`. Text that already fits comes back whole.
    pub fn layout_ellipsized(&mut self, text: &str, attrs: &TextAttrs, max_width: f32) -> TextLayout {
        let key = cache_key(text, attrs, Some(max_width));
        if let Some(layout) = self.cache.get(key, text, attrs) {
            return layout;
        }
        let layout = self.fit(text, attrs, max_width);
        self.cache.put(key, text, attrs, &layout);
        layout
    }

    fn fit(&mut self, text: &str, attrs: &TextAttrs, max_width: f32) -> TextLayout {
        let mut whole = self.layout(text, attrs);
        if whole.measure(None).width <= max_width {
            return whole;
        }
        let ends: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        let cut = |count: usize| {
            let end = ends.get(count).copied().unwrap_or(text.len());
            format!("{}…", text[..end].trim_end())
        };
        // The most characters that still fit, by bisection: shaping is the costly part.
        let (mut low, mut high) = (0, ends.len());
        while low < high {
            let middle = (low + high).div_ceil(2);
            if self.layout(&cut(middle), attrs).measure(None).width <= max_width {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let mut fitted = self.layout(&cut(low), attrs);
        fitted.measure(None);
        fitted
    }
}

/// One line of a broken [`TextLayout`], as reported by [`TextLayout::lines`].
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    /// Byte range of the line in the source text, trailing whitespace included.
    pub range: std::ops::Range<usize>,
    /// Width of the line's content, trailing whitespace excluded.
    pub width: f32,
    /// Top of the line box, from the top of the layout.
    pub top: f32,
    /// Baseline, from the top of the layout.
    pub baseline: f32,
    /// Height of the line box.
    pub height: f32,
}

/// One glyph of a [`TextLayout`], as reported by [`TextLayout::glyphs`].
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    /// The glyph's index in the font.
    pub id: u32,
    /// Horizontal position, from the left of the layout.
    pub x: f32,
    /// How far the next glyph advances.
    pub advance: f32,
    /// Byte range of the source text this glyph's cluster stands for.
    pub text: std::ops::Range<usize>,
}

/// Where the lines of a text sit across the width they are broken at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextAlign {
    /// At the start of the line: the left, in a left-to-right language.
    #[default]
    Start,
    /// In the middle.
    Center,
    /// At the end of the line: the right, in a left-to-right language.
    End,
}

/// Shaped text, ready to be broken into lines at any width and drawn.
#[derive(Clone)]
pub struct TextLayout {
    layout: Layout<()>,
    /// The width limit of the last line breaking, to skip redundant work.
    broken_at: Option<Option<f32>>,
    /// Where the lines sit across that width.
    align: TextAlign,
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
    /// Where the lines sit across the width the next [`Self::measure`] breaks them at. Lines
    /// shorter than that width move to the middle or the end; the size of the text does not
    /// change.
    pub fn set_align(&mut self, align: TextAlign) {
        if self.align != align {
            self.align = align;
            self.broken_at = None;
        }
    }

    /// Where the lines sit, as [`Self::set_align`] left it.
    pub fn align(&self) -> TextAlign {
        self.align
    }

    /// Breaks lines at `max_width` (or never, for `None`) and returns the text's size.
    pub fn measure(&mut self, max_width: Option<f32>) -> Size {
        if self.broken_at != Some(max_width) {
            self.layout.break_all_lines(max_width);
            let alignment = match self.align {
                TextAlign::Start => Alignment::Start,
                TextAlign::Center => Alignment::Center,
                TextAlign::End => Alignment::End,
            };
            self.layout.align(alignment, AlignmentOptions::default());
            self.broken_at = Some(max_width);
        }
        Size::new(self.layout.width(), self.layout.height())
    }

    /// The lines of the last [`Self::measure`], with their text ranges and
    /// positions (documents and PDF export draw text line by line).
    pub fn lines(&self) -> Vec<TextLine> {
        self.layout
            .lines()
            .map(|line| {
                let m = line.metrics();
                TextLine {
                    range: line.text_range(),
                    width: (m.advance - m.hanging_advance).max(0.0),
                    top: m.block_min_coord,
                    baseline: m.baseline,
                    height: m.block_max_coord - m.block_min_coord,
                }
            })
            .collect()
    }

    /// The glyphs of the last [`Self::measure`] in visual order, with the text
    /// each one stands for (document exporters embed glyphs, not characters).
    pub fn glyphs(&self) -> Vec<ShapedGlyph> {
        let mut glyphs = Vec::new();
        for line in self.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else { continue };
                let mut x = run.offset();
                for cluster in run.run().visual_clusters() {
                    let text = cluster.text_range();
                    for g in cluster.glyphs() {
                        glyphs.push(ShapedGlyph {
                            id: g.id,
                            x: x + g.x,
                            advance: g.advance,
                            text: text.clone(),
                        });
                        x += g.advance;
                    }
                }
            }
        }
        glyphs
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
    fn reports_lines_with_ranges_and_baselines() {
        let mut ts = TextSystem::new();
        let text = "one two three four five six";
        let mut layout = ts.layout(text, &TextAttrs::default());
        layout.measure(Some(60.0));
        let lines = layout.lines();
        assert!(lines.len() > 1);
        assert_eq!(lines[0].range.start, 0);
        assert_eq!(lines.last().map(|l| l.range.end), Some(text.len()));
        for pair in lines.windows(2) {
            assert_eq!(pair[0].range.end, pair[1].range.start, "lines cover the text without gaps");
            assert!(pair[1].baseline > pair[0].baseline);
        }
        assert!(lines.iter().all(|l| l.width <= 60.0 + 0.5 && l.height == 22.0));
        assert!(text[lines[0].range.clone()].ends_with(' '), "trailing space stays in the range");
    }

    #[test]
    fn the_same_text_is_shaped_once() {
        let mut ts = TextSystem::new();
        let attrs = TextAttrs::default();
        let mut first = ts.layout("Repeated text", &attrs);
        let mut again = ts.layout("Repeated text", &attrs);
        assert_eq!(ts.stats().shapes, 1);
        assert_eq!(first.measure(None), again.measure(None));
        ts.layout("Other text", &attrs);
        ts.layout("Repeated text", &TextAttrs { weight: 600.0, ..TextAttrs::default() });
        assert_eq!(ts.stats().shapes, 3, "other text and other attributes are other layouts");
    }

    #[test]
    fn breaking_a_cached_layout_leaves_the_next_one_alone() {
        let mut ts = TextSystem::new();
        let text = "one two three four five six";
        let mut narrow = ts.layout(text, &TextAttrs::default());
        let narrow = narrow.measure(Some(60.0));
        let wide = ts.layout(text, &TextAttrs::default()).measure(None);
        assert!(narrow.height > wide.height, "the cache keeps no one's line breaks");
    }

    #[test]
    fn cutting_text_to_fit_is_kept_per_width() {
        let mut ts = TextSystem::new();
        let attrs = TextAttrs::default();
        let text = "A long sentence that will not fit in the space it was given";
        ts.layout_ellipsized(text, &attrs, 100.0);
        let shapes = ts.stats().shapes;
        let mut fitted = ts.layout_ellipsized(text, &attrs, 100.0);
        assert_eq!(ts.stats().shapes, shapes, "the bisection does not run again");
        assert!(fitted.measure(None).width <= 100.0);
        let mut wider = ts.layout_ellipsized(text, &attrs, 200.0);
        assert!(wider.measure(None).width > fitted.measure(None).width);
    }

    #[test]
    fn a_new_font_empties_the_cache() {
        let mut ts = TextSystem::new();
        ts.layout("Text", &TextAttrs::default());
        ts.register_font(INTER_REGULAR.to_vec());
        ts.layout("Text", &TextAttrs::default());
        assert_eq!(ts.stats().shapes, 2);
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

    #[test]
    fn the_lines_sit_at_the_start_in_the_middle_or_at_the_end() {
        let mut ts = TextSystem::new();
        let first_x = |ts: &mut TextSystem, align: TextAlign| {
            let mut layout = ts.layout("Raven", &TextAttrs::default());
            layout.set_align(align);
            let width = layout.measure(Some(200.0)).width;
            let x = layout.glyphs().first().expect("a glyph").x;
            (x, width)
        };
        let (start, width) = first_x(&mut ts, TextAlign::Start);
        let (center, centered_width) = first_x(&mut ts, TextAlign::Center);
        let (end, _) = first_x(&mut ts, TextAlign::End);
        assert!(start.abs() < 0.5, "{start}");
        assert!((center - (200.0 - width) / 2.0).abs() < 1.0, "{center} of {width}");
        assert!((end - (200.0 - width)).abs() < 1.0, "{end} of {width}");
        // Where the lines sit does not change how big the text is.
        assert_eq!(width, centered_width);
    }

    #[test]
    fn changing_the_alignment_breaks_the_lines_again() {
        let mut ts = TextSystem::new();
        let mut layout = ts.layout("Raven", &TextAttrs::default());
        layout.measure(Some(200.0));
        layout.set_align(TextAlign::End);
        layout.measure(Some(200.0));
        assert!(layout.glyphs().first().expect("a glyph").x > 100.0);
    }
}
