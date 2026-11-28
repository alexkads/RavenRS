//! Pictures of the three workbenches, drawn on the GPU the way the visual tests of `ravenrs` are.
//! It writes nothing unless asked: `RAVENRS_CAPTURE_DIR=<folder> cargo test --manifest-path examples/dock-workbench/Cargo.toml
//! --test capture`. A machine without a GPU adapter skips it.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::path::PathBuf;

use dock_workbench::Workbench;
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, Renderer, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn render(gpu: &GpuContext, view: AnyView, theme: Theme, size: Size) -> image::RgbaImage {
    let mut ui = Ui::new(view, theme);
    ui.set_viewport(size, 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let (w, h) = (size.width as u32, size.height as u32);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("capture"),
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
    renderer.render(ui.scene(), &target, (w, h), 1.0);
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
    gpu.queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let data = slice.get_mapped_range().expect("mapped range");
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    image::RgbaImage::from_raw(w, h, pixels).expect("an image")
}

#[test]
fn the_three_workbenches_as_pictures() {
    let Some(dir) = std::env::var_os("RAVENRS_CAPTURE_DIR").map(PathBuf::from) else { return };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = match pollster::block_on(GpuContext::new(&instance, None)) {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("no GPU, nothing drawn: {e}");
            return;
        }
    };
    std::fs::create_dir_all(&dir).expect("the folder");
    for which in Workbench::ALL {
        let picture = render(&gpu, which.app(), Theme::raven_dark(), Size::new(1440.0, 900.0));
        picture.save(dir.join(format!("{}.png", which.word()))).expect("saved");
    }
}
