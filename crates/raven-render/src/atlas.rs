//! Glyph atlas: an R8 coverage texture packed with simple shelves.

use std::collections::HashMap;

use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

/// Subpixel positions per pixel along x. Four keeps text crisp with few entries.
pub(crate) const SUBPIXEL_BINS: f32 = 4.0;
const PADDING: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GlyphKey {
    pub font_id: u64,
    pub font_index: u32,
    pub glyph: u32,
    /// Size in device pixels times 4.
    pub size_q: u32,
    pub subpixel: u8,
}

/// Where a rasterized glyph lives and how to place it relative to the pen.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AtlasGlyph {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

#[derive(Debug)]
struct Shelf {
    y: u32,
    height: u32,
    cursor: u32,
}

pub(crate) enum Lookup {
    Hit(Option<AtlasGlyph>),
    /// The atlas has no room; the caller should grow it and retry.
    Full,
}

pub(crate) struct GlyphAtlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: u32,
    shelves: Vec<Shelf>,
    /// `None` marks glyphs with no pixels (spaces), so they are not re-rasterized.
    cache: HashMap<GlyphKey, Option<AtlasGlyph>>,
    scaler: ScaleContext,
}

impl GlyphAtlas {
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("raven glyph atlas"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view, size, shelves: Vec::new(), cache: HashMap::new(), scaler: ScaleContext::new() }
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns the cached glyph, rasterizing and uploading it on a miss.
    pub fn get_or_insert(&mut self, queue: &wgpu::Queue, key: GlyphKey, font_bytes: &[u8]) -> Lookup {
        if let Some(entry) = self.cache.get(&key) {
            return Lookup::Hit(*entry);
        }
        let Some(image) = self.rasterize(key, font_bytes) else {
            self.cache.insert(key, None);
            return Lookup::Hit(None);
        };
        let (w, h) = (image.placement.width, image.placement.height);
        if w == 0 || h == 0 {
            self.cache.insert(key, None);
            return Lookup::Hit(None);
        }
        let Some((x, y)) = self.allocate(w, h) else {
            return Lookup::Full;
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &image.data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let glyph =
            AtlasGlyph { x, y, width: w, height: h, left: image.placement.left, top: image.placement.top };
        self.cache.insert(key, Some(glyph));
        Lookup::Hit(Some(glyph))
    }

    fn rasterize(&mut self, key: GlyphKey, font_bytes: &[u8]) -> Option<swash::scale::image::Image> {
        let font = swash::FontRef::from_index(font_bytes, key.font_index as usize)?;
        let glyph_id = u16::try_from(key.glyph).ok()?;
        let mut scaler = self.scaler.builder(font).size(key.size_q as f32 / 4.0).hint(false).build();
        Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(f32::from(key.subpixel) / SUBPIXEL_BINS, 0.0))
            .render(&mut scaler, glyph_id)
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let (pw, ph) = (w + PADDING, h + PADDING);
        if pw > self.size || ph > self.size {
            return None;
        }
        // Best fit among shelves tall enough (but not wastefully tall).
        let size = self.size;
        let best = self
            .shelves
            .iter_mut()
            .filter(|s| s.height >= ph && s.height <= ph + ph / 2 + 2 && s.cursor + pw <= size)
            .min_by_key(|s| s.height - ph);
        if let Some(shelf) = best {
            let x = shelf.cursor;
            shelf.cursor += pw;
            return Some((x, shelf.y));
        }
        let y = self.shelves.last().map_or(0, |s| s.y + s.height);
        if y + ph > self.size {
            return None;
        }
        self.shelves.push(Shelf { y, height: ph, cursor: pw });
        Some((0, y))
    }
}
