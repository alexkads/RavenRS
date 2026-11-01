//! GPU textures for scene images, cached by image id and evicted when unused.

use std::collections::HashMap;

use ravenrs_core::ImageData;

/// Frames an image may go unused before its texture is released.
const EVICT_AFTER_FRAMES: u64 = 240;

pub(crate) struct GpuImage {
    pub bind_group: wgpu::BindGroup,
    last_used: u64,
}

#[derive(Default)]
pub(crate) struct ImageCache {
    entries: HashMap<u64, GpuImage>,
    frame: u64,
}

impl ImageCache {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        self.entries.retain(|_, e| frame - e.last_used <= EVICT_AFTER_FRAMES);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: u64) -> Option<&GpuImage> {
        self.entries.get(&id)
    }

    /// Makes sure `image` is on the GPU, uploading it with a full mip chain once.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        image: &ImageData,
    ) {
        let frame = self.frame;
        if let Some(entry) = self.entries.get_mut(&image.id()) {
            entry.last_used = frame;
            return;
        }
        let limit = device.limits().max_texture_dimension_2d;
        let mut levels = mip_chain(image.width(), image.height(), image.pixels());
        // Images larger than the device allows start at the first mip that fits.
        while levels.len() > 1 && (levels[0].0 > limit || levels[0].1 > limit) {
            levels.remove(0);
        }
        let (width, height, _) = &levels[0];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("raven image"),
            size: wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, (w, h, pixels)) in levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(*h),
                },
                wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raven image"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        self.entries.insert(image.id(), GpuImage { bind_group, last_used: frame });
    }
}

/// The image followed by successively halved levels (2x2 box filter, alpha
/// weighted so transparent pixels do not darken edges), down to 1x1.
fn mip_chain(width: u32, height: u32, pixels: &[u8]) -> Vec<(u32, u32, Vec<u8>)> {
    let mut levels = vec![(width, height, pixels.to_vec())];
    while let Some((w, h, src)) = levels.last() {
        if *w == 1 && *h == 1 {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut dst = vec![0u8; nw as usize * nh as usize * 4];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(w - 1);
                    let sy = (y * 2 + dy).min(h - 1);
                    let i = ((sy * w + sx) * 4) as usize;
                    let a = f32::from(src[i + 3]);
                    for c in 0..3 {
                        acc[c] += f32::from(src[i + c]) * a;
                    }
                    acc[3] += a;
                }
                let o = ((y * nw + x) * 4) as usize;
                for c in 0..3 {
                    dst[o + c] = if acc[3] > 0.0 { (acc[c] / acc[3]).round() as u8 } else { 0 };
                }
                dst[o + 3] = (acc[3] / 4.0).round() as u8;
            }
        }
        levels.push((nw, nh, dst));
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_chain_halves_to_one_pixel_and_ignores_transparent_color() {
        // A white pixel next to a transparent black one averages to half-opaque white.
        let pixels = [255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0];
        let levels = mip_chain(2, 2, &pixels);
        assert_eq!(levels.len(), 2);
        assert_eq!((levels[1].0, levels[1].1), (1, 1));
        assert_eq!(levels[1].2, vec![255, 255, 255, 128]);
        assert_eq!(
            mip_chain(5, 3, &[0; 60]).iter().map(|l| (l.0, l.1)).collect::<Vec<_>>(),
            vec![(5, 3), (2, 1), (1, 1)]
        );
    }
}
