//! Renders real pages of the sample reports with the report engine and the GPU
//! renderer, for the fan of pages on the project's home page:
//!
//! ```text
//! cargo run -p report-designer --example site_pages
//! ```
//!
//! Writes `site/assets/reports/page-N.jpg`, back to front. Rerun it when the engine
//! or the samples change how a page looks.

use std::path::PathBuf;

use ravenrs::core::{Point, Scene};
use ravenrs::render::{GpuContext, Renderer, wgpu};
use ravenrs::text::TextSystem;
use ravenrs_report::{Document, Report, ReportEngine, paint_page, samples};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Width of each picture in pixels: twice what the page shows on the site, for sharp
/// text on high-density screens.
const WIDTH: f32 = 520.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None))?;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    let mut engine = ReportEngine::new();

    let render = |engine: &mut ReportEngine, report: Report| -> Document {
        let data = report.embedded_data().cloned().unwrap_or(serde_json::Value::Null);
        engine.render(&report, &data)
    };
    let customers = render(&mut engine, samples::customer_list());
    let invoice = engine.render(&samples::invoice(), &samples::invoice_data());
    let dashboard = render(&mut engine, samples::sales_dashboard());
    let sales = render(&mut engine, samples::sales_by_region(10_000));
    for doc in [&customers, &invoice, &dashboard, &sales] {
        if !doc.errors.is_empty() {
            return Err(format!("{}: {:?}", doc.title, doc.errors).into());
        }
    }
    println!("sales by region: {} pages", sales.pages.len());

    // Back to front: the fan shows the last one on top.
    let last = sales.pages.len().saturating_sub(1);
    let pages = [(&customers, 0), (&invoice, 0), (&dashboard, 0), (&sales, last), (&sales, 1), (&sales, 0)];
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../site/assets/reports");
    std::fs::create_dir_all(&out)?;
    let mut text = TextSystem::new();
    for (n, (doc, index)) in pages.iter().enumerate() {
        let page = doc.pages.get(*index).ok_or("missing page")?;
        let scale = WIDTH / page.width;
        let (w, h) = (WIDTH as u32, (page.height * scale).round() as u32);
        let mut scene = Scene::new();
        paint_page(page, &mut text, &mut scene, Point::ZERO, scale);
        let pixels = draw(&gpu, &mut renderer, &scene, w, h)?;
        let rgb = image::RgbaImage::from_raw(w, h, pixels).ok_or("image size")?;
        let rgb = image::DynamicImage::ImageRgba8(rgb).to_rgb8();
        let path = out.join(format!("page-{}.jpg", n + 1));
        let mut file = std::io::BufWriter::new(std::fs::File::create(&path)?);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 82).encode_image(&rgb)?;
        println!("{} ({}, page {})", path.display(), doc.title, index + 1);
    }
    Ok(())
}

/// Renders `scene` into a `w` by `h` texture and reads the pixels back.
fn draw(gpu: &GpuContext, renderer: &mut Renderer, scene: &Scene, w: u32, h: u32) -> Result<Vec<u8>, String> {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("site page"),
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
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let data = slice.get_mapped_range().map_err(|e| format!("reading the picture back: {e:?}"))?;
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(data.get(start..start + (w * 4) as usize).ok_or("short readback")?);
    }
    Ok(pixels)
}
