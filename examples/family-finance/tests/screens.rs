//! Draws every screen on the GPU into PNG files, to look at them and to keep the
//! README's pictures current. It only runs when asked:
//!
//! ```text
//! FF_SCREENSHOTS=target/screens cargo test -p family-finance --test screens
//! ```
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::path::Path;

use family_finance::db::Database;
use family_finance::seed;
use family_finance::ui::App;
use family_finance::update::{News, UpdateStatus, Updater};
use ravenrs::core::{AdapterReport, DiagnosticReport, FrameSample, GpuReport, SurfaceReport, SystemReport};
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, Renderer, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NAMES: [&str; 8] =
    ["painel", "lancamentos", "contas", "agenda", "orcamentos", "relatorios", "cadastros", "ajustes"];

fn capture(gpu: &GpuContext, ui: &mut Ui, size: Size, file: &Path) {
    let scale = 2.0;
    ui.set_viewport(size, scale);
    for _ in 0..4 {
        ui.update();
    }
    let (w, h) = ((size.width * scale) as u32, (size.height * scale) as u32);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screen"),
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
    image::RgbaImage::from_raw(w, h, pixels).expect("image").save(file).expect("save");
}

fn app(theme: Theme, page: usize, updater: Updater) -> Ui {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let app = App::new(db, today, None, updater);
    app.open_page(page);
    Ui::new(app, theme)
}

/// The panel with a made-up report of an old GPU left without its driver.
fn diagnostics_panel() -> Ui {
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let app = App::new(db, today, None, Updater::idle());
    let diagnostics = Diagnostics::default();
    diagnostics.set_report_source(|| {
        let adapter = |name: &str, device_type: &str, software| AdapterReport {
            name: name.into(),
            backend: "vulkan".into(),
            device_type: device_type.into(),
            driver: "Mesa".into(),
            driver_info: "26.1.3".into(),
            software,
        };
        let cpu = adapter("llvmpipe (LLVM 21, 256 bits)", "Cpu", true);
        DiagnosticReport {
            gpu: Ok(GpuReport {
                active: cpu.clone(),
                available: vec![cpu],
                hidden: vec![adapter("Intel(R) Haswell Mobile", "IntegratedGpu", false)],
                surface: Some(SurfaceReport {
                    format: "Bgra8Unorm".into(),
                    present_mode: "AutoVsync".into(),
                    supported_present_modes: vec!["Fifo".into(), "Mailbox".into()],
                }),
            }),
            system: SystemReport {
                version: "27.1.1".into(),
                os: "Fedora Linux 44 (Workstation Edition)".into(),
                session: Some("wayland".into()),
                desktop: Some("GNOME".into()),
                vulkan_drivers: vec!["intel_hasvk_icd.x86_64.json".into(), "lvp_icd.x86_64.json".into()],
                overrides: vec![],
                optimized: true,
            },
        }
    });
    let ms = std::time::Duration::from_millis;
    for i in 0..60 {
        diagnostics.record(FrameSample {
            update: ms(9 + i % 5),
            acquire: ms(2),
            render: ms(38 + i % 21),
            present: ms(1),
            quads: 840,
            glyphs: 2310,
            draw_calls: 46,
            size: (2720, 1720),
            scale: 2.0,
        });
    }
    diagnostics.set_open(true);
    Ui::new(Localized::new(Labels::pt_br(), DiagnosticsHost::new(app, diagnostics)), Theme::raven_light())
}

#[test]
fn draw_every_screen() {
    let Some(folder) = std::env::var_os("FF_SCREENSHOTS") else { return };
    let folder = Path::new(&folder);
    std::fs::create_dir_all(folder).expect("folder");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else {
        eprintln!("no GPU adapter: nothing drawn");
        return;
    };
    let size = Size::new(1360.0, 860.0);
    for (page, name) in NAMES.iter().enumerate() {
        let mut ui = app(Theme::raven_light(), page, Updater::idle());
        capture(&gpu, &mut ui, size, &folder.join(format!("{name}.png")));
    }
    // The grid and the timeline on the glass theme, whose tokens are translucent.
    let mut glass = app(Theme::raven_glass(), 2, Updater::idle());
    capture(&gpu, &mut glass, size, &folder.join("contas-vidro.png"));
    let mut glass = app(Theme::raven_glass(), 3, Updater::idle());
    capture(&gpu, &mut glass, size, &folder.join("agenda-vidro.png"));

    let mut panel = diagnostics_panel();
    for frame in 0..120 {
        panel.set_viewport(size, 2.0);
        panel.set_time(f64::from(frame) / 60.0);
        panel.update();
    }
    capture(&gpu, &mut panel, size, &folder.join("diagnostico.png"));

    let mut dark = app(Theme::raven_dark(), 0, Updater::idle());
    capture(&gpu, &mut dark, size, &folder.join("painel-escuro.png"));

    let updater = Updater::idle();
    updater.set_status(UpdateStatus::Available(News {
        versao: "0.2.0".to_owned(),
        data: None,
        titulo: "Agora dá para anexar o boleto a cada conta.".to_owned(),
        importante: false,
        novidades: vec!["Anexar boleto".to_owned()],
        por_que_atualizar: "Para não perder o papel.".to_owned(),
    }));
    let mut update = app(Theme::raven_light(), 0, updater);
    capture(&gpu, &mut update, size, &folder.join("atualizacao.png"));

    let news = |important, version: &str| News {
        versao: version.to_owned(),
        data: Some("2026-11-20".to_owned()),
        titulo: "Agora dá para anexar o boleto a cada conta.".to_owned(),
        importante: important,
        novidades: vec!["Anexar boleto".to_owned(), "Abrir o boleto a partir da conta".to_owned()],
        por_que_atualizar: "Para não perder o papel.".to_owned(),
    };
    let states = [
        ("rodape-importante", UpdateStatus::Available(news(true, "0.2.0"))),
        ("rodape-compilando", UpdateStatus::Compiling(news(false, "0.2.0"))),
        ("rodape-instalada", UpdateStatus::Ready(news(false, "0.2.0"))),
        ("rodape-falhou", UpdateStatus::Failed(news(false, "0.2.0"), "a compilação falhou.".to_owned())),
        ("rodape-em-dia", UpdateStatus::UpToDate),
    ];
    for (name, status) in states {
        let updater = Updater::idle();
        updater.set_status(status);
        let mut foot = app(Theme::raven_glass(), 0, updater);
        capture(&gpu, &mut foot, size, &folder.join(format!("{name}.png")));
    }
    let updater = Updater::idle();
    updater.set_status(UpdateStatus::Available(news(false, "0.2.0")));
    let mut dialog = app(Theme::raven_light(), 0, updater);
    dialog.set_viewport(size, 2.0);
    dialog.update();
    let tree = dialog.access_tree();
    let id = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Ver novidades")).expect("the button").id;
    dialog.perform_action(id, AccessAction::Click);
    capture(&gpu, &mut dialog, size, &folder.join("novidades.png"));

    // The agenda's month, and the form that plans something.
    for (buttons, name) in [
        (vec![(Role::RadioButton, "Mês")], "agenda-mes"),
        (vec![(Role::Button, "Programar"), (Role::RadioButton, "Semanal")], "dialogo-programar"),
    ] {
        let mut screen = app(Theme::raven_light(), 3, Updater::idle());
        screen.set_viewport(size, 2.0);
        screen.update();
        screen.update();
        for (role, button) in buttons {
            let tree = screen.access_tree();
            let id = tree
                .nodes
                .iter()
                .find(|n| n.role == role && n.name.as_deref() == Some(button))
                .expect("the control")
                .id;
            screen.perform_action(id, AccessAction::Click);
            screen.update();
            screen.update();
        }
        capture(&gpu, &mut screen, size, &folder.join(format!("{name}.png")));
    }

    // A menu opened from inside a dialog: it sits under its field, above the backdrop.
    let mut paying = app(Theme::raven_light(), 0, Updater::idle());
    paying.set_viewport(size, 2.0);
    paying.update();
    for (role, start) in [(Role::Button, "Pagar "), (Role::ComboBox, "Pagar com")] {
        let tree = paying.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| n.role == role && n.name.as_deref().is_some_and(|t| t.starts_with(start)))
            .expect("the control")
            .id;
        paying.perform_action(id, AccessAction::Click);
        paying.update();
        paying.update();
    }
    capture(&gpu, &mut paying, size, &folder.join("dialogo-pagamento-menu.png"));

    for (page, button, name) in
        [(1, "Nova despesa", "dialogo-lancamento"), (2, "Nova conta", "dialogo-conta")]
    {
        let mut dialog = app(Theme::raven_light(), page, Updater::idle());
        dialog.set_viewport(size, 2.0);
        dialog.update();
        let tree = dialog.access_tree();
        let id = tree.nodes.iter().find(|n| n.name.as_deref() == Some(button)).expect("the button").id;
        dialog.perform_action(id, AccessAction::Click);
        capture(&gpu, &mut dialog, size, &folder.join(format!("{name}.png")));
    }
}
