//! Draws real Lottie files with our engine and compares them with reference pictures
//! from lottie-web, the player everything else is measured by. Needs both folders:
//!
//! ```text
//! LOTTIE_CORPUS=json LOTTIE_REFERENCE=refs LOTTIE_OUT=ours \
//!     cargo test --release -p ravenrs --test lottie_corpus -- --nocapture
//! ```
#![allow(clippy::expect_used, clippy::print_stdout, reason = "a measuring tool")]

use std::path::Path;
use std::time::Instant;

use ravenrs_core::{Color, PathFill, Rect, Scene};
use ravenrs_lottie::Animation;
use ravenrs_render::{GpuContext, Renderer, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SIDE: u32 = 512;
/// Where each file is compared, as a part of its length.
const MOMENTS: [f64; 3] = [0.25, 0.5, 0.75];
/// A channel this far off counts as a different pixel.
const DIFFERENT: i32 = 48;

fn draw(gpu: &GpuContext, renderer: &mut Renderer, scene: &Scene, w: u32, h: u32) -> image::RgbaImage {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("corpus"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    renderer.render(scene, &view, (w, h), 1.0);
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
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let data = slice.get_mapped_range().expect("range");
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    image::RgbaImage::from_raw(w, h, pixels).expect("image")
}

/// The part of the pixels that differ, compared only over what either of them drew.
fn difference(a: &image::RgbaImage, b: &image::RgbaImage) -> f32 {
    let (mut drawn, mut different) = (0u32, 0u32);
    for (p, q) in a.pixels().zip(b.pixels()) {
        let ink = |c: &image::Rgba<u8>| c.0[..3].iter().any(|v| *v < 250);
        if !ink(p) && !ink(q) {
            continue;
        }
        drawn += 1;
        if p.0[..3].iter().zip(&q.0[..3]).any(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() > DIFFERENT) {
            different += 1;
        }
    }
    if drawn == 0 { 0.0 } else { different as f32 / drawn as f32 }
}

#[test]
fn real_files_against_the_reference() {
    let (Some(corpus), Some(reference), Some(out)) = (
        std::env::var_os("LOTTIE_CORPUS"),
        std::env::var_os("LOTTIE_REFERENCE"),
        std::env::var_os("LOTTIE_OUT"),
    ) else {
        return;
    };
    let (corpus, reference, out) = (Path::new(&corpus), Path::new(&reference), Path::new(&out));
    std::fs::create_dir_all(out).expect("folder");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None)).expect("a GPU");
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut names: Vec<_> = std::fs::read_dir(corpus)
        .expect("corpus")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    names.sort();
    let (mut good, mut close, mut far, mut unread) = (0, 0, 0, 0);
    for path in names {
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = std::fs::read(&path).expect("file");
        let animation = match Animation::from_json(&bytes) {
            Ok(animation) => animation,
            Err(why) => {
                println!("NÃO LEU {name}: {why}");
                unread += 1;
                continue;
            }
        };
        let scale = (SIDE as f32 / animation.width().max(1.0)).min(SIDE as f32 / animation.height().max(1.0));
        let x = (SIDE as f32 - animation.width() * scale) / 2.0;
        let y = (SIDE as f32 - animation.height() * scale) / 2.0;
        let mut scene = Scene::new();
        scene.set_clear_color(Color::rgba(1.0, 1.0, 1.0, 1.0));
        let started = Instant::now();
        let mut built = 0;
        for (i, moment) in MOMENTS.iter().enumerate() {
            let shift = (i as u32 * SIDE) as f32;
            scene.push_clip(Rect::new(x + shift, y, animation.width() * scale, animation.height() * scale));
            for path in animation.paths_at(animation.duration() * moment, scale) {
                built += 1;
                let clip = path.clip.clone();
                scene.push_path(PathFill { transform: [scale, 0.0, 0.0, scale, x + shift, y], clip, ..path });
            }
            scene.pop_clip();
        }
        let build_ms = started.elapsed().as_secs_f32() * 1000.0 / MOMENTS.len() as f32;
        let ours = draw(&gpu, &mut renderer, &scene, SIDE * MOMENTS.len() as u32, SIDE);
        ours.save(out.join(format!("{name}.png"))).expect("save");
        let score = match image::open(reference.join(format!("{name}.png"))) {
            Ok(picture) => difference(&ours, &picture.into_rgba8()),
            Err(_) => {
                println!("SEM REFERÊNCIA {name}");
                continue;
            }
        };
        match score {
            s if s < 0.03 => good += 1,
            s if s < 0.12 => close += 1,
            _ => far += 1,
        }
        let missing = animation.unsupported().join(", ");
        println!("{:6.1}%  {build_ms:5.1} ms  {built:4} formas  {name}  [{missing}]", score * 100.0);
    }
    println!("fiéis: {good}  próximos: {close}  distantes: {far}  não leu: {unread}");
}

/// Draws one file at the given moments, in rows of six, to look at beside its reference:
///
/// ```text
/// LOTTIE_STRIP=logo.json LOTTIE_TIMES=120,300,480 LOTTIE_OUT=strip.png \
///     cargo test --release -p ravenrs --test lottie_corpus strip -- --nocapture
/// ```
#[test]
fn strip_of_one_file() {
    let (Some(file), Some(times), Some(out)) = (
        std::env::var_os("LOTTIE_STRIP"),
        std::env::var("LOTTIE_TIMES").ok(),
        std::env::var_os("LOTTIE_OUT"),
    ) else {
        return;
    };
    const CELL: u32 = 256;
    const PER_ROW: u32 = 6;
    let times: Vec<f64> = times.split(',').map(|t| t.parse().expect("milliseconds")).collect();
    let animation = Animation::from_json(&std::fs::read(file).expect("file")).expect("a Lottie");
    println!("sem suporte: {:?}", animation.unsupported());
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None)).expect("a GPU");
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let scale = CELL as f32 / animation.width().max(animation.height()).max(1.0);
    let mut scene = Scene::new();
    scene.set_clear_color(Color::rgba(0.906, 0.914, 0.945, 1.0));
    for (i, at) in times.iter().enumerate() {
        let (x, y) = (((i as u32 % PER_ROW) * CELL) as f32, ((i as u32 / PER_ROW) * CELL) as f32);
        scene.push_clip(Rect::new(x, y, CELL as f32, CELL as f32));
        for path in animation.paths_at(at / 1000.0, scale) {
            let clip = path.clip.clone();
            scene.push_path(PathFill { transform: [scale, 0.0, 0.0, scale, x, y], clip, ..path });
        }
        scene.pop_clip();
    }
    let rows = (times.len() as u32).div_ceil(PER_ROW);
    draw(&gpu, &mut renderer, &scene, CELL * PER_ROW, CELL * rows).save(out).expect("save");
}
