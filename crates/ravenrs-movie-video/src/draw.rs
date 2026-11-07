//! Draws a view into pixels, with no window.

use ravenrs_core::Size;
use ravenrs_render::{GpuContext, Renderer, wgpu};
use ravenrs_style::Theme;
use ravenrs_widgets::{Ui, View};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A GPU, a renderer and a texture of the size of the video.
pub(crate) struct Canvas {
    gpu: GpuContext,
    renderer: Renderer,
    texture: wgpu::Texture,
    buffer: wgpu::Buffer,
    padded: u32,
    size: (u32, u32),
}

impl Canvas {
    pub(crate) fn new(size: (u32, u32)) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let gpu = pollster::block_on(GpuContext::new(&instance, None)).ok()?;
        let renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let padded =
            (size.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video readback"),
            size: u64::from(padded * size.1),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Some(Self { gpu, renderer, texture, buffer, padded, size })
    }

    /// What `ui` draws now, as rows of RGBA.
    pub(crate) fn draw(&mut self, ui: &Ui, scale: f32) -> Vec<u8> {
        let (w, h) = self.size;
        let view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer.render(ui.scene(), &view, (w, h), scale);
        let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit(Some(encoder.finish()));
        let slice = self.buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        if let Ok(data) = slice.get_mapped_range() {
            for row in 0..h {
                let start = (row * self.padded) as usize;
                pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
            }
        }
        self.buffer.unmap();
        pixels
    }
}

/// A Ui for `root` on a screen of `logical` size.
pub(crate) fn ui_for(root: impl View + 'static, theme: Theme, logical: Size, scale: f32) -> Ui {
    let mut ui = Ui::new(root, theme);
    ui.set_viewport(logical, scale);
    ui.set_time(0.0);
    ui.update();
    ui
}
