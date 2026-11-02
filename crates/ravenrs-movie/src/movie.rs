//! A film in memory and in a file: what the app drew, each thing stored once.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::Arc;

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use ravenrs_core::{
    Border, Brush, Color, FontRef, GlyphRun, ImageData, ImageQuad, LinearGradient, Point, PositionedGlyph,
    Primitive, Quad, Rect, Shadow, Size,
};
use serde::{Deserialize, Serialize};

use crate::project::{OverlayKind, Project};

const MAGIC: &[u8] = b"RAVENMOVIE2\n";

/// A picture in a film needs no more pixels than twice what it is drawn at.
const IMAGE_DETAIL: f32 = 2.0;

/// What can go wrong with a film.
#[derive(Debug)]
#[non_exhaustive]
pub enum MovieError {
    /// The file is not a film, or it was cut short.
    Damaged(String),
    /// The film has no frames.
    Empty,
}

impl std::fmt::Display for MovieError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Damaged(why) => write!(f, "the film is damaged: {why}"),
            Self::Empty => f.write_str("the film has no frames"),
        }
    }
}

impl std::error::Error for MovieError {}

/// A part of the film, in the list of chapters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    /// Its name.
    pub title: String,
    /// What it teaches.
    pub text: String,
    /// When it starts, in milliseconds.
    pub at_ms: u64,
}

/// What a spot on the screen does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HotspotKind {
    /// The film stops here until someone clicks the spot.
    Stop,
    /// The spot's text shows while it lasts; clicking it brings the text back.
    Note,
}

/// A spot on the screen that talks to whoever is watching.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hotspot {
    /// What it does.
    pub kind: HotspotKind,
    /// Where, in the logical pixels of the app.
    pub x: f32,
    /// Where, in the logical pixels of the app.
    pub y: f32,
    /// Its width.
    pub width: f32,
    /// Its height.
    pub height: f32,
    /// In a few words.
    pub title: String,
    /// What to say.
    pub text: String,
    /// When it appears, in milliseconds.
    pub from_ms: u64,
    /// When it goes away, in milliseconds.
    pub until_ms: u64,
}

impl Hotspot {
    /// Where it is on the screen of the app.
    pub fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// Whether it talks about the screen as a whole, with no spot to mark. It
    /// has no size, and its box shows in the middle of the film.
    pub fn is_general(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// A sound a film makes by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FilmSound {
    Click,
    Key,
}

/// Where the pointer was at a moment.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointerKey {
    /// When, in milliseconds.
    pub at_ms: u64,
    /// Where across, in logical pixels.
    pub x: f32,
    /// Where down, in logical pixels.
    pub y: f32,
}

fn tidy(value: f32) -> f32 {
    (value * 64.0).round() / 64.0
}

fn tidy_color(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a].map(|c| (c * 1024.0).round() / 1024.0)
}

fn tidy_rect(rect: Rect) -> [f32; 4] {
    [rect.x(), rect.y(), rect.width(), rect.height()].map(tidy)
}

fn color_of([r, g, b, a]: [f32; 4]) -> Color {
    Color::rgba(r, g, b, a)
}

fn rect_of([x, y, width, height]: [f32; 4]) -> Rect {
    Rect::new(x, y, width, height)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum StoredBrush {
    S([f32; 4]),
    G([f32; 2], [f32; 2], [f32; 4], [f32; 4]),
}

/// One thing the app drew, in as few characters as a file can have it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Stored {
    Q {
        r: [f32; 4],
        c: [f32; 4],
        f: StoredBrush,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<(f32, [f32; 4])>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        s: Option<([f32; 2], f32, [f32; 4])>,
    },
    T {
        f: u16,
        s: f32,
        c: [f32; 4],
        y: f32,
        g: Vec<(u32, f32)>,
    },
    I {
        i: u16,
        r: [f32; 4],
        u: [f32; 4],
        c: [f32; 4],
        o: f32,
    },
    C([f32; 4]),
    P,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum StoredFont {
    Builtin(u8),
    Embedded { blob: u32, index: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredImage {
    width: u32,
    height: u32,
    blob: u32,
}

/// A frame as the difference from the one before: this many things stay at the
/// start, this many at the end, and these come between.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredFrame {
    at: u64,
    head: u32,
    tail: u32,
    mid: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Header {
    logical: [f32; 2],
    background: [f32; 4],
    duration_ms: u64,
    fonts: Vec<StoredFont>,
    images: Vec<StoredImage>,
    shapes: Vec<Stored>,
    frames: Vec<StoredFrame>,
    pointer: Vec<PointerKey>,
    clicks: Vec<PointerKey>,
    /// When a key was pressed. Films recorded before there was sound have none.
    #[serde(default)]
    keys: Vec<u64>,
    chapters: Vec<Chapter>,
    hotspots: Vec<Hotspot>,
    blob_lengths: Vec<u64>,
}

#[derive(Debug)]
struct Film {
    header: Header,
    blobs: Vec<Vec<u8>>,
    primitives: Vec<Primitive>,
    frames: Vec<(u64, Rc<[u32]>)>,
}

/// A film and its edits. Cloning is cheap: the film itself is shared.
#[derive(Clone, Debug)]
pub struct Movie {
    film: Rc<Film>,
    project: Project,
    chapters: Rc<[Chapter]>,
    hotspots: Rc<[Hotspot]>,
    sounds: Rc<[(u64, FilmSound)]>,
}

impl Movie {
    fn with(film: Rc<Film>, project: Project) -> Self {
        let (chapters, hotspots) = edited_marks(&film.header, &project);
        let sounds = edited_sounds(&film.header, &project);
        Self { film, project, chapters: chapters.into(), hotspots: hotspots.into(), sounds: sounds.into() }
    }

    /// The same film with these edits.
    pub fn with_project(&self, project: Project) -> Self {
        Self::with(self.film.clone(), project)
    }

    /// Its edits.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// The size of the app it shows, in logical pixels.
    pub fn logical_size(&self) -> Size {
        Size::new(self.film.header.logical[0], self.film.header.logical[1])
    }

    /// The color behind everything.
    pub fn background(&self) -> Color {
        color_of(self.film.header.background)
    }

    /// How long it runs with its edits, in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.project.duration_ms()
    }

    /// How long the film runs with no edits.
    pub fn film_duration_ms(&self) -> u64 {
        self.film.header.duration_ms
    }

    /// Its chapters, on the edited timeline. A chapter that was cut starts where
    /// the cut left it, unless nothing of it remains.
    pub fn chapters(&self) -> &[Chapter] {
        &self.chapters
    }

    /// A moment to show before it plays: past any fade from black at the start.
    pub fn poster_ms(&self) -> u64 {
        self.project
            .overlays
            .iter()
            .filter(|o| matches!(o.kind, OverlayKind::FadeIn) && o.start_ms == 0)
            .map(crate::project::Overlay::end_ms)
            .max()
            .unwrap_or(0)
            .min(self.duration_ms())
    }

    /// The chapter playing at `ms` of the timeline.
    pub fn chapter_at(&self, ms: u64) -> Option<usize> {
        self.chapters.iter().rposition(|chapter| chapter.at_ms <= ms)
    }

    /// Its hotspots, on the edited timeline.
    pub fn hotspots(&self) -> &[Hotspot] {
        &self.hotspots
    }

    /// The clicks and keys after `from_ms` and up to `to_ms` of the timeline.
    pub(crate) fn sounds_between(&self, from_ms: u64, to_ms: u64) -> &[(u64, FilmSound)] {
        let start = self.sounds.partition_point(|(at, _)| *at <= from_ms);
        let end = self.sounds.partition_point(|(at, _)| *at <= to_ms);
        self.sounds.get(start..end).unwrap_or(&[])
    }

    /// What the app drew at this moment of the film.
    pub fn draw_list(&self, film_ms: u64) -> Option<(usize, Rc<[u32]>)> {
        let frames = &self.film.frames;
        let index = frames.partition_point(|(at, _)| *at <= film_ms).saturating_sub(1);
        frames.get(index).map(|(_, list)| (index, list.clone()))
    }

    /// A thing the app drew, from a [`Self::draw_list`].
    pub fn primitive(&self, index: u32) -> Option<&Primitive> {
        self.film.primitives.get(index as usize)
    }

    /// Every frame of the film: when it starts and what it draws.
    pub(crate) fn frames(&self) -> &[(u64, Rc<[u32]>)] {
        &self.film.frames
    }

    /// Everywhere the pointer was noted.
    pub(crate) fn pointer_keys(&self) -> &[PointerKey] {
        &self.film.header.pointer
    }

    /// Every click.
    pub(crate) fn click_keys(&self) -> &[PointerKey] {
        &self.film.header.clicks
    }

    /// Where the pointer was at this moment of the film.
    pub fn pointer_at(&self, film_ms: u64) -> Option<Point> {
        let keys = &self.film.header.pointer;
        let after = keys.partition_point(|key| key.at_ms <= film_ms);
        match (after.checked_sub(1).and_then(|i| keys.get(i)), keys.get(after)) {
            (Some(a), Some(b)) if b.at_ms > a.at_ms => {
                let t = (film_ms - a.at_ms) as f32 / (b.at_ms - a.at_ms) as f32;
                Some(Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t))
            }
            (Some(a), _) => Some(Point::new(a.x, a.y)),
            (None, Some(b)) => Some(Point::new(b.x, b.y)),
            (None, None) => None,
        }
    }

    /// The last click at or before this moment of the film, and how long ago it was.
    pub fn click_before(&self, film_ms: u64) -> Option<(Point, u64)> {
        let clicks = &self.film.header.clicks;
        let key = clicks.get(clicks.partition_point(|key| key.at_ms <= film_ms).checked_sub(1)?)?;
        Some((Point::new(key.x, key.y), film_ms - key.at_ms))
    }

    /// The whole film as the bytes of a file, with its edits.
    pub fn to_bytes(&self) -> Vec<u8> {
        let header = serde_json::to_vec(&self.film.header).unwrap_or_default();
        let project = serde_json::to_vec(&self.project).unwrap_or_default();
        let body = deflate(&header);
        let edits = deflate(&project);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        for part in [&body, &edits] {
            bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
            bytes.extend_from_slice(part);
        }
        for blob in &self.film.blobs {
            bytes.extend_from_slice(blob);
        }
        bytes
    }

    /// Reads a film back from the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MovieError> {
        let damaged = |why: &str| MovieError::Damaged(why.to_owned());
        let mut rest = bytes.strip_prefix(MAGIC).ok_or_else(|| damaged("it does not start like a film"))?;
        let mut part = || -> Result<Vec<u8>, MovieError> {
            let (length, after) =
                rest.split_first_chunk::<8>().ok_or_else(|| damaged("a part is missing"))?;
            let length =
                usize::try_from(u64::from_le_bytes(*length)).map_err(|_| damaged("a part is too long"))?;
            let data = after.get(..length).ok_or_else(|| damaged("a part was cut"))?;
            rest = &after[length..];
            inflate(data)
        };
        let header = part()?;
        let project = part()?;
        let header: Header =
            serde_json::from_slice(&header).map_err(|e| MovieError::Damaged(e.to_string()))?;
        let project: Project =
            serde_json::from_slice(&project).map_err(|e| MovieError::Damaged(e.to_string()))?;
        let mut blobs = Vec::with_capacity(header.blob_lengths.len());
        for length in &header.blob_lengths {
            let length = usize::try_from(*length).map_err(|_| damaged("a blob is too long"))?;
            let blob = rest.get(..length).ok_or_else(|| damaged("a blob was cut"))?;
            blobs.push(blob.to_vec());
            rest = &rest[length..];
        }
        if header.frames.is_empty() {
            return Err(MovieError::Empty);
        }
        let film = Film::open(header, blobs)?;
        Ok(Self::with(Rc::new(film), project))
    }
}

/// Chapters and hotspots, moved to where the edits put them, with the hotspots
/// added on the timeline.
fn edited_marks(header: &Header, project: &Project) -> (Vec<Chapter>, Vec<Hotspot>) {
    let mut chapters: Vec<Chapter> = header
        .chapters
        .iter()
        .filter_map(|chapter| {
            // If its first moments were cut, it starts where the film picks up again.
            let next = project.clips.iter().filter_map(|clip| match clip.kind {
                crate::project::ClipKind::Film { from_ms, .. } if from_ms >= chapter.at_ms => Some(from_ms),
                _ => None,
            });
            let at = project.film_to_timeline(chapter.at_ms).or_else(|| {
                let end = header
                    .chapters
                    .iter()
                    .map(|c| c.at_ms)
                    .find(|at| *at > chapter.at_ms)
                    .unwrap_or(u64::MAX);
                next.filter(|from| *from < end).min().and_then(|from| project.film_to_timeline(from))
            })?;
            Some(Chapter { at_ms: at, ..chapter.clone() })
        })
        .collect();
    chapters.extend(project.markers.iter().map(|marker| Chapter {
        title: marker.title.clone(),
        text: String::new(),
        at_ms: marker.at_ms,
    }));
    chapters.sort_by_key(|chapter| chapter.at_ms);
    let mut hotspots: Vec<Hotspot> = header
        .hotspots
        .iter()
        .filter_map(|spot| {
            let from = project.film_to_timeline(spot.from_ms)?;
            let length = spot.until_ms.saturating_sub(spot.from_ms);
            Some(Hotspot { from_ms: from, until_ms: from + length, ..spot.clone() })
        })
        .collect();
    for overlay in &project.overlays {
        if let OverlayKind::Hotspot { region, title, text, stop } = &overlay.kind {
            hotspots.push(Hotspot {
                kind: if *stop { HotspotKind::Stop } else { HotspotKind::Note },
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
                title: title.clone(),
                text: text.clone(),
                from_ms: overlay.start_ms,
                until_ms: overlay.end_ms(),
            });
        }
    }
    hotspots.sort_by_key(|spot| spot.from_ms);
    (chapters, hotspots)
}

/// Clicks and keys on the edited timeline: a part that was cut makes no sound.
fn edited_sounds(header: &Header, project: &Project) -> Vec<(u64, FilmSound)> {
    let clicks = header.clicks.iter().map(|click| (click.at_ms, FilmSound::Click));
    let keys = header.keys.iter().map(|at| (*at, FilmSound::Key));
    let mut sounds: Vec<(u64, FilmSound)> =
        clicks.chain(keys).filter_map(|(at, sound)| Some((project.film_to_timeline(at)?, sound))).collect();
    sounds.sort_by_key(|(at, _)| *at);
    sounds
}

impl Film {
    fn open(header: Header, blobs: Vec<Vec<u8>>) -> Result<Self, MovieError> {
        let damaged = |why: &str| MovieError::Damaged(why.to_owned());
        let mut fonts = Vec::with_capacity(header.fonts.len());
        for font in &header.fonts {
            fonts.push(match font {
                StoredFont::Builtin(number) => ravenrs_text::builtin_font(*number)
                    .ok_or_else(|| damaged("it asks for a font that is not here"))?,
                StoredFont::Embedded { blob, index } => {
                    let bytes =
                        inflate(blobs.get(*blob as usize).ok_or_else(|| damaged("a font is missing"))?)?;
                    FontRef { id: font_id(&bytes), data: Arc::new(bytes), index: *index }
                }
            });
        }
        let mut images = Vec::with_capacity(header.images.len());
        for image in &header.images {
            let pixels =
                inflate(blobs.get(image.blob as usize).ok_or_else(|| damaged("a picture is missing"))?)?;
            images.push(
                ImageData::from_rgba8(image.width, image.height, pixels)
                    .ok_or_else(|| damaged("a picture is the wrong size"))?,
            );
        }
        let primitives = header
            .shapes
            .iter()
            .map(|shape| shape.primitive(&fonts, &images))
            .collect::<Result<Vec<_>, _>>()?;
        let mut frames: Vec<(u64, Rc<[u32]>)> = Vec::with_capacity(header.frames.len());
        let mut list: Vec<u32> = Vec::new();
        for frame in &header.frames {
            let (head, tail) = (frame.head as usize, frame.tail as usize);
            if head + tail > list.len() {
                return Err(damaged("a frame does not fit the one before"));
            }
            let mut next = Vec::with_capacity(head + frame.mid.len() + tail);
            next.extend_from_slice(&list[..head]);
            next.extend_from_slice(&frame.mid);
            next.extend_from_slice(&list[list.len() - tail..]);
            if next.iter().any(|shape| *shape as usize >= primitives.len()) {
                return Err(damaged("a frame draws something that is not there"));
            }
            frames.push((frame.at, next.as_slice().into()));
            list = next;
        }
        Ok(Self { header, blobs, primitives, frames })
    }
}

fn font_id(bytes: &[u8]) -> u64 {
    // FNV-1a, with a high bit so that it does not meet the ids fonts get at run time.
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    hash | (1 << 63)
}

impl Stored {
    fn primitive(&self, fonts: &[FontRef], images: &[ImageData]) -> Result<Primitive, MovieError> {
        let damaged = |why: &str| MovieError::Damaged(why.to_owned());
        Ok(match self {
            Self::Q { r, c, f, b, s } => Primitive::Quad(Quad {
                rect: rect_of(*r),
                corner_radii: *c,
                fill: match f {
                    StoredBrush::S(color) => Brush::Solid(color_of(*color)),
                    StoredBrush::G(start, end, from, to) => Brush::LinearGradient(LinearGradient {
                        start: Point::new(start[0], start[1]),
                        end: Point::new(end[0], end[1]),
                        from: color_of(*from),
                        to: color_of(*to),
                    }),
                },
                border: b.map(|(width, color)| Border { width, color: color_of(color) }),
                shadow: s.map(|(offset, blur, color)| Shadow {
                    offset: Point::new(offset[0], offset[1]),
                    blur,
                    color: color_of(color),
                }),
            }),
            Self::T { f, s, c, y, g } => Primitive::Glyphs(GlyphRun {
                font: fonts.get(usize::from(*f)).ok_or_else(|| damaged("a font is missing"))?.clone(),
                size: *s,
                color: color_of(*c),
                glyphs: g.iter().map(|(id, x)| PositionedGlyph { id: *id, x: *x, y: *y }).collect(),
            }),
            Self::I { i, r, u, c, o } => {
                let image =
                    images.get(usize::from(*i)).ok_or_else(|| damaged("a picture is missing"))?.clone();
                Primitive::Image(ImageQuad {
                    rect: rect_of(*r),
                    image,
                    uv: *u,
                    corner_radii: *c,
                    opacity: *o,
                })
            }
            Self::C(r) => Primitive::PushClip(rect_of(*r)),
            Self::P => Primitive::PopClip,
        })
    }
}

fn deflate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    // Writing to a `Vec` does not fail.
    let _ = encoder.write_all(bytes);
    encoder.finish().unwrap_or_default()
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, MovieError> {
    let mut bytes = Vec::new();
    ZlibDecoder::new(data).read_to_end(&mut bytes).map_err(|e| MovieError::Damaged(e.to_string()))?;
    Ok(bytes)
}

fn shrink(image: &ImageData, rect: Rect) -> (u32, u32, Vec<u8>) {
    let (width, height) = (image.width(), image.height());
    let wanted = |size: f32, have: u32| ((size * IMAGE_DETAIL).ceil() as u32).clamp(1, have);
    let (new_width, new_height) = (wanted(rect.width(), width), wanted(rect.height(), height));
    if (new_width, new_height) == (width, height) {
        return (width, height, image.pixels().to_vec());
    }
    let mut pixels = Vec::with_capacity(new_width as usize * new_height as usize * 4);
    for y in 0..new_height {
        let (top, bottom) =
            (y * height / new_height, ((y + 1) * height / new_height).max(y * height / new_height + 1));
        for x in 0..new_width {
            let (left, right) =
                (x * width / new_width, ((x + 1) * width / new_width).max(x * width / new_width + 1));
            let mut sum = [0_u32; 4];
            for sy in top..bottom {
                for sx in left..right {
                    let at = (sy as usize * width as usize + sx as usize) * 4;
                    for (total, value) in sum.iter_mut().zip(&image.pixels()[at..at + 4]) {
                        *total += u32::from(*value);
                    }
                }
            }
            let count = (bottom - top) * (right - left);
            pixels.extend(sum.map(|total| (total / count) as u8));
        }
    }
    (new_width, new_height, pixels)
}

/// Puts a film together from what the app draws, keeping each different thing once.
#[derive(Clone, Debug)]
pub struct FilmBuilder {
    header: Header,
    blobs: Vec<Vec<u8>>,
    known: HashMap<String, u32>,
    known_fonts: HashMap<(u64, u32), u16>,
    known_images: HashMap<u64, u16>,
    list: Vec<u32>,
}

impl FilmBuilder {
    /// A film of an app of this logical size, over this color.
    pub fn new(logical: Size, background: Color) -> Self {
        Self {
            header: Header {
                logical: [logical.width, logical.height],
                background: tidy_color(background),
                duration_ms: 0,
                fonts: Vec::new(),
                images: Vec::new(),
                shapes: Vec::new(),
                frames: Vec::new(),
                pointer: Vec::new(),
                clicks: Vec::new(),
                keys: Vec::new(),
                chapters: Vec::new(),
                hotspots: Vec::new(),
                blob_lengths: Vec::new(),
            },
            blobs: Vec::new(),
            known: HashMap::new(),
            known_fonts: HashMap::new(),
            known_images: HashMap::new(),
            list: Vec::new(),
        }
    }

    fn add_blob(&mut self, bytes: &[u8]) -> u32 {
        let blob = deflate(bytes);
        self.header.blob_lengths.push(blob.len() as u64);
        self.blobs.push(blob);
        (self.blobs.len() - 1) as u32
    }

    fn font(&mut self, font: &FontRef) -> u16 {
        let key = (font.id, font.index);
        if let Some(known) = self.known_fonts.get(&key) {
            return *known;
        }
        let stored = match ravenrs_text::builtin_font_number(font.bytes()) {
            Some(number) => StoredFont::Builtin(number),
            None => StoredFont::Embedded { blob: self.add_blob(font.bytes()), index: font.index },
        };
        self.header.fonts.push(stored);
        let number = (self.header.fonts.len() - 1) as u16;
        self.known_fonts.insert(key, number);
        number
    }

    fn image(&mut self, quad: &ImageQuad) -> u16 {
        if let Some(known) = self.known_images.get(&quad.image.id()) {
            return *known;
        }
        let (width, height, pixels) = shrink(&quad.image, quad.rect);
        let blob = self.add_blob(&pixels);
        self.header.images.push(StoredImage { width, height, blob });
        let number = (self.header.images.len() - 1) as u16;
        self.known_images.insert(quad.image.id(), number);
        number
    }

    fn stored(&mut self, primitive: &Primitive) -> Option<Stored> {
        Some(match primitive {
            Primitive::Quad(quad) => Stored::Q {
                r: tidy_rect(quad.rect),
                c: quad.corner_radii.map(tidy),
                f: match quad.fill {
                    Brush::Solid(color) => StoredBrush::S(tidy_color(color)),
                    Brush::LinearGradient(g) => StoredBrush::G(
                        [g.start.x, g.start.y],
                        [g.end.x, g.end.y],
                        tidy_color(g.from),
                        tidy_color(g.to),
                    ),
                    _ => return None,
                },
                b: quad.border.map(|border| (tidy(border.width), tidy_color(border.color))),
                s: quad.shadow.map(|shadow| {
                    (
                        [tidy(shadow.offset.x), tidy(shadow.offset.y)],
                        tidy(shadow.blur),
                        tidy_color(shadow.color),
                    )
                }),
            },
            Primitive::Glyphs(run) => {
                let y = run.glyphs.first().map_or(0.0, |glyph| tidy(glyph.y));
                Stored::T {
                    f: self.font(&run.font),
                    s: tidy(run.size),
                    c: tidy_color(run.color),
                    y,
                    g: run.glyphs.iter().map(|glyph| (glyph.id, tidy(glyph.x))).collect(),
                }
            }
            Primitive::Image(quad) => Stored::I {
                i: self.image(quad),
                r: tidy_rect(quad.rect),
                u: quad.uv,
                c: quad.corner_radii.map(tidy),
                o: quad.opacity,
            },
            Primitive::PushClip(rect) => Stored::C(tidy_rect(*rect)),
            Primitive::PopClip => Stored::P,
            _ => return None,
        })
    }

    /// Adds what the app draws at `at_ms`. Nothing is added when it is the same
    /// as the last time: that picture just stays longer on screen.
    pub fn frame(&mut self, at_ms: u64, primitives: &[Primitive]) {
        self.header.duration_ms = self.header.duration_ms.max(at_ms);
        let mut list = Vec::with_capacity(primitives.len());
        for primitive in primitives {
            let Some(stored) = self.stored(primitive) else {
                continue;
            };
            // The text of a thing is its identity, floats included.
            let key = serde_json::to_string(&stored).unwrap_or_default();
            let next = self.header.shapes.len() as u32;
            let index = *self.known.entry(key).or_insert(next);
            if index == next {
                self.header.shapes.push(stored);
            }
            list.push(index);
        }
        if !self.header.frames.is_empty() && list == self.list {
            return;
        }
        let head = list.iter().zip(&self.list).take_while(|(a, b)| a == b).count();
        let tail =
            list[head..].iter().rev().zip(self.list[head..].iter().rev()).take_while(|(a, b)| a == b).count();
        let mid = list[head..list.len() - tail].to_vec();
        self.header.frames.push(StoredFrame { at: at_ms, head: head as u32, tail: tail as u32, mid });
        self.list = list;
    }

    /// Notes where the pointer is.
    pub fn pointer(&mut self, at_ms: u64, position: Point) {
        self.header.pointer.push(PointerKey { at_ms, x: tidy(position.x), y: tidy(position.y) });
    }

    /// Notes a click.
    pub fn click(&mut self, at_ms: u64, position: Point) {
        self.header.clicks.push(PointerKey { at_ms, x: tidy(position.x), y: tidy(position.y) });
    }

    /// Notes a key being pressed, for its sound.
    pub fn key(&mut self, at_ms: u64) {
        self.header.keys.push(at_ms);
    }

    /// Starts a chapter at `at_ms`.
    pub fn chapter(&mut self, title: impl Into<String>, text: impl Into<String>, at_ms: u64) {
        self.header.chapters.push(Chapter { title: title.into(), text: text.into(), at_ms });
    }

    /// Adds a spot on the screen.
    pub fn hotspot(&mut self, hotspot: Hotspot) {
        self.header.hotspots.push(hotspot);
    }

    /// The film, lasting until `duration_ms` even if the picture stopped changing before.
    pub fn finish(mut self, duration_ms: u64) -> Result<Movie, MovieError> {
        self.header.duration_ms = self.header.duration_ms.max(duration_ms);
        if self.header.frames.is_empty() {
            return Err(MovieError::Empty);
        }
        let project = Project::whole(self.header.duration_ms);
        let film = Film::open(self.header, self.blobs)?;
        Ok(Movie::with(Rc::new(film), project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{OverlayKind, Region};

    fn square(x: f32) -> Primitive {
        Primitive::Quad(Quad::new(Rect::new(x, 0.0, 10.0, 10.0), Color::BLACK).radius(2.0))
    }

    fn builder() -> FilmBuilder {
        FilmBuilder::new(Size::new(100.0, 50.0), Color::hex(0xffffff))
    }

    fn drawn(movie: &Movie, ms: u64) -> Vec<Primitive> {
        let (_, list) = movie.draw_list(ms).expect("a frame");
        list.iter().map(|i| movie.primitive(*i).expect("a shape").clone()).collect()
    }

    #[test]
    fn the_same_picture_is_not_stored_and_each_thing_is_stored_once() {
        let mut film = builder();
        film.frame(0, &[square(0.0), square(20.0)]);
        film.frame(100, &[square(0.0), square(20.0)]);
        film.frame(200, &[square(0.0), square(30.0)]);
        film.frame(300, &[square(0.0), square(20.0)]);
        let movie = film.finish(1000).expect("film");
        assert_eq!(movie.film.frames.len(), 3);
        assert_eq!(movie.film.header.shapes.len(), 3, "three different squares in all");
        assert_eq!(movie.duration_ms(), 1000);
        assert_eq!(drawn(&movie, 150), [square(0.0), square(20.0)]);
        assert_eq!(drawn(&movie, 250), [square(0.0), square(30.0)]);
    }

    #[test]
    fn a_film_survives_a_file_with_its_edits() {
        let mut film = builder();
        film.frame(0, &[square(0.0), square(5.0), square(20.0)]);
        film.frame(100, &[square(0.0), square(7.0), square(9.0), square(20.0)]);
        film.frame(200, &[square(20.0)]);
        film.pointer(0, Point::new(1.0, 2.0));
        film.click(100, Point::new(1.0, 2.0));
        film.chapter("One", "first", 0);
        let mut project = Project::whole(300);
        project.add_overlay(OverlayKind::Caption { text: "hello".to_owned() }, 0, 100);
        let movie = film.finish(300).expect("film").with_project(project.clone());
        let back = Movie::from_bytes(&movie.to_bytes()).expect("read back");
        assert_eq!(back.project(), &project);
        assert_eq!(back.chapters()[0].title, "One");
        for ms in [0, 100, 200] {
            assert_eq!(drawn(&back, ms), drawn(&movie, ms), "at {ms}");
        }
        assert_eq!(back.click_before(150), Some((Point::new(1.0, 2.0), 50)));
    }

    #[test]
    fn text_keeps_its_glyphs_and_builtin_fonts_are_not_copied() {
        let font = ravenrs_text::builtin_font(0).expect("a font");
        let run = |x: f32| {
            Primitive::Glyphs(GlyphRun {
                font: font.clone(),
                size: 14.0,
                color: Color::BLACK,
                glyphs: vec![
                    PositionedGlyph { id: 42, x, y: 20.0 },
                    PositionedGlyph { id: 43, x: x + 8.0, y: 20.0 },
                ],
            })
        };
        let mut film = builder();
        film.frame(0, &[run(3.0)]);
        let bytes = film.finish(0).expect("film").to_bytes();
        assert!(bytes.len() < 1024, "{} bytes", bytes.len());
        assert_eq!(drawn(&Movie::from_bytes(&bytes).expect("back"), 0), [run(3.0)]);
    }

    #[test]
    fn a_big_picture_shrinks_to_what_it_is_drawn_at_and_stored_once() {
        let image =
            ImageData::from_fn(256, 256, |x, _| if x < 128 { Color::hex(0) } else { Color::hex(0xffffff) })
                .expect("image");
        let quad = |x: f32| Primitive::Image(ImageQuad::new(Rect::new(x, 0.0, 16.0, 16.0), image.clone()));
        let mut film = builder();
        film.frame(0, &[quad(0.0)]);
        film.frame(10, &[quad(5.0)]);
        let movie = film.finish(20).expect("film");
        assert_eq!(movie.film.header.images.len(), 1);
        assert_eq!((movie.film.header.images[0].width, movie.film.header.images[0].height), (32, 32));
    }

    #[test]
    fn the_pointer_slides_between_where_it_was_noted() {
        let mut film = builder();
        film.frame(0, &[square(0.0)]);
        film.pointer(100, Point::new(0.0, 0.0));
        film.pointer(200, Point::new(10.0, 20.0));
        let movie = film.finish(300).expect("film");
        assert_eq!(movie.pointer_at(150), Some(Point::new(5.0, 10.0)));
        assert_eq!(movie.pointer_at(0), Some(Point::new(0.0, 0.0)));
        assert_eq!(movie.pointer_at(999), Some(Point::new(10.0, 20.0)));
    }

    #[test]
    fn marks_follow_the_edits() {
        let mut film = builder();
        film.frame(0, &[square(0.0)]);
        film.chapter("One", "", 0);
        film.chapter("Two", "", 5_000);
        film.chapter("Three", "", 8_000);
        let spot = |at: u64| Hotspot {
            kind: HotspotKind::Stop,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            title: format!("at {at}"),
            text: String::new(),
            from_ms: at,
            until_ms: at,
        };
        film.hotspot(spot(3_000));
        film.hotspot(spot(6_000));
        let movie = film.finish(10_000).expect("film");
        let mut project = movie.project().clone();
        project.split(2_000);
        project.split(5_500);
        project.delete_clip(1);
        project.add_overlay(
            OverlayKind::Hotspot {
                region: Region::new(1.0, 1.0, 2.0, 2.0),
                title: "added".to_owned(),
                text: String::new(),
                stop: false,
            },
            1_000,
            500,
        );
        let edited = movie.with_project(project);
        let chapters: Vec<(&str, u64)> =
            edited.chapters().iter().map(|c| (c.title.as_str(), c.at_ms)).collect();
        assert_eq!(chapters, [("One", 0), ("Two", 2_000), ("Three", 4_500)], "two begins where it picks up");
        let spots: Vec<(&str, u64)> =
            edited.hotspots().iter().map(|s| (s.title.as_str(), s.from_ms)).collect();
        assert_eq!(spots, [("added", 1_000), ("at 6000", 2_500)], "the one in the cut went");
    }

    #[test]
    fn something_that_is_not_a_film_says_so() {
        assert!(matches!(Movie::from_bytes(b"nope"), Err(MovieError::Damaged(_))));
        let mut film = builder();
        film.frame(0, &[square(0.0)]);
        let bytes = film.finish(0).expect("film").to_bytes();
        assert!(matches!(Movie::from_bytes(&bytes[..bytes.len() - 2]), Err(MovieError::Damaged(_))));
        assert!(matches!(builder().finish(0), Err(MovieError::Empty)));
    }
}
