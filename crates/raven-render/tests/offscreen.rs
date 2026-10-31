//! Renders scenes into an offscreen texture and checks real pixels. Reading back
//! from the GPU happens only here, never in the renderer itself.
//!
//! Skipped (with a message) on machines without any GPU adapter.

#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use raven_core::{Color, LinearGradient, Point, Quad, Rect, Scene, Shadow};
use raven_render::{GpuContext, Renderer, wgpu};
use raven_text::{TextAttrs, TextSystem};

const W: u32 = 200;
const H: u32 = 100;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn gpu() -> Option<GpuContext> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    match pollster::block_on(GpuContext::new(&instance, None)) {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

/// Renders at `scale` and returns RGBA8 pixels of a `W*scale x H*scale` image.
fn render(gpu: &GpuContext, renderer: &mut Renderer, scene: &Scene, scale: f32) -> (u32, u32, Vec<u8>) {
    let (w, h) = ((W as f32 * scale) as u32, (H as f32 * scale) as u32);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    renderer.render(scene, &view, (w, h), scale);

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
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback buffer"));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll device");
    let data = slice.get_mapped_range().expect("mapped range");
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    (w, h, pixels)
}

fn pixel(img: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.0 + x) * 4) as usize;
    [img.2[i], img.2[i + 1], img.2[i + 2], img.2[i + 3]]
}

fn close(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

const BG: Color = Color::hex(0x0b0d12);

#[test]
fn rounded_quad_border_and_clear() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut scene = Scene::new();
    scene.set_clear_color(BG);
    scene.push_quad(
        Quad::new(Rect::new(20.0, 20.0, 100.0, 60.0), Color::hex(0xff0000))
            .radius(20.0)
            .border(4.0, Color::hex(0x00ff00)),
    );

    for scale in [1.0, 2.0] {
        let img = render(&gpu, &mut renderer, &scene, scale);
        let s = |v: f32| (v * scale) as u32;
        assert!(close(pixel(&img, s(70.0), s(50.0)), [255, 0, 0, 255], 2), "fill at center, scale {scale}");
        assert!(
            close(pixel(&img, s(70.0), s(21.0)), [0, 255, 0, 255], 2),
            "border on top edge, scale {scale}"
        );
        assert!(
            close(pixel(&img, s(21.0), s(21.0)), [11, 13, 18, 255], 2),
            "rounded corner is background, scale {scale}"
        );
        assert!(close(pixel(&img, s(180.0), s(90.0)), [11, 13, 18, 255], 1), "clear color, scale {scale}");
    }
}

#[test]
fn gradient_runs_top_to_bottom() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut scene = Scene::new();
    scene.push_quad(Quad::new(
        Rect::new(0.0, 0.0, 200.0, 100.0),
        LinearGradient::vertical(Color::hex(0x000000), Color::hex(0xffffff)),
    ));
    let img = render(&gpu, &mut renderer, &scene, 1.0);
    let top = pixel(&img, 100, 2)[0];
    let mid = pixel(&img, 100, 50)[0];
    let bottom = pixel(&img, 100, 97)[0];
    assert!(top < 15 && bottom > 240, "ends: {top} {bottom}");
    assert!((110..=145).contains(&mid), "middle is mid-grey: {mid}");
}

#[test]
fn shadow_darkens_below_the_shape_and_fades() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut scene = Scene::new();
    scene.set_clear_color(Color::WHITE);
    scene.push_quad(Quad::new(Rect::new(50.0, 20.0, 100.0, 40.0), Color::WHITE).radius(8.0).shadow(Shadow {
        offset: Point::new(0.0, 8.0),
        blur: 16.0,
        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
    }));
    let img = render(&gpu, &mut renderer, &scene, 1.0);
    let near = pixel(&img, 100, 64)[0];
    let far = pixel(&img, 100, 95)[0];
    assert!(near < 235, "shadow visible just below the card: {near}");
    assert!(far > near, "shadow fades with distance: {far} vs {near}");
    assert_eq!(pixel(&img, 100, 40), [255, 255, 255, 255], "card itself is opaque white");
}

#[test]
fn text_draws_coverage_inside_its_bounds() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut text = TextSystem::new();
    let mut layout = text
        .layout("Raven", &TextAttrs { size: 32.0, line_height: 38.0, weight: 600.0, ..Default::default() });
    let size = layout.measure(None);
    let mut scene = Scene::new();
    scene.set_clear_color(Color::BLACK);
    layout.paint(Point::new(10.0, 10.0), Color::WHITE, &mut scene);

    let img = render(&gpu, &mut renderer, &scene, 2.0);
    let (mut lit_inside, mut lit_outside) = (0, 0);
    for y in 0..img.1 {
        for x in 0..img.0 {
            if pixel(&img, x, y)[0] > 128 {
                let (lx, ly) = (x as f32 / 2.0, y as f32 / 2.0);
                let inside = lx >= 9.0 && lx <= 11.0 + size.width && ly >= 9.0 && ly <= 11.0 + size.height;
                if inside { lit_inside += 1 } else { lit_outside += 1 }
            }
        }
    }
    assert!(lit_inside > 500, "text produced too few lit pixels: {lit_inside}");
    assert_eq!(lit_outside, 0, "text drew outside its layout box");

    // A second frame reuses the cached glyphs.
    let before = renderer.render(
        &scene,
        &gpu.device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: 4, height: 4, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default()),
        (4, 4),
        2.0,
    );
    assert_eq!(before.glyphs, 5);
    assert_eq!(before.draw_calls, 1);
}
