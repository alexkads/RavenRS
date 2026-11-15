//! Renders scenes into an offscreen texture and checks real pixels. Reading back
//! from the GPU happens only here, never in the renderer itself.
//!
//! Skipped (with a message) on machines without any GPU adapter.

#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::sync::Arc;

use ravenrs_core::{
    Color, ColorStop, FillRule, ImageData, LinearGradient, PathElement, PathFill, PathPaint, Point, Quad,
    Rect, Scene, Shadow,
};
use ravenrs_render::{GpuContext, Renderer, wgpu};
use ravenrs_text::{TextAttrs, TextSystem};

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
    render_counted(gpu, renderer, scene, scale).0
}

/// The pixels, and what the renderer counted drawing them.
fn render_counted(
    gpu: &GpuContext,
    renderer: &mut Renderer,
    scene: &Scene,
    scale: f32,
) -> ((u32, u32, Vec<u8>), ravenrs_render::RenderStats) {
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
    let stats = renderer.render(scene, &view, (w, h), scale);

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
    ((w, h, pixels), stats)
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

#[test]
fn clip_rect_limits_drawing_and_nests() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut scene = Scene::new();
    scene.set_clear_color(Color::BLACK);
    scene.push_clip(Rect::new(0.0, 0.0, 100.0, 100.0));
    scene.push_clip(Rect::new(50.0, 0.0, 100.0, 50.0));
    scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 200.0, 100.0), Color::WHITE));
    scene.pop_clip();
    scene.pop_clip();
    scene.push_quad(Quad::new(Rect::new(180.0, 80.0, 20.0, 20.0), Color::hex(0x00ff00)));
    let img = render(&gpu, &mut renderer, &scene, 2.0);
    assert_eq!(pixel(&img, 2 * 75, 2 * 25), [255, 255, 255, 255], "inside both clips");
    assert_eq!(pixel(&img, 2 * 25, 2 * 25), [0, 0, 0, 255], "outside the inner clip");
    assert_eq!(pixel(&img, 2 * 120, 2 * 25), [0, 0, 0, 255], "outside the outer clip");
    assert_eq!(pixel(&img, 2 * 190, 2 * 90), [0, 255, 0, 255], "clip popped");
}

#[test]
fn images_draw_with_rounded_corners_uv_crop_and_mipmaps() {
    use ravenrs_core::{ImageData, ImageQuad};
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    // Left half red, right half blue.
    let halves =
        ImageData::from_fn(64, 64, |x, _| if x < 32 { Color::hex(0xff0000) } else { Color::hex(0x0000ff) })
            .expect("image");
    // 1px checkerboard: averages to mid grey when minified.
    let checker =
        ImageData::from_fn(256, 256, |x, y| if (x + y) % 2 == 0 { Color::WHITE } else { Color::BLACK })
            .expect("image");
    let mut scene = Scene::new();
    scene.set_clear_color(Color::hex(0x00ff00));
    scene.push_image(ImageQuad::new(Rect::new(10.0, 10.0, 80.0, 80.0), halves.clone()).radius(20.0));
    let mut crop = ImageQuad::new(Rect::new(100.0, 10.0, 40.0, 40.0), halves);
    crop.uv = [0.5, 0.0, 1.0, 1.0];
    scene.push_image(crop);
    scene.push_image(ImageQuad::new(Rect::new(150.0, 10.0, 32.0, 32.0), checker));

    let img = render(&gpu, &mut renderer, &scene, 1.0);
    assert!(close(pixel(&img, 25, 50), [255, 0, 0, 255], 3), "left half red");
    assert!(close(pixel(&img, 75, 50), [0, 0, 255, 255], 3), "right half blue");
    assert!(close(pixel(&img, 11, 11), [0, 255, 0, 255], 3), "rounded corner shows the background");
    assert!(close(pixel(&img, 110, 30), [0, 0, 255, 255], 3), "uv crop shows only the blue half");
    let grey = pixel(&img, 166, 26);
    assert!(
        (100..=156).contains(&grey[0]) && grey[0].abs_diff(grey[1]) < 4,
        "minified checkerboard is grey: {grey:?}"
    );
    let stats = renderer.render(
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
        1.0,
    );
    assert_eq!((stats.images, stats.cached_images), (3, 2), "textures are reused across frames");
}

fn box_path(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<PathElement> {
    let p = Point::new;
    vec![
        PathElement::MoveTo(p(x0, y0)),
        PathElement::LineTo(p(x1, y0)),
        PathElement::LineTo(p(x1, y1)),
        PathElement::LineTo(p(x0, y1)),
        PathElement::Close,
    ]
}

/// A circle of four cubic curves.
fn circle_path(cx: f32, cy: f32, r: f32) -> Vec<PathElement> {
    let k = r * 0.552_284_8;
    let p = Point::new;
    vec![
        PathElement::MoveTo(p(cx + r, cy)),
        PathElement::CubicTo(p(cx + r, cy + k), p(cx + k, cy + r), p(cx, cy + r)),
        PathElement::CubicTo(p(cx - k, cy + r), p(cx - r, cy + k), p(cx - r, cy)),
        PathElement::CubicTo(p(cx - r, cy - k), p(cx - k, cy - r), p(cx, cy - r)),
        PathElement::CubicTo(p(cx + k, cy - r), p(cx + r, cy - k), p(cx + r, cy)),
        PathElement::Close,
    ]
}

fn draw_paths(paths: Vec<PathFill>) -> Option<(u32, u32, Vec<u8>)> {
    let gpu = gpu()?;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut scene = Scene::new();
    scene.set_clear_color(Color::rgba(0.0, 0.0, 0.0, 1.0));
    for path in paths {
        scene.push_path(path);
    }
    Some(render(&gpu, &mut renderer, &scene, 1.0))
}

#[test]
fn a_path_edge_covers_exactly_the_part_of_the_pixel_it_takes() {
    let white = Color::rgba(1.0, 1.0, 1.0, 1.0);
    // The left edge halfway through a pixel, the top a quarter of the way into one.
    let Some(img) = draw_paths(vec![PathFill::solid(box_path(10.5, 10.25, 30.0, 30.0), white)]) else {
        return;
    };
    assert_eq!(pixel(&img, 9, 20)[0], 0, "outside");
    assert!(close(pixel(&img, 10, 20), [128, 128, 128, 255], 2), "half: {:?}", pixel(&img, 10, 20));
    assert!(close(pixel(&img, 20, 10), [191, 191, 191, 255], 2), "three quarters: {:?}", pixel(&img, 20, 10));
    assert!(close(pixel(&img, 10, 10), [96, 96, 96, 255], 2), "their product: {:?}", pixel(&img, 10, 10));
    assert_eq!(pixel(&img, 20, 20)[0], 255, "inside");
    assert_eq!(pixel(&img, 30, 20)[0], 0, "and nothing past the right edge");
}

#[test]
fn a_slanted_edge_gets_its_true_area_too() {
    let white = Color::rgba(1.0, 1.0, 1.0, 1.0);
    let p = Point::new;
    // A diagonal through the corners of each pixel leaves half of each.
    let triangle = vec![
        PathElement::MoveTo(p(10.0, 10.0)),
        PathElement::LineTo(p(50.0, 50.0)),
        PathElement::LineTo(p(10.0, 50.0)),
        PathElement::Close,
    ];
    let Some(img) = draw_paths(vec![PathFill::solid(triangle, white)]) else { return };
    assert!(close(pixel(&img, 30, 30), [128, 128, 128, 255], 2), "{:?}", pixel(&img, 30, 30));
    assert_eq!(pixel(&img, 20, 40)[0], 255);
    assert_eq!(pixel(&img, 40, 20)[0], 0);
    // A shallow one: at this column the edge is a third of the way down the row, and
    // the shape is what is under it.
    let shallow = vec![
        PathElement::MoveTo(p(10.0, 20.0)),
        PathElement::LineTo(p(70.0, 21.0)),
        PathElement::LineTo(p(70.0, 40.0)),
        PathElement::LineTo(p(10.0, 40.0)),
        PathElement::Close,
    ];
    let Some(img) = draw_paths(vec![PathFill::solid(shallow, white)]) else { return };
    let along = f32::from(pixel(&img, 30, 20)[0]) / 255.0;
    assert!((along - (1.0 - 20.5 / 60.0)).abs() < 0.02, "{along}");
}

#[test]
fn a_curved_path_keeps_the_area_of_a_circle() {
    let white = Color::rgba(1.0, 1.0, 1.0, 1.0);
    let Some(img) = draw_paths(vec![PathFill::solid(circle_path(50.0, 50.0, 30.0), white)]) else { return };
    let total: f32 = img.2.as_chunks::<4>().0.iter().map(|px| f32::from(px[0]) / 255.0).sum();
    let circle = std::f32::consts::PI * 30.0 * 30.0;
    assert!((total - circle).abs() < circle * 0.002, "{total} of {circle}");
    let partial = img.2.as_chunks::<4>().0.iter().filter(|px| px[0] > 0 && px[0] < 255).count();
    assert!(partial > 100, "soft edges: {partial}");
}

#[test]
fn the_fill_rules_treat_a_hole_differently() {
    let white = Color::rgba(1.0, 1.0, 1.0, 1.0);
    let mut both = box_path(10.0, 10.0, 60.0, 60.0);
    both.extend(box_path(25.0, 25.0, 45.0, 45.0));
    let nonzero = PathFill::solid(both.clone(), white);
    let Some(img) = draw_paths(vec![nonzero]) else { return };
    assert_eq!(pixel(&img, 35, 35)[0], 255, "wound twice the same way, still inside");
    let even_odd = PathFill { rule: FillRule::EvenOdd, ..PathFill::solid(both, white) };
    let Some(img) = draw_paths(vec![even_odd]) else { return };
    assert_eq!(pixel(&img, 35, 35)[0], 0, "an even count is outside");
    assert_eq!(pixel(&img, 15, 35)[0], 255);
}

#[test]
fn gradients_opacity_and_transform_reach_the_pixels() {
    let black = Color::rgba(0.0, 0.0, 0.0, 1.0);
    let white = Color::rgba(1.0, 1.0, 1.0, 1.0);
    let stops: Arc<[ColorStop]> =
        Arc::from([ColorStop { offset: 0.0, color: black }, ColorStop { offset: 1.0, color: white }]);
    let linear = PathFill {
        paint: PathPaint::Linear {
            start: Point::new(0.0, 0.0),
            end: Point::new(100.0, 0.0),
            stops: stops.clone(),
        },
        ..PathFill::solid(box_path(0.0, 0.0, 100.0, 50.0), white)
    };
    let Some(img) = draw_paths(vec![linear]) else { return };
    assert!(close(pixel(&img, 50, 20), [128, 128, 128, 255], 4), "{:?}", pixel(&img, 50, 20));
    assert!(pixel(&img, 90, 20)[0] > 215);

    let radial = PathFill {
        paint: PathPaint::Radial { center: Point::new(50.0, 50.0), radius: 40.0, stops },
        ..PathFill::solid(box_path(0.0, 0.0, 100.0, 100.0), white)
    };
    let Some(img) = draw_paths(vec![radial]) else { return };
    assert!(pixel(&img, 50, 50)[0] < 10, "{:?}", pixel(&img, 50, 50));
    assert!(close(pixel(&img, 70, 50), [128, 128, 128, 255], 6), "{:?}", pixel(&img, 70, 50));

    let moved = PathFill {
        transform: [2.0, 0.0, 0.0, 2.0, 40.0, 10.0],
        opacity: 0.5,
        ..PathFill::solid(box_path(0.0, 0.0, 10.0, 10.0), white)
    };
    let Some(img) = draw_paths(vec![moved]) else { return };
    assert!(close(pixel(&img, 50, 20), [128, 128, 128, 255], 2), "{:?}", pixel(&img, 50, 20));
    assert_eq!(pixel(&img, 39, 20)[0], 0);
    assert_eq!(pixel(&img, 61, 20)[0], 0);
}

#[test]
fn paths_keep_their_place_among_other_primitives_and_obey_the_clip() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let (red, green) = (Color::rgba(1.0, 0.0, 0.0, 1.0), Color::rgba(0.0, 1.0, 0.0, 1.0));
    let mut scene = Scene::new();
    scene.set_clear_color(Color::rgba(0.0, 0.0, 0.0, 1.0));
    scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 40.0, 40.0), red));
    scene.push_path(PathFill::solid(box_path(20.0, 0.0, 60.0, 40.0), green));
    scene.push_quad(Quad::new(Rect::new(50.0, 0.0, 40.0, 40.0), red));
    scene.push_clip(Rect::new(0.0, 50.0, 30.0, 30.0));
    scene.push_path(PathFill::solid(box_path(10.0, 40.0, 100.0, 90.0), green));
    scene.pop_clip();
    let img = render(&gpu, &mut renderer, &scene, 2.0);
    assert_eq!(pixel(&img, 20, 20), [255, 0, 0, 255], "the quad before it");
    assert_eq!(pixel(&img, 60, 20), [0, 255, 0, 255], "the path over that");
    assert_eq!(pixel(&img, 110, 20), [255, 0, 0, 255], "and the quad after over the path");
    assert_eq!(pixel(&img, 40, 120), [0, 255, 0, 255], "inside the clip");
    assert_eq!(pixel(&img, 80, 120), [0, 0, 0, 255], "past it");
    assert_eq!(pixel(&img, 40, 90), [0, 0, 0, 255], "above it");
}

#[test]
fn an_image_seen_through_a_path_goes_where_it_is_placed() {
    // Red on the left half, and fully transparent green on the right.
    let pixels = [[255, 0, 0, 255], [255, 0, 0, 255], [0, 255, 0, 0], [0, 255, 0, 0]].concat();
    let image = ImageData::from_rgba8(4, 1, pixels).expect("pixels");
    let paint = |placement| PathPaint::Image { image: image.clone(), placement };
    let path = |x0, placement| PathFill {
        elements: box_path(x0, 0.0, x0 + 40.0, 40.0).into(),
        rule: FillRule::NonZero,
        paint: paint(placement),
        transform: PathFill::IDENTITY,
        opacity: 1.0,
        clip: None,
    };
    let flipped = [-40.0, 0.0, 0.0, 40.0, 140.0, 0.0];
    let Some(drawn) = draw_paths(vec![path(20.0, [40.0, 0.0, 0.0, 40.0, 20.0, 0.0]), path(100.0, flipped)])
    else {
        return;
    };
    let at = |x: u32, y: u32| {
        let p = pixel(&drawn, x, y);
        [p[0], p[1], p[2]]
    };
    assert_eq!(at(25, 20), [255, 0, 0], "its opaque side");
    assert_eq!(at(55, 20), [0, 0, 0], "what is transparent shows what is behind, with no green");
    assert_eq!(at(10, 20), [0, 0, 0], "nothing past the outline");
    assert_eq!(at(135, 20), [255, 0, 0], "mirrored, the opaque side is now on the right");
    assert_eq!(at(105, 20), [0, 0, 0]);
    let blend = at(40, 20);
    assert!(blend[0] > 0 && blend[0] < 255 && blend[1] == 0, "edges fade, and never toward green: {blend:?}");
}

#[test]
fn more_shapes_than_the_coverage_atlas_holds_are_all_drawn_in_order() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let (white, red) = (Color::rgba(1.0, 1.0, 1.0, 1.0), Color::rgba(1.0, 0.0, 0.0, 1.0));
    let blue = Color::rgba(0.0, 0.0, 1.0, 1.0);
    let window = box_path(0.0, 0.0, W as f32, H as f32);
    let mut scene = Scene::new();
    scene.set_clear_color(Color::rgba(0.0, 0.0, 0.0, 1.0));
    // At 4x, 800x400 pixels each: about 209 fit in an atlas of 8192.
    for _ in 0..300 {
        scene.push_path(PathFill::solid(window.clone(), white));
    }
    // What comes between shapes keeps its place, even with an atlas filled twice around it.
    scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 100.0, 100.0), blue));
    for _ in 0..300 {
        scene.push_path(PathFill::solid(box_path(0.0, 0.0, W as f32, 50.0), white));
    }
    scene.push_path(PathFill::solid(box_path(0.0, 0.0, 50.0, 50.0), red));
    let img = render(&gpu, &mut renderer, &scene, 4.0);
    assert_eq!(pixel(&img, 40, 40), [255, 0, 0, 255], "the last shape of all is on top");
    assert_eq!(pixel(&img, 40, 300), [0, 0, 255, 255], "the quad, over the first shapes");
    assert_eq!(pixel(&img, 600, 300), [255, 255, 255, 255], "the first shapes, past the quad");
    assert_eq!(pixel(&img, 300, 40), [255, 255, 255, 255], "the second shapes, over the quad");
}

/// The box around the pixels brighter than half on the red channel.
fn lit_box(img: &(u32, u32, Vec<u8>)) -> Option<(u32, u32, u32, u32)> {
    let mut found: Option<(u32, u32, u32, u32)> = None;
    for y in 0..img.1 {
        for x in 0..img.0 {
            if pixel(img, x, y)[0] > 128 {
                found =
                    Some(found.map_or((x, y, x, y), |(x0, y0, x1, y1)| {
                        (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
                    }));
            }
        }
    }
    found
}

#[test]
fn a_turned_quad_and_image_cover_the_turned_box() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let turn = std::f32::consts::FRAC_PI_2;
    let image = ImageData::from_fn(4, 4, |_, _| Color::hex(0xff0000)).expect("an image");
    for kind in ["quad", "image"] {
        let mut scene = Scene::new();
        scene.set_clear_color(Color::BLACK);
        // 40 by 10 around (100, 50): turned a quarter, it stands 10 wide and 40 tall.
        let rect = Rect::new(80.0, 45.0, 40.0, 10.0);
        if kind == "quad" {
            scene.push_quad(Quad::new(rect, Color::hex(0xff0000)).rotated(turn));
        } else {
            scene.push_image(ravenrs_core::ImageQuad::new(rect, image.clone()).rotated(turn));
        }
        let img = render(&gpu, &mut renderer, &scene, 1.0);
        assert!(pixel(&img, 100, 33)[0] > 200, "{kind}: the top of the standing strip");
        assert!(pixel(&img, 100, 67)[0] > 200, "{kind}: its bottom");
        assert!(pixel(&img, 85, 50)[0] < 30, "{kind}: where the lying strip was");
        assert!(pixel(&img, 115, 50)[0] < 30, "{kind}: and its other end");
        let (x0, y0, x1, y1) = lit_box(&img).expect("something drawn");
        assert!((94..=96).contains(&x0) && (104..=106).contains(&x1), "{kind}: {x0}..{x1}");
        assert!((29..=31).contains(&y0) && (69..=71).contains(&y1), "{kind}: {y0}..{y1}");
    }
}

#[test]
fn turned_text_runs_down_a_column() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut text = TextSystem::new();
    let mut layout = text
        .layout("Raven", &TextAttrs { size: 16.0, line_height: 20.0, weight: 600.0, ..Default::default() });
    layout.measure(None);
    let mut scene = Scene::new();
    scene.set_clear_color(Color::BLACK);
    layout.paint(Point::new(100.0, 10.0), Color::WHITE, &mut scene);
    let flat = lit_box(&render(&gpu, &mut renderer, &scene, 1.0)).expect("flat text");
    assert!(flat.2 - flat.0 > flat.3 - flat.1, "flat text is wider than tall: {flat:?}");
    let end = scene.len();
    scene.apply_effect(
        0..end,
        ravenrs_core::Effect::rotated(std::f32::consts::FRAC_PI_2),
        Point::new(100.0, 10.0),
    );
    let (x0, y0, x1, y1) = lit_box(&render(&gpu, &mut renderer, &scene, 1.0)).expect("turned text");
    assert!(y1 - y0 > (x1 - x0) * 2, "turned text is taller than wide: {:?}", (x0, y0, x1, y1));
    assert!(x1 <= 101 && y0 >= 9, "it turned around its start, to the left and down: {:?}", (x0, y0, x1, y1));
}

mod cables {
    use ravenrs_core::cable::{Cable, CableEnd, CableWorld, CpuCables};
    use ravenrs_core::{Color, Point, Rect, Scene};
    use ravenrs_render::{GpuCables, Renderer};

    use super::{FORMAT, close, gpu, pixel, render, render_counted};

    fn cable() -> Cable {
        let mut c = Cable::new(
            7,
            CableEnd::at(Point::new(20.0, 20.0)).toward(1.0, 0.0),
            CableEnd::at(Point::new(180.0, 20.0)).toward(-1.0, 0.0),
        );
        c.grow = true;
        c.look.color = Color::rgb(1.0, 0.0, 0.0);
        c.look.shine = 0.0;
        c.look.thickness = 6.0;
        c
    }

    fn view() -> Rect {
        Rect::new(0.0, 0.0, 200.0, 100.0)
    }

    #[test]
    fn the_gpu_world_moves_cables_as_the_cpu_world_does() {
        let Some(gpu) = gpu() else { return };
        let Some(mut on_gpu) = GpuCables::new(&gpu.device, &gpu.queue) else {
            eprintln!("skipping: no compute on this device");
            return;
        };
        let mut on_cpu = CpuCables::new();
        let c = cable();
        for _ in 0..45 {
            for world in [&mut on_gpu as &mut dyn CableWorld, &mut on_cpu] {
                world.sync(std::slice::from_ref(&c), view(), false);
                world.step(1.0 / 60.0);
            }
            on_gpu.finish();
        }
        assert!(on_gpu.dispatches() >= 45);
        let (a, b) = (on_gpu.shape(7).expect("gpu"), on_cpu.shape(7).expect("cpu"));
        let lowest = b.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(lowest > 30.0, "it hangs: {lowest}");
        for (p, q) in a.iter().zip(b.iter()) {
            assert!((p.x - q.x).abs() < 0.5 && (p.y - q.y).abs() < 0.5, "gpu {p:?} vs cpu {q:?}");
        }
    }

    #[test]
    fn the_renderer_draws_a_cable_from_the_gpu_buffer() {
        let Some(gpu) = gpu() else { return };
        let Some(mut world) = GpuCables::new(&gpu.device, &gpu.queue) else { return };
        let mut c = cable();
        c.grow = false;
        world.sync(std::slice::from_ref(&c), view(), false);
        world.step(1.0 / 60.0);
        world.finish();
        let shape = world.shape(7).expect("placed");
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
        renderer.set_cables(Some(world.drawer()));
        let mut scene = Scene::new();
        scene.set_clear_color(Color::WHITE);
        scene.push_cable(c.placed(shape.clone()));
        let img = render(&gpu, &mut renderer, &scene, 1.0);
        let middle = shape[shape.len() / 2];
        let at = pixel(&img, middle.x.round() as u32, middle.y.round() as u32);
        assert!(close(at, [255, 0, 0, 255], 40), "red at the middle of the cable: {at:?}");
        assert!(close(pixel(&img, 100, 5), [255, 255, 255, 255], 2), "white above it");
    }

    #[test]
    fn a_picture_on_a_cable_is_drawn_by_the_gpu_as_the_shapes_draw_it() {
        use ravenrs_core::ImageData;
        use ravenrs_core::cable::{CableTexture, cable_primitives};
        let Some(gpu) = gpu() else { return };
        let Some(mut world) = GpuCables::new(&gpu.device, &gpu.queue) else { return };
        // Blue on the side the cable's left hand points to, green on the other.
        let image = ImageData::from_fn(4, 8, |_, y| {
            if y < 4 { Color::rgb(0.0, 0.0, 1.0) } else { Color::rgb(0.0, 1.0, 0.0) }
        })
        .expect("picture");
        let mut c = cable();
        c.grow = false;
        c.look.thickness = 12.0;
        c.look.texture = CableTexture::Image { image, repeat: 24.0 };
        world.sync(std::slice::from_ref(&c), view(), false);
        world.step(1.0 / 60.0);
        world.finish();
        let shape = world.shape(7).expect("placed");
        let placed = c.placed(shape.clone());
        let middle = shape[shape.len() / 2];
        let (x, y) = (middle.x.round() as u32, middle.y.round() as u32);

        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
        renderer.set_cables(Some(world.drawer()));
        let mut scene = Scene::new();
        scene.set_clear_color(Color::WHITE);
        scene.push_cable(placed.clone());
        let (on_gpu, stats) = render_counted(&gpu, &mut renderer, &scene, 1.0);
        assert_eq!(stats.paths, 0, "no shapes: the picture went to the GPU");

        let mut shapes = Scene::new();
        shapes.set_clear_color(Color::WHITE);
        shapes.extend_from_slice(&cable_primitives(&placed, &shape));
        let mut plain = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
        let by_shapes = render(&gpu, &mut plain, &shapes, 1.0);
        for (dy, color) in [(-3_i32, [0, 0, 255, 255]), (3, [0, 255, 0, 255])] {
            let row = (y as i32 + dy) as u32;
            let (a, b) = (pixel(&on_gpu, x, row), pixel(&by_shapes, x, row));
            assert!(close(a, color, 40), "gpu at {dy}: {a:?}");
            assert!(close(a, b, 40), "gpu {a:?} and shapes {b:?} at {dy}");
        }
    }
}
