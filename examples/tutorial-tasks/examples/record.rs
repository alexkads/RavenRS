//! Gera o filme nativo e quadros reais para a edição do tutorial.
//! `cargo run -p tutorial-tasks --example record -- <pasta-de-saida>`

use std::path::Path;

use ravenrs::core::Scene;
use ravenrs::movie::Recorder;
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, Renderer, wgpu};

fn capture(gpu: &GpuContext, scene: &Scene, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = (1200_u32, 760_u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("RavenDevLab tutorial"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    renderer.render(scene, &view, (w, h), 1.0);
    let padded = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tutorial readback"),
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
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
    rx.recv()??;
    let data = slice.get_mapped_range()?;
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    let image = image::RgbaImage::from_raw(w, h, pixels).ok_or("invalid image size")?;
    image.save(path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or("informe a pasta de saída")?;
    let output = Path::new(&output);
    std::fs::create_dir_all(output)?;
    let mut take = Recorder::new(tutorial_tasks::app(), Theme::raven_dark(), Size::new(1200.0, 760.0));
    take.wait(1.0);
    take.chapter("A lista gerada", "A entidade Task gera a navegação e a lista.");
    take.wait(2.0);
    take.chapter("Novo registro", "Abra o formulário pelo comando Novo.");
    take.click("Novo")?;
    take.wait(2.0);
    take.chapter("Título obrigatório", "O campo required não aceita um título vazio.");
    take.click("Salvar")?;
    take.wait(2.0);
    if !take.ui_mut().access_tree().nodes.iter().any(|node| {
        node.name.as_deref().is_some_and(|name| name.contains("Título é obrigatório"))
            || node.value.as_deref().is_some_and(|value| value.contains("Título é obrigatório"))
    }) {
        return Err("a validação de título obrigatório não apareceu".into());
    }
    take.chapter("Preencher a tarefa", "Digite o título da primeira tarefa.");
    take.type_into("Título", "Publicar meu primeiro app")?;
    take.wait(2.0);
    take.chapter("Salvar e fechar", "A tarefa aparece na lista.");
    take.click("Salvar e fechar")?;
    take.wait(3.0);
    // Check behavior before showing it in a tutorial.
    let has_title = take.ui_mut().access_tree().nodes.iter().any(|node| {
        node.name.as_deref() == Some("Publicar meu primeiro app")
            || node.value.as_deref() == Some("Publicar meu primeiro app")
    });
    if !has_title {
        return Err("a tarefa não apareceu na lista".into());
    }
    let movie = take.finish()?;
    std::fs::write(output.join("tarefas.ravenmovie"), movie.to_bytes())?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None))?;
    for i in 0..movie.chapters().len() {
        let end = movie.chapters().get(i + 1).map_or(movie.duration_ms(), |next| next.at_ms);
        let at = end.saturating_sub(100);
        let (_, list) = movie.draw_list(at).ok_or("missing frame")?;
        let primitives: Vec<_> = list.iter().filter_map(|id| movie.primitive(*id)).cloned().collect();
        let mut scene = Scene::new();
        scene.set_clear_color(movie.background());
        scene.extend_from_slice(&primitives);
        capture(&gpu, &scene, &output.join(format!("app-{i}.png")))?;
    }
    println!("Filme gravado: {} ms, {} capítulos", movie.duration_ms(), movie.chapters().len());
    Ok(())
}
