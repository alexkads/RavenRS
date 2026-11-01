//! PDF export for RavenRS Reporting: draws a [`Document`] as vector pages, with
//! subsets of the fonts it was measured with embedded, so the file looks the
//! same everywhere and its text can be selected and searched.
//!
//! ```
//! use ravenrs_report::{Band, Control, Report, ReportEngine};
//! use serde_json::json;
//!
//! let report = Report::new("Sales").detail(Band::new(18.0).control(Control::field("[Customer]").at(0.0, 0.0, 300.0, 18.0)));
//! let document = ReportEngine::new().render(&report, &json!([{ "Customer": "Ana" }]));
//! let pdf = ravenrs_report_pdf::to_pdf(&document)?;
//! assert!(pdf.starts_with(b"%PDF-"));
//! # Ok::<(), ravenrs_report_pdf::PdfError>(())
//! ```

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::rc::Rc;

use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::types::{
    ActionType, AnnotationType, CidFontType, FontFlags, PageMode, SystemInfo, UnicodeCmap,
};
use pdf_writer::writers::Resources;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PdfRect, Ref, Str, TextStr};
use ravenrs_core::{Color, Rect};
use ravenrs_report::vector::{self, PathSegment};
use ravenrs_report::{Document, DocumentImage, ImageFormat, Item, Page, TextBlock};
use ravenrs_text::{TextAttrs, TextSystem};
use skrifa::instance::{LocationRef, Size};
use skrifa::{GlyphId, MetadataProvider};
use subsetter::GlyphRemapper;

/// Why a document could not be written as PDF.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PdfError {
    /// A font could not be read or reduced to the glyphs in use.
    Font(String),
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Font(reason) => write!(f, "could not embed a font: {reason}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// Writes `document` as a PDF 1.7 file.
pub fn to_pdf(document: &Document) -> Result<Vec<u8>, PdfError> {
    let mut writer = Writer::new()?;
    let contents: Vec<(Vec<u8>, &Page)> =
        document.pages.iter().map(|page| (writer.page_content(page), page)).collect();
    writer.finish(document, contents)
}

/// Text is shaped at this size, so positions come out in PDF's text space units.
const UNITS: f32 = 1000.0;

/// A shaped line of text, in thousandths of the font size.
struct Shaped {
    glyphs: Vec<ShapedGlyph>,
}

struct ShapedGlyph {
    /// The glyph's index in the subset, which is also its CID.
    cid: u16,
    x: f32,
}

/// An embedded font and the glyphs used from it so far.
struct Font {
    data: &'static [u8],
    bold: bool,
    remapper: GlyphRemapper,
    /// Original glyph of each subset glyph.
    originals: Vec<u16>,
    /// Width of each subset glyph, in thousandths of the font size.
    widths: Vec<f32>,
    /// The text each subset glyph stands for, when known.
    unicode: Vec<Option<String>>,
    /// Width of a glyph in the font, by original glyph index.
    advances: Vec<f32>,
}

impl Font {
    fn new(bold: bool) -> Result<Self, PdfError> {
        let data = ravenrs_text::inter_font(bold);
        let font = skrifa::FontRef::new(data).map_err(|e| PdfError::Font(e.to_string()))?;
        let metrics = font.glyph_metrics(Size::new(UNITS), LocationRef::default());
        let advances: Vec<f32> = (0..metrics.glyph_count())
            .map(|g| metrics.advance_width(GlyphId::new(g)).unwrap_or(0.0))
            .collect();
        let notdef = advance(&advances, 0);
        Ok(Self {
            data,
            bold,
            remapper: GlyphRemapper::new(),
            originals: vec![0],
            widths: vec![notdef],
            unicode: vec![None],
            advances,
        })
    }

    /// The CID of an original glyph, adding it to the subset the first time.
    fn cid(&mut self, glyph: u16, text: &str) -> u16 {
        let cid = self.remapper.remap(glyph);
        if usize::from(cid) == self.originals.len() {
            self.originals.push(glyph);
            self.widths.push(advance(&self.advances, glyph));
            self.unicode.push(None);
        }
        let slot = &mut self.unicode[usize::from(cid)];
        if slot.is_none() && !text.is_empty() {
            *slot = Some(text.to_owned());
        }
        cid
    }
}

fn advance(advances: &[f32], glyph: u16) -> f32 {
    advances.get(usize::from(glyph)).copied().unwrap_or(0.0)
}

struct Writer {
    text: TextSystem,
    fonts: [Font; 2],
    shaped: HashMap<(bool, String), Rc<Shaped>>,
    /// Opacities in use, as 0–255; each becomes a graphics state.
    alphas: BTreeSet<u8>,
    /// Images in use; a logo on every page is stored once. Position is the name's number.
    images: Vec<DocumentImage>,
    image_index: HashMap<u64, usize>,
}

impl Writer {
    fn new() -> Result<Self, PdfError> {
        Ok(Self {
            text: TextSystem::new(),
            fonts: [Font::new(false)?, Font::new(true)?],
            shaped: HashMap::new(),
            alphas: BTreeSet::new(),
            images: Vec::new(),
            image_index: HashMap::new(),
        })
    }

    fn shape(&mut self, text: &str, bold: bool) -> Rc<Shaped> {
        let key = (bold, text.to_owned());
        if let Some(shaped) = self.shaped.get(&key) {
            return shaped.clone();
        }
        let attrs = TextAttrs {
            size: UNITS,
            line_height: UNITS,
            weight: if bold { 600.0 } else { 400.0 },
            ..TextAttrs::default()
        };
        let mut layout = self.text.layout(text, &attrs);
        layout.measure(None);
        let font = &mut self.fonts[usize::from(bold)];
        let glyphs = layout
            .glyphs()
            .into_iter()
            .map(|g| {
                let id = u16::try_from(g.id).unwrap_or(0);
                ShapedGlyph { cid: font.cid(id, text.get(g.text).unwrap_or_default()), x: g.x }
            })
            .collect();
        let shaped = Rc::new(Shaped { glyphs });
        self.shaped.insert(key, shaped.clone());
        shaped
    }

    /// The drawing commands of a page, compressed.
    fn page_content(&mut self, page: &Page) -> Vec<u8> {
        let mut content = Content::new();
        let h = page.height;
        for item in &page.items {
            match item {
                Item::Rect { rect, fill, stroke, radius } => {
                    self.draw(&mut content, *fill, *stroke, |c| {
                        write_path(c, &vector::rectangle(*rect, *radius), h)
                    });
                }
                Item::Ellipse { rect, fill, stroke } => {
                    self.draw(&mut content, *fill, *stroke, |c| write_path(c, &vector::ellipse(*rect), h));
                }
                Item::Line { from, to, width, color } => {
                    let alpha = self.begin_alpha(&mut content, *color);
                    content.set_stroke_rgb(color.r, color.g, color.b);
                    content.set_line_width(*width);
                    content.move_to(from.x, h - from.y);
                    content.line_to(to.x, h - to.y);
                    content.stroke();
                    end_alpha(&mut content, alpha);
                }
                Item::Text(block) => self.draw_text(&mut content, block, h),
                Item::Image { rect, clip, image } => {
                    let index = self.image_number(image);
                    content.save_state();
                    content.rect(clip.x(), h - clip.max_y(), clip.width(), clip.height());
                    content.clip_nonzero();
                    content.end_path();
                    content.transform([rect.width(), 0.0, 0.0, rect.height(), rect.x(), h - rect.max_y()]);
                    content.x_object(Name(image_name(index).as_bytes()));
                    content.restore_state();
                }
                Item::Vector { shapes, .. } => {
                    for shape in shapes {
                        self.draw(&mut content, shape.fill, shape.stroke, |c| {
                            write_path(c, &shape.path, h);
                        });
                    }
                }
                // Links and bookmarks print nothing; they are written with the page.
                _ => {}
            }
        }
        compress_to_vec_zlib(&content.finish(), 6)
    }

    fn image_number(&mut self, image: &DocumentImage) -> usize {
        let images = &mut self.images;
        *self.image_index.entry(image.pixels().id()).or_insert_with(|| {
            images.push(image.clone());
            images.len() - 1
        })
    }

    fn draw(
        &mut self,
        content: &mut Content,
        fill: Option<Color>,
        stroke: Option<(f32, Color)>,
        path: impl Fn(&mut Content),
    ) {
        if let Some(fill) = fill.filter(|c| c.a > 0.0) {
            let alpha = self.begin_alpha(content, fill);
            content.set_fill_rgb(fill.r, fill.g, fill.b);
            path(content);
            content.fill_nonzero();
            end_alpha(content, alpha);
        }
        if let Some((width, color)) = stroke.filter(|(w, c)| *w > 0.0 && c.a > 0.0) {
            let alpha = self.begin_alpha(content, color);
            content.set_stroke_rgb(color.r, color.g, color.b);
            content.set_line_width(width);
            path(content);
            content.stroke();
            end_alpha(content, alpha);
        }
    }

    /// Opens a graphics state for a translucent color; returns whether it did.
    fn begin_alpha(&mut self, content: &mut Content, color: Color) -> bool {
        let alpha = (color.a.clamp(0.0, 1.0) * 255.0).round() as u8;
        if alpha == 255 {
            return false;
        }
        self.alphas.insert(alpha);
        content.save_state();
        content.set_parameters(Name(alpha_name(alpha).as_bytes()));
        true
    }

    fn draw_text(&mut self, content: &mut Content, block: &TextBlock, h: f32) {
        // Viewers clip text to its box; clipping only when needed keeps the file small.
        let c = block.clip;
        let overflows = block.lines.iter().any(|l| {
            l.x < c.x() - 0.5
                || l.x + l.width > c.max_x() + 0.5
                || l.top < c.y() - 0.5
                || l.baseline + block.size * 0.3 > c.max_y() + 0.5
        });
        let alpha = self.begin_alpha(content, block.color);
        if overflows {
            content.save_state();
            content.rect(c.x(), h - c.max_y(), c.width(), c.height());
            content.clip_nonzero();
            content.end_path();
        }
        content.set_fill_rgb(block.color.r, block.color.g, block.color.b);
        content.begin_text();
        content.set_font(Name(font_name(block.bold).as_bytes()), block.size);
        let mut encoded = Vec::new();
        for line in &block.lines {
            let shaped = self.shape(&line.text, block.bold);
            if shaped.glyphs.is_empty() {
                continue;
            }
            content.set_text_matrix([1.0, 0.0, 0.0, 1.0, line.x, h - line.baseline]);
            let widths = &self.fonts[usize::from(block.bold)].widths;
            let mut show = content.show_positioned();
            let mut items = show.items();
            let mut pen = 0.0;
            for glyph in &shaped.glyphs {
                // Kerning and shaping move a glyph away from where the font's width would put it.
                if (glyph.x - pen).abs() > 0.01 {
                    if !encoded.is_empty() {
                        items.show(Str(&encoded));
                        encoded.clear();
                    }
                    items.adjust(pen - glyph.x);
                    pen = glyph.x;
                }
                encoded.extend_from_slice(&glyph.cid.to_be_bytes());
                pen += widths[usize::from(glyph.cid)];
            }
            if !encoded.is_empty() {
                items.show(Str(&encoded));
                encoded.clear();
            }
            items.finish();
            show.finish();
        }
        content.end_text();
        if overflows {
            content.restore_state();
        }
        end_alpha(content, alpha);
    }

    fn finish(self, document: &Document, contents: Vec<(Vec<u8>, &Page)>) -> Result<Vec<u8>, PdfError> {
        let mut next = 1;
        let mut alloc = || {
            let id = Ref::new(next);
            next += 1;
            id
        };
        let (catalog, tree, resources, info) = (alloc(), alloc(), alloc(), alloc());
        let page_ids: Vec<(Ref, Ref)> = contents.iter().map(|_| (alloc(), alloc())).collect();
        let mut pdf = Pdf::new();
        let outline = write_outline(&mut pdf, &contents, &page_ids, &mut alloc);
        let mut root = pdf.catalog(catalog);
        root.pages(tree);
        if let Some(outline) = outline {
            root.outlines(outline);
            root.page_mode(PageMode::UseOutlines);
        }
        root.finish();
        let mut about = pdf.document_info(info);
        about.title(TextStr(&document.title));
        about.producer(TextStr("RavenRS Reporting"));
        about.finish();

        let font_ids: Vec<Ref> =
            self.fonts.iter().map(|font| write_font(&mut pdf, font, &mut alloc)).collect::<Result<_, _>>()?;

        let alpha_ids: Vec<(u8, Ref)> = self.alphas.iter().map(|a| (*a, alloc())).collect();
        for (alpha, id) in &alpha_ids {
            let value = f32::from(*alpha) / 255.0;
            pdf.ext_graphics(*id).non_stroking_alpha(value).stroking_alpha(value);
        }
        let image_ids: Vec<Ref> =
            self.images.iter().map(|image| write_image(&mut pdf, image, &mut alloc)).collect();
        let mut res = pdf.indirect(resources).start::<Resources>();
        let mut fonts = res.fonts();
        for (bold, id) in [false, true].into_iter().zip(&font_ids) {
            fonts.pair(Name(font_name(bold).as_bytes()), *id);
        }
        fonts.finish();
        let mut states = res.ext_g_states();
        for (alpha, id) in &alpha_ids {
            states.pair(Name(alpha_name(*alpha).as_bytes()), *id);
        }
        states.finish();
        let mut x_objects = res.x_objects();
        for (index, id) in image_ids.iter().enumerate() {
            x_objects.pair(Name(image_name(index).as_bytes()), *id);
        }
        x_objects.finish();
        res.finish();

        let mut pages = pdf.pages(tree);
        pages.kids(page_ids.iter().map(|(page, _)| *page));
        pages.count(i32::try_from(page_ids.len()).unwrap_or(i32::MAX));
        pages.pair(Name(b"Resources"), resources);
        pages.finish();
        for ((data, page), (id, content_id)) in contents.iter().zip(&page_ids) {
            let mut out = pdf.page(*id);
            out.parent(tree);
            out.media_box(PdfRect::new(0.0, 0.0, page.width, page.height));
            out.contents(*content_id);
            let links: Vec<(&Rect, &str)> = page
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Link { rect, url } => Some((rect, url.as_str())),
                    _ => None,
                })
                .collect();
            let annotation_ids: Vec<Ref> = links.iter().map(|_| alloc()).collect();
            if !annotation_ids.is_empty() {
                out.annotations(annotation_ids.iter().copied());
            }
            out.finish();
            pdf.stream(*content_id, data).filter(Filter::FlateDecode);
            for ((rect, url), annotation_id) in links.iter().zip(&annotation_ids) {
                let mut annotation = pdf.annotation(*annotation_id);
                annotation.subtype(AnnotationType::Link);
                annotation.rect(PdfRect::new(
                    rect.x(),
                    page.height - rect.max_y(),
                    rect.max_x(),
                    page.height - rect.y(),
                ));
                annotation.border(0.0, 0.0, 0.0, None);
                annotation.action().action_type(ActionType::Uri).uri(Str(url.as_bytes()));
            }
        }
        Ok(pdf.finish())
    }
}

fn end_alpha(content: &mut Content, opened: bool) {
    if opened {
        content.restore_state();
    }
}

fn font_name(bold: bool) -> &'static str {
    if bold { "F1" } else { "F0" }
}

fn alpha_name(alpha: u8) -> String {
    format!("A{alpha}")
}

fn image_name(index: usize) -> String {
    format!("I{index}")
}

/// Embeds an image. A JPEG goes in as the original file when PDF can show it
/// directly, so a photo keeps its quality and size; anything else as pixels,
/// with transparency as a separate mask.
fn write_image(pdf: &mut Pdf, image: &DocumentImage, alloc: &mut impl FnMut() -> Ref) -> Ref {
    let id = alloc();
    let pixels = image.pixels();
    let (width, height) = (pixels.width() as i32, pixels.height() as i32);
    if image.format() == ImageFormat::Jpeg && matches!(image.channels(), 1 | 3) {
        let mut out = pdf.image_xobject(id, image.encoded());
        out.filter(Filter::DctDecode);
        out.width(width);
        out.height(height);
        if image.channels() == 1 {
            out.color_space().device_gray();
        } else {
            out.color_space().device_rgb();
        }
        out.bits_per_component(8);
        return id;
    }
    let (rgba, _) = pixels.pixels().as_chunks::<4>();
    let rgb: Vec<u8> = rgba.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
    let mask = rgba.iter().any(|p| p[3] != 255).then(|| {
        let alpha: Vec<u8> = rgba.iter().map(|p| p[3]).collect();
        let mask_id = alloc();
        let compressed = compress_to_vec_zlib(&alpha, 6);
        let mut out = pdf.image_xobject(mask_id, &compressed);
        out.filter(Filter::FlateDecode);
        out.width(width);
        out.height(height);
        out.color_space().device_gray();
        out.bits_per_component(8);
        out.finish();
        mask_id
    });
    let compressed = compress_to_vec_zlib(&rgb, 6);
    let mut out = pdf.image_xobject(id, &compressed);
    out.filter(Filter::FlateDecode);
    out.width(width);
    out.height(height);
    out.color_space().device_rgb();
    out.bits_per_component(8);
    if let Some(mask) = mask {
        out.s_mask(mask);
    }
    id
}

/// A bookmark in the outline tree.
struct OutlineNode<'d> {
    id: Ref,
    title: &'d str,
    page: Ref,
    top: f32,
    parent: Option<usize>,
    children: Vec<usize>,
    descendants: i32,
}

/// Writes the outline from the pages' bookmarks; `None` when there are none.
fn write_outline(
    pdf: &mut Pdf,
    contents: &[(Vec<u8>, &Page)],
    page_ids: &[(Ref, Ref)],
    alloc: &mut impl FnMut() -> Ref,
) -> Option<Ref> {
    let mut nodes: Vec<OutlineNode<'_>> = Vec::new();
    let mut roots = Vec::new();
    // The open path: the level and node of each ancestor of what comes next.
    let mut path: Vec<(u32, usize)> = Vec::new();
    for ((_, page), (page_id, _)) in contents.iter().zip(page_ids) {
        for item in &page.items {
            let Item::Bookmark { title, level, y } = item else { continue };
            // The parent is the latest entry at a lesser level, so a level that skips one still fits.
            while path.last().is_some_and(|(open, _)| open >= level) {
                path.pop();
            }
            let index = nodes.len();
            let parent = path.last().map(|(_, node)| *node);
            nodes.push(OutlineNode {
                id: alloc(),
                title,
                page: *page_id,
                top: page.height - y,
                parent,
                children: Vec::new(),
                descendants: 0,
            });
            match parent {
                Some(parent) => nodes[parent].children.push(index),
                None => roots.push(index),
            }
            path.push((*level, index));
        }
    }
    if nodes.is_empty() {
        return None;
    }
    // Children come after their parents, so going backwards adds them up.
    for index in (0..nodes.len()).rev() {
        if let Some(parent) = nodes[index].parent {
            nodes[parent].descendants += nodes[index].descendants + 1;
        }
    }
    let root = alloc();
    let mut outline = pdf.outline(root);
    outline.first(nodes[roots[0]].id);
    outline.last(nodes[roots[roots.len() - 1]].id);
    outline.count(nodes.len() as i32);
    outline.finish();
    for (index, node) in nodes.iter().enumerate() {
        let siblings = node.parent.map_or(&roots, |parent| &nodes[parent].children);
        let at = siblings.iter().position(|sibling| *sibling == index).unwrap_or(0);
        let mut item = pdf.outline_item(node.id);
        item.title(TextStr(node.title));
        item.parent(node.parent.map_or(root, |parent| nodes[parent].id));
        if at > 0 {
            item.prev(nodes[siblings[at - 1]].id);
        }
        if let Some(next) = siblings.get(at + 1) {
            item.next(nodes[*next].id);
        }
        if let (Some(first), Some(last)) = (node.children.first(), node.children.last()) {
            item.first(nodes[*first].id);
            item.last(nodes[*last].id);
            item.count(node.descendants);
        }
        item.dest().page(node.page).xyz(0.0, node.top, None);
    }
    Some(root)
}

/// Embeds a font's subset; returns the reference pages use.
fn write_font(pdf: &mut Pdf, font: &Font, alloc: &mut impl FnMut() -> Ref) -> Result<Ref, PdfError> {
    let (type0, cid, descriptor, file, cmap) = (alloc(), alloc(), alloc(), alloc(), alloc());
    let name =
        format!("{}+Inter-{}", subset_tag(&font.originals), if font.bold { "SemiBold" } else { "Regular" });
    let name = Name(name.as_bytes());

    let subset =
        subsetter::subset(font.data, 0, &font.remapper).map_err(|e| PdfError::Font(e.to_string()))?;
    let compressed = compress_to_vec_zlib(&subset, 6);
    pdf.stream(file, &compressed).filter(Filter::FlateDecode).pair(Name(b"Length1"), subset.len() as i32);

    let reader = skrifa::FontRef::new(font.data).map_err(|e| PdfError::Font(e.to_string()))?;
    let metrics = reader.metrics(Size::new(UNITS), LocationRef::default());
    let bbox = metrics.bounds.map_or(PdfRect::new(0.0, -300.0, 1000.0, 1000.0), |b| {
        PdfRect::new(b.x_min, b.y_min, b.x_max, b.y_max)
    });
    let mut desc = pdf.font_descriptor(descriptor);
    desc.name(name);
    desc.flags(FontFlags::NON_SYMBOLIC);
    desc.bbox(bbox);
    desc.italic_angle(0.0);
    desc.ascent(metrics.ascent);
    desc.descent(metrics.descent);
    desc.cap_height(metrics.cap_height.unwrap_or(metrics.ascent));
    desc.stem_v(if font.bold { 120.0 } else { 80.0 });
    desc.font_file2(file);
    desc.finish();

    let system = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 };
    let mut cid_font = pdf.cid_font(cid);
    cid_font.subtype(CidFontType::Type2);
    cid_font.base_font(name);
    cid_font.system_info(system);
    cid_font.font_descriptor(descriptor);
    cid_font.cid_to_gid_map_predefined(Name(b"Identity"));
    let mut widths = cid_font.widths();
    widths.consecutive(0, font.widths.iter().copied());
    widths.finish();
    cid_font.finish();

    let mut to_unicode = UnicodeCmap::<u16>::new(Name(b"Custom"), system);
    for (cid, text) in font.unicode.iter().enumerate() {
        if let (Some(text), Ok(cid)) = (text, u16::try_from(cid)) {
            to_unicode.pair_with_multiple(cid, text.chars());
        }
    }
    let unicode = compress_to_vec_zlib(&to_unicode.finish(), 6);
    pdf.cmap(cmap, &unicode).filter(Filter::FlateDecode);

    let mut out = pdf.type0_font(type0);
    out.base_font(name);
    out.encoding_predefined(Name(b"Identity-H"));
    out.descendant_font(cid);
    out.to_unicode(cmap);
    out.finish();
    Ok(type0)
}

/// The six capital letters PDF requires in front of a subset font's name.
fn subset_tag(glyphs: &[u16]) -> String {
    let mut hash = glyphs.iter().fold(2_166_136_261_u32, |h, g| (h ^ u32::from(*g)).wrapping_mul(16_777_619));
    (0..6)
        .map(|_| {
            let letter = char::from(b'A' + (hash % 26) as u8);
            hash /= 26;
            letter
        })
        .collect()
}

/// A shape's outline, with `y` measured from the top of a page of height `h`.
fn write_path(content: &mut Content, path: &[PathSegment], h: f32) {
    for segment in path {
        match segment {
            PathSegment::Move(p) => {
                content.move_to(p.x, h - p.y);
            }
            PathSegment::Line(p) => {
                content.line_to(p.x, h - p.y);
            }
            PathSegment::Cubic(a, b, c) => {
                content.cubic_to(a.x, h - a.y, b.x, h - b.y, c.x, h - c.y);
            }
            PathSegment::Close => {
                content.close_path();
            }
            _ => {}
        }
    }
}
