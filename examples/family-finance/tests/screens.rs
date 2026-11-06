//! Draws every screen on the GPU into PNG files, to look at them and to keep the
//! README's pictures current. It only runs when asked:
//!
//! ```text
//! FF_SCREENSHOTS=target/screens cargo test -p family-finance --test screens
//! FF_SCREENSHOTS_SIZES=target/sizes cargo test -p family-finance --test screens draw_at_other_sizes
//! FF_SCREENSHOTS_STATES=target/states cargo test -p family-finance --test screens draw_the_states
//! FF_SCREENSHOTS_FILM=target/film cargo test -p family-finance --test screens draw_the_film
//! ```
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::path::Path;

use family_finance::db::Database;
use family_finance::ui::App;
use family_finance::update::{News, UpdateStatus, Updater};
use family_finance::{seed, themes};
use ravenrs::core::{AdapterReport, DiagnosticReport, FrameSample, GpuReport, SurfaceReport, SystemReport};
use ravenrs::core::{Event, Key, Modifiers, PointerButton};
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
    capture_now(gpu, ui, size, file);
}

/// Draws the scene as it is, without giving it more frames.
fn capture_now(gpu: &GpuContext, ui: &Ui, size: Size, file: &Path) {
    let scale = 2.0;
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
fn diagnostics_panel() -> (Ui, Diagnostics) {
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
    diagnostics.start_test();
    let host = DiagnosticsHost::new(app, diagnostics.clone());
    (Ui::new(Localized::new(Labels::pt_br(), host), themes::light()), diagnostics)
}

/// What a screen looks like beyond opening it: forms, menus, an empty database and the themes.
#[test]
fn draw_the_states() {
    let Some(folder) = std::env::var_os("FF_SCREENSHOTS_STATES") else { return };
    let folder = Path::new(&folder);
    std::fs::create_dir_all(folder).expect("folder");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else {
        eprintln!("no GPU adapter: nothing drawn");
        return;
    };
    let size = Size::new(1360.0, 860.0);
    let press = |ui: &mut Ui, name: &str| {
        let tree = ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)));
        let id = found.unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
        ui.update();
    };
    let shots: [(&str, usize, &[&str]); 11] = [
        ("novo-menu", 0, &["Novo"]),
        ("nova-despesa", 1, &["Nova despesa"]),
        ("nova-receita", 1, &["Nova receita"]),
        ("filtrar-menu", 1, &["Filtrar"]),
        ("filtro-avancado", 1, &["Filtrar", "Filtro avançado"]),
        ("nova-conta", 2, &["Nova conta"]),
        ("pagar-conta", 0, &["Pagar Plano"]),
        ("programar", 3, &["Programar"]),
        ("agenda-mes", 3, &["Mês"]),
        ("limite", 4, &["Definir limite de Mercado"]),
        ("comandos", 0, &["Procurar comando"]),
    ];
    for (name, page, steps) in shots {
        let mut ui = app(themes::light(), page, Updater::idle());
        ui.set_viewport(size, 2.0);
        ui.update();
        ui.update();
        for step in steps {
            press(&mut ui, step);
        }
        capture(&gpu, &mut ui, size, &folder.join(format!("{name}.png")));
    }
    // Nothing yet: what someone who starts from zero sees.
    let today = Date::new(2026, 11, 15).expect("valid date");
    for (page, name) in NAMES.iter().enumerate() {
        let db = Database::in_memory().expect("database");
        seed::load_defaults(&db).expect("defaults");
        let app = App::new(db, today, None, Updater::idle());
        app.open_page(page);
        let mut ui = Ui::new(app, themes::light());
        capture(&gpu, &mut ui, size, &folder.join(format!("vazio-{name}.png")));
    }
    // The themes.
    for (theme, label) in [(themes::dark(), "escuro"), (themes::glass(), "vidro")] {
        for (page, name) in NAMES.iter().enumerate() {
            let mut ui = app(theme, page, Updater::idle());
            capture(&gpu, &mut ui, size, &folder.join(format!("{label}-{name}.png")));
        }
        let mut ui = app(theme, 8, Updater::idle());
        capture(&gpu, &mut ui, size, &folder.join(format!("{label}-treinamentos.png")));
    }
    // The agenda under the pointer: the tooltip with its row lit, and a right click.
    let mut agenda = app(themes::light(), 3, Updater::idle());
    agenda.set_viewport(size, 2.0);
    for _ in 0..4 {
        agenda.update();
    }
    let tree = agenda.access_tree();
    let bar = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Aula de natação")).expect("a bar").bounds;
    agenda.handle_event(&Event::PointerMove { position: bar.center() });
    agenda.update();
    capture(&gpu, &mut agenda, size, &folder.join("agenda-dica.png"));
    agenda.handle_event(&Event::PointerDown {
        position: bar.center(),
        button: ravenrs::core::PointerButton::Secondary,
    });
    agenda.update();
    capture(&gpu, &mut agenda, size, &folder.join("agenda-menu.png"));

    // The menu reduced to icons, and a "?" open.
    let mut menu = app(themes::light(), 2, Updater::idle());
    menu.set_viewport(size, 2.0);
    menu.update();
    menu.update();
    let tree = menu.access_tree();
    let tip = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref().is_some_and(|t| t.starts_with("Ajuda: Contas")))
        .expect("?");
    let at = tip.bounds.center();
    menu.handle_event(&Event::PointerMove { position: at });
    menu.update();
    capture(&gpu, &mut menu, size, &folder.join("ajuda-aberta.png"));
    menu.handle_event(&Event::PointerMove { position: ravenrs::core::Point::new(800.0, 700.0) });
    press(&mut menu, "Recolher menu");
    for frame in 0..90 {
        menu.set_time(f64::from(frame) / 60.0);
        menu.update();
    }
    capture(&gpu, &mut menu, size, &folder.join("menu-recolhido.png"));

    // The assistant, with an answer, and the sidebar as a sheet in a small window.
    let mut chat = app(themes::light(), 0, Updater::idle());
    chat.set_viewport(size, 2.0);
    chat.update();
    press(&mut chat, "Assistente");
    press(&mut chat, "Como pagar uma conta?");
    capture(&gpu, &mut chat, size, &folder.join("assistente-resposta.png"));

    // The bug report, with the box drawn around the problem.
    let mut report = app(themes::light(), 0, Updater::idle());
    report.set_viewport(size, 2.0);
    let mut now = 0.0;
    let mut tick = |ui: &mut Ui, seconds: f64| {
        let end: f64 = now + seconds;
        while now < end {
            now += 0.1;
            ui.set_time(now);
            ui.update();
        }
    };
    tick(&mut report, 1.0);
    press(&mut report, "Relatar problema");
    tick(&mut report, 1.0);
    capture_now(&gpu, &report, size, &folder.join("relato.png"));
    press(&mut report, "Marcar o problema na tela");
    tick(&mut report, 1.0);
    let (from, to) = (ravenrs::core::Point::new(280.0, 130.0), ravenrs::core::Point::new(900.0, 520.0));
    report.handle_event(&Event::PointerMove { position: from });
    report.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    report.handle_event(&Event::PointerMove { position: to });
    tick(&mut report, 0.3);
    capture_now(&gpu, &report, size, &folder.join("relato-marcando.png"));
    report.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    tick(&mut report, 1.0);
    capture_now(&gpu, &report, size, &folder.join("relato-marcado.png"));
    let small = Size::new(800.0, 640.0);
    let mut sheet = app(themes::light(), 0, Updater::idle());
    sheet.set_viewport(small, 2.0);
    sheet.update();
    sheet.update();
    let tree = sheet.access_tree();
    let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).take(12).collect();
    println!("{names:?}");
    capture(&gpu, &mut sheet, small, &folder.join("pequena-painel.png"));
}

/// The first frames of the window and of a change of screen, frame by frame.
#[test]
fn draw_the_opening() {
    let Some(folder) = std::env::var_os("FF_SCREENSHOTS_OPENING") else { return };
    let folder = Path::new(&folder);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else { return };
    let size = Size::new(1360.0, 860.0);
    let frames = [0.0, 0.03, 0.08, 0.15, 0.3, 0.6];
    let mut ui = app(themes::light(), 0, Updater::idle());
    ui.set_viewport(size, 2.0);
    for (i, at) in frames.iter().enumerate() {
        ui.set_time(*at);
        ui.update();
        capture_now(&gpu, &ui, size, &folder.join(format!("abrir-{i}.png")));
    }
    let tree = ui.access_tree();
    let row = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Agenda")).expect("row").id;
    ui.perform_action(row, AccessAction::Click);
    for (i, at) in frames.iter().enumerate() {
        ui.set_time(1.0 + *at);
        ui.update();
        capture_now(&gpu, &ui, size, &folder.join(format!("trocar-{i}.png")));
    }
    let tree = ui.access_tree();
    let button = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Programar")).expect("button").id;
    ui.perform_action(button, AccessAction::Click);
    for (i, at) in frames.iter().enumerate() {
        ui.set_time(2.0 + *at);
        ui.update();
        capture_now(&gpu, &ui, size, &folder.join(format!("dialogo-{i}.png")));
    }
}

/// A film as it plays: the zoom on the way to a step, the step, and the end.
#[test]
fn draw_the_film() {
    let Some(folder) = std::env::var_os("FF_SCREENSHOTS_FILM") else { return };
    let folder = Path::new(&folder);
    std::fs::create_dir_all(folder).expect("folder");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else { return };
    let size = Size::new(1360.0, 860.0);
    let mut ui = app(themes::light(), 8, Updater::idle());
    ui.set_viewport(size, 2.0);
    ui.set_time(0.0);
    ui.update();
    let click = |ui: &mut Ui, prefix: &str| {
        let tree = ui.access_tree();
        let node = tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(prefix)));
        let id = node.unwrap_or_else(|| panic!("no {prefix}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
    };
    let says = |ui: &mut Ui, prefix: &str| {
        ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|t| t.starts_with(prefix)))
    };
    click(&mut ui, "Reproduzir");
    let mut at = 0.0;
    let mut shot = 0;
    while at < 240.0 && !says(&mut ui, "Passo ") {
        at += 1.0 / 30.0;
        ui.set_time(at);
        ui.update();
        if ((at * 30.0).round() as u64).is_multiple_of(150) {
            capture_now(&gpu, &ui, size, &folder.join(format!("tocando-{shot}.png")));
            shot += 1;
        }
    }
    for _ in 0..30 {
        at += 1.0 / 30.0;
        ui.set_time(at);
        ui.update();
    }
    capture_now(&gpu, &ui, size, &folder.join("passo.png"));
    click(&mut ui, "Parar em cada passo");
    click(&mut ui, "Reproduzir");
    while at < 480.0 && !says(&mut ui, "Você terminou") {
        at += 1.0 / 30.0;
        ui.set_time(at);
        ui.update();
    }
    for _ in 0..30 {
        at += 1.0 / 30.0;
        ui.set_time(at);
        ui.update();
    }
    capture_now(&gpu, &ui, size, &folder.join("fim.png"));
}

/// The screens at other window sizes, to see how they adapt.
#[test]
fn draw_at_other_sizes() {
    let Some(folder) = std::env::var_os("FF_SCREENSHOTS_SIZES") else { return };
    let folder = Path::new(&folder);
    std::fs::create_dir_all(folder).expect("folder");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else {
        eprintln!("no GPU adapter: nothing drawn");
        return;
    };
    for (width, height) in [(800.0, 640.0), (1100.0, 720.0), (1360.0, 860.0), (1920.0, 1080.0)] {
        let size = Size::new(width, height);
        for (page, name) in NAMES.iter().enumerate() {
            let mut ui = app(themes::light(), page, Updater::idle());
            capture(&gpu, &mut ui, size, &folder.join(format!("{name}-{width}.png")));
        }
        // Training: the page, a question, and the moment it opens, with its box.
        let mut page = app(themes::light(), 8, Updater::idle());
        capture(&gpu, &mut page, size, &folder.join(format!("treinamentos-{width}.png")));
        let tree = page.access_tree();
        let field = tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("the question field").id;
        page.perform_action(field, AccessAction::Focus);
        for letter in "como pago uma conta".chars() {
            page.handle_event(&Event::KeyDown {
                key: Key::Character(letter.to_string()),
                modifiers: Modifiers::default(),
            });
            page.update();
        }
        capture(&gpu, &mut page, size, &folder.join(format!("treino-pergunta-{width}.png")));
        page.handle_event(&Event::KeyDown { key: Key::Enter, modifiers: Modifiers::default() });
        page.update();
        capture(&gpu, &mut page, size, &folder.join(format!("treino-resposta-{width}.png")));
    }
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
        let mut ui = app(themes::light(), page, Updater::idle());
        capture(&gpu, &mut ui, size, &folder.join(format!("{name}.png")));
    }
    // The grid, grouped by dragging columns to its panel, one group inside the other.
    let header = |ui: &mut Ui, name: &str| {
        let tree = ui.access_tree();
        let found =
            tree.nodes.iter().find(|n| n.role == Role::ColumnHeader && n.name.as_deref() == Some(name));
        found.unwrap_or_else(|| panic!("no {name}")).bounds
    };
    let mut ui = app(themes::light(), 1, Updater::idle());
    capture(&gpu, &mut ui, size, &folder.join("grade-1-inicial.png"));
    for (name, nudge) in [("Conta", 0.0), ("Categoria", 200.0)] {
        let from = header(&mut ui, name).center();
        let panel = header(&mut ui, "Data");
        let to = Point::new(panel.x() + 400.0 + nudge, panel.y() - 20.0);
        ui.handle_event(&Event::PointerMove { position: from });
        ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
        ui.update();
        ui.handle_event(&Event::PointerMove { position: to });
        ui.update();
        ui.update();
        capture(&gpu, &mut ui, size, &folder.join(format!("grade-2-arrastando-{name}.png")));
        ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
        ui.update();
        ui.update();
    }
    capture(&gpu, &mut ui, size, &folder.join("grade-3-agrupada.png"));
    let at = header(&mut ui, "Valor").center();
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Secondary });
    capture(&gpu, &mut ui, size, &folder.join("grade-4-menu.png"));
    ui.handle_event(&Event::KeyDown { key: Key::Escape, modifiers: Modifiers::default() });
    ui.update();
    let press = |ui: &mut Ui, role: Role, name: &str| {
        let tree = ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == role && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no {name}")).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
        ui.update();
    };
    press(&mut ui, Role::Button, "Editar resumo Valor");
    capture(&gpu, &mut ui, size, &folder.join("grade-5-resumo.png"));
    press(&mut ui, Role::RadioButton, "Fórmula");
    capture(&gpu, &mut ui, size, &folder.join("grade-6-formula.png"));
    // A report whose rows have no barcode to leave room for.
    let mut ui = app(themes::light(), 5, Updater::idle());
    capture(&gpu, &mut ui, size, &folder.join("relatorio-extrato.png"));
    let tree = ui.access_tree();
    let tab = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::RadioButton && n.name.as_deref() == Some("Contas a pagar"));
    ui.perform_action(tab.expect("the tab").id, AccessAction::Click);
    capture(&gpu, &mut ui, size, &folder.join("relatorio-contas.png"));
    // The whole of the first screen, with the pointer over its charts.
    let tall = Size::new(1360.0, 1560.0);
    for (theme, name) in [(themes::light(), "painel-inteiro"), (themes::dark(), "painel-escuro")] {
        let mut ui = app(theme, 0, Updater::idle());
        capture(&gpu, &mut ui, tall, &folder.join(format!("{name}.png")));
        ui.handle_event(&Event::PointerMove { position: Point::new(800.0, 900.0) });
        ui.update();
        ui.update();
        capture(&gpu, &mut ui, tall, &folder.join(format!("{name}-apontado.png")));
    }
    // The grid and the timeline on the glass theme, whose tokens are translucent.
    let mut glass = app(themes::glass(), 2, Updater::idle());
    capture(&gpu, &mut glass, size, &folder.join("contas-vidro.png"));
    let mut glass = app(themes::glass(), 3, Updater::idle());
    capture(&gpu, &mut glass, size, &folder.join("agenda-vidro.png"));

    // A made-up test: 16 frames a second, most of each spent drawing.
    let (mut panel, diagnostics) = diagnostics_panel();
    let ms = std::time::Duration::from_millis;
    for frame in 0..=80_u32 {
        panel.set_viewport(size, 2.0);
        panel.set_time(f64::from(frame) / 16.0);
        panel.update();
        diagnostics.record(FrameSample {
            update: ms(u64::from(9 + frame % 5)),
            acquire: ms(2),
            render: ms(u64::from(38 + frame % 21)),
            present: ms(1),
            quads: 840,
            glyphs: 2310,
            draw_calls: 46,
            size: (2720, 1720),
            scale: 2.0,
            ..Default::default()
        });
    }
    for frame in 0..60 {
        panel.set_time(5.0 + f64::from(frame) / 60.0);
        panel.update();
    }
    capture(&gpu, &mut panel, size, &folder.join("diagnostico.png"));

    let mut ajustes = app(themes::light(), 7, Updater::idle());
    capture(&gpu, &mut ajustes, size, &folder.join("ajustes-desempenho.png"));

    let mut dark = app(themes::dark(), 0, Updater::idle());
    capture(&gpu, &mut dark, size, &folder.join("painel-escuro.png"));

    // The training films: the page, moments of the film, and the editor over it.
    let film =
        ravenrs::movie::Movie::from_bytes(include_bytes!("../assets/treinamentos/lancamentos.ravenmovie"))
            .expect("film");
    let mut page = app(themes::light(), 8, Updater::idle());
    capture(&gpu, &mut page, size, &folder.join("treinamentos.png"));
    // A question, and the moment it opens.
    let field =
        page.access_tree().nodes.iter().find(|n| n.role == Role::TextInput).expect("the question field").id;
    page.perform_action(field, AccessAction::Focus);
    for letter in "como pago uma conta".chars() {
        page.handle_event(&Event::KeyDown {
            key: Key::Character(letter.to_string()),
            modifiers: Modifiers::default(),
        });
        page.update();
    }
    capture(&gpu, &mut page, size, &folder.join("treinamentos-pergunta.png"));
    page.handle_event(&Event::KeyDown { key: Key::Enter, modifiers: Modifiers::default() });
    page.update();
    capture(&gpu, &mut page, size, &folder.join("treinamentos-resposta.png"));
    // The assistant in the corner, before and after a question.
    let mut chat = app(themes::light(), 0, Updater::idle());
    capture(&gpu, &mut chat, size, &folder.join("assistente-fechado.png"));
    let button = |ui: &mut Ui, name: &str| {
        let tree = ui.access_tree();
        let node = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name));
        ui.perform_action(node.expect("the button").id, AccessAction::Click);
        ui.update();
    };
    button(&mut chat, "Assistente");
    capture(&gpu, &mut chat, size, &folder.join("assistente-aberto.png"));
    button(&mut chat, "Como pagar uma conta?");
    capture(&gpu, &mut chat, size, &folder.join("assistente-resposta.png"));
    let mut sphere = app(themes::dark(), 0, Updater::idle());
    capture(&gpu, &mut sphere, size, &folder.join("assistente-escuro.png"));
    let total = film.duration_ms();
    for (name, at) in
        [("titulo", 1_000), ("filme-a", 12_000), ("filme-b", total / 2), ("filme-c", total - 5_000)]
    {
        let mut moment =
            Ui::new(ravenrs::movie::Composition::new(film.clone(), at).width(size.width), themes::light());
        capture(&gpu, &mut moment, size, &folder.join(format!("filme-{name}.png")));
    }
    let state = ravenrs::movie::EditorState::new(film.clone());
    state.seek(12_000);
    state.select_overlay(film.project().overlays[2].id);
    let editor = ravenrs::movie::MovieEditor::new(state)
        .labels(ravenrs::movie::EditorLabels::pt_br())
        .player_labels(ravenrs::movie::MovieLabels::pt_br())
        .title("O essencial do Raven Finanças")
        .on_save(|_: &ravenrs::movie::Movie| {});
    let mut editor = Ui::new(editor, themes::dark());
    capture(&gpu, &mut editor, size, &folder.join("editor.png"));

    // Settings on a real file, with the copies it keeps.
    let data = folder.join("dados-dos-ajustes");
    let _ = std::fs::remove_dir_all(&data);
    let path = data.join("financas.db");
    let db = Database::open(&path).expect("database");
    let today = Date::new(2026, 11, 15).expect("valid date");
    seed::load_demo(&db, today).expect("seed");
    family_finance::backup::keep_daily_copy(&db, &family_finance::backup::copies_folder(&path), today)
        .expect("copy");
    let mut ui = Ui::new(App::new(db, today, Some(path), Updater::idle()), themes::light());
    ui.set_viewport(size, 1.0);
    ui.update();
    let press = |ui: &mut Ui, name: &str| {
        let tree = ui.access_tree();
        let id = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).id;
        ui.perform_action(id, AccessAction::Click);
        ui.update();
        ui.update();
    };
    press(&mut ui, "Ajustes");
    capture(&gpu, &mut ui, size, &folder.join("ajustes-backup.png"));
    let tree = ui.access_tree();
    let row = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref().is_some_and(|t| t.contains("Cópia do dia")))
        .expect("row")
        .id;
    ui.perform_action(row, AccessAction::Click);
    ui.update();
    ui.update();
    capture(&gpu, &mut ui, size, &folder.join("ajustes-restaurar.png"));

    let updater = Updater::idle();
    updater.set_status(UpdateStatus::Available(News {
        versao: "0.2.0".to_owned(),
        data: None,
        titulo: "Agora dá para anexar o boleto a cada conta.".to_owned(),
        importante: false,
        novidades: vec!["Anexar boleto".to_owned()],
        por_que_atualizar: "Para não perder o papel.".to_owned(),
        contribuidores: Vec::new(),
    }));
    let mut update = app(themes::light(), 0, updater);
    capture(&gpu, &mut update, size, &folder.join("atualizacao.png"));

    let news = |important, version: &str| News {
        versao: version.to_owned(),
        data: Some("2026-11-20".to_owned()),
        titulo: "Agora dá para anexar o boleto a cada conta.".to_owned(),
        importante: important,
        novidades: vec!["Anexar boleto".to_owned(), "Abrir o boleto a partir da conta".to_owned()],
        por_que_atualizar: "Para não perder o papel.".to_owned(),
        contribuidores: Vec::new(),
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
        let mut foot = app(themes::glass(), 0, updater);
        capture(&gpu, &mut foot, size, &folder.join(format!("{name}.png")));
    }
    let updater = Updater::idle();
    updater.set_status(UpdateStatus::Available(news(false, "0.2.0")));
    let mut dialog = app(themes::light(), 0, updater);
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
        let mut screen = app(themes::light(), 3, Updater::idle());
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
    let mut paying = app(themes::light(), 0, Updater::idle());
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
        let mut dialog = app(themes::light(), page, Updater::idle());
        dialog.set_viewport(size, 2.0);
        dialog.update();
        let tree = dialog.access_tree();
        let id = tree.nodes.iter().find(|n| n.name.as_deref() == Some(button)).expect("the button").id;
        dialog.perform_action(id, AccessAction::Click);
        capture(&gpu, &mut dialog, size, &folder.join(format!("{name}.png")));
    }
}
