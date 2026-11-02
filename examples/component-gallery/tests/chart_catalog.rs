//! Draws the chart catalog to files, to compare it with the patterns it follows:
//!
//! GALLERY_CHARTS=target/charts cargo test -p component-gallery --test chart_catalog
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::path::Path;

use component_gallery::charts::{FAMILIES, family};
use ravenrs::core::Event;
use ravenrs::layout::{Axis, LayoutStyle};
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, Renderer, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const WIDTH: f32 = 1500.0;

fn capture(gpu: &GpuContext, ui: &mut Ui, size: Size, file: &Path) {
    let scale = 1.0;
    ui.set_viewport(size, scale);
    ui.set_time(0.0);
    ui.update();
    ui.set_time(5.0);
    ui.update();
    ui.update();
    capture_now(gpu, ui, size, file);
}

fn capture_now(gpu: &GpuContext, ui: &Ui, size: Size, file: &Path) {
    let scale = 1.0;
    let (w, h) = (size.width as u32, size.height as u32);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("catalog"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    renderer.render(ui.scene(), &target, (w, h), scale);
    let padded = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let data = slice.get_mapped_range().expect("mapped range");
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in data.chunks(padded as usize).take(h as usize) {
        pixels.extend_from_slice(&row[..(w * 4) as usize]);
    }
    let image = image::RgbaImage::from_raw(w, h, pixels).expect("image");
    image.save(file).expect("save");
}

struct Page {
    index: usize,
}

impl View for Page {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = VStack::new((Grid::new(3, family(self.index)),)).align(Align::Stretch).padding(24.0);
        let inner = cx.child(&content);
        let style =
            LayoutStyle { width: Some(WIDTH), align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) };
        let fill = cx.theme().color(Semantic::Canvas);
        cx.container(style, &[inner], Paint::Quad(QuadPaint::new(fill)))
    }
}

#[test]
fn draw_the_catalog() {
    let Some(folder) = std::env::var_os("GALLERY_CHARTS") else { return };
    let folder = Path::new(&folder);
    std::fs::create_dir_all(folder).expect("folder");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else { return };
    for (theme, name) in [(Theme::raven_light(), "claro"), (Theme::raven_dark(), "escuro")] {
        for (index, family_name) in FAMILIES.iter().enumerate() {
            let rows = family(index).len().div_ceil(3) as f32;
            let size = Size::new(WIDTH, rows * 460.0 + 48.0);
            let mut ui = Ui::new(Page { index }, theme);
            capture(&gpu, &mut ui, size, &folder.join(format!("{family_name}-{name}.png")));
            // The pointer over the second category of the first chart, as in the catalog.
            ui.handle_event(&Event::PointerMove { position: Point::new(190.0, 220.0) });
            ui.update();
            ui.set_time(6.0);
            ui.update();
            capture_now(&gpu, &ui, size, &folder.join(format!("{family_name}-{name}-apontado.png")));
        }
    }
}
