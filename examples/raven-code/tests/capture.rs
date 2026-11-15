//! A picture of Raven Code on the sample project, drawn on the GPU the way the visual tests of
//! `ravenrs` are. It writes nothing unless asked: `RAVENRS_CAPTURE_DIR=<folder> cargo test -p
//! raven-code --test capture`. A machine without a GPU adapter skips it.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use std::path::PathBuf;

use raven_code::RavenCode;
use raven_code::project::{Project, write_sample};
use ravenrs::core::{Event, Key, Modifiers};
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, Renderer, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[test]
fn raven_code_as_a_picture() {
    let Some(dir) = std::env::var_os("RAVENRS_CAPTURE_DIR").map(PathBuf::from) else { return };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let root = std::env::temp_dir().join("raven-code-capture");
    let _ = std::fs::remove_dir_all(&root);
    write_sample(&root).unwrap();
    let app = RavenCode::new(Project::open(&root).unwrap());
    app.open_file("src/greeting.rs");
    app.open_file("src/main.rs");
    app.edit(
        "src/main.rs",
        "mod greeting;\n\nfn main() {\n    printn!(\"{}\", greeting::hello(\"Raven Code\"));\n}\n",
    );
    app.take_output(
        &[
            r#"{"reason":"compiler-message","message":{"rendered":"error: cannot find macro `printn` in this scope\n --> src/main.rs:4:5\n  |\n4 |     printn!(\"{}\", greeting::hello(\"Raven Code\"));\n  |     ^^^^^^ help: a macro with a similar name exists: `println`\n","level":"error","message":"cannot find macro `printn` in this scope","spans":[{"file_name":"src/main.rs","line_start":4,"line_end":4,"column_start":5,"column_end":11,"is_primary":true}]}}"#,
            r#"{"reason":"build-finished","success":false}"#,
        ],
        false,
    );
    let finder = app.clone();
    let mut ui = Ui::new(app, Theme::raven_dark());
    let (w, h) = (1440_u32, 900_u32);
    ui.set_viewport(Size::new(w as f32, h as f32), 1.0);
    for _ in 0..6 {
        ui.update();
    }
    draw(&gpu, &ui, (w, h), &dir.join("raven-code.png"));

    // The same project searched: the Search panel lists the hits, and the find bar of the
    // editor marks the matches in the file.
    let ctrl = |key: Key, shift: bool| Event::KeyDown {
        key,
        modifiers: Modifiers { ctrl: true, shift, ..Modifiers::default() },
    };
    finder.search("hello");
    ui.handle_event(&ctrl(Key::Character("f".into()), true));
    finder.go_to_place("src/main.rs", 4, 20);
    for _ in 0..4 {
        ui.update();
    }
    ui.handle_event(&ctrl(Key::Character("f".into()), false));
    ui.update();
    for c in "greeting".chars() {
        ui.handle_event(&Event::KeyDown { key: Key::Character(c.to_string()), modifiers: Modifiers::default() });
        ui.update();
    }
    ui.handle_event(&Event::KeyDown { key: Key::Enter, modifiers: Modifiers::default() });
    for _ in 0..6 {
        ui.update();
    }
    draw(&gpu, &ui, (w, h), &dir.join("raven-code-search.png"));
}

/// Draws what `ui` shows to a PNG at `file`.
fn draw(gpu: &GpuContext, ui: &Ui, (w, h): (u32, u32), file: &std::path::Path) {
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
    image::RgbaImage::from_raw(w, h, pixels).unwrap().save(file).unwrap();
}
