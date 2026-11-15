//! Pictures of the screens, drawn with no window, to look at them next to the design. It only
//! runs when asked: `RAVEN_FLOWS_TELAS=pasta cargo test -p raven-flows --test telas -- --ignored`.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use raven_flows::Worker;
use ravenrs::core::AccessAction;
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, render_to_rgba, wgpu};

#[test]
#[ignore = "draws pictures into the folder given"]
fn pictures_of_the_screens() {
    let folder = std::path::PathBuf::from(std::env::var_os("RAVEN_FLOWS_TELAS").expect("a folder"));
    let home = folder.join("casa");
    let _ = std::fs::remove_dir_all(&home);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None)).expect("a GPU");
    let mut ui = Ui::new(raven_flows::app_in(&home, Worker::Inline).expect("opens"), Theme::raven_dark());
    let (width, height) = (1440, 900);
    ui.set_viewport(Size::new(width as f32, height as f32), 1.0);
    let settle = |ui: &mut Ui| {
        for _ in 0..6 {
            ui.update();
        }
    };
    let press = |ui: &mut Ui, name: &str| {
        let id = ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).id;
        ui.perform_action(id, AccessAction::Click);
        settle(ui);
    };
    let snap = |ui: &mut Ui, name: &str| {
        settle(ui);
        let pixels = render_to_rgba(&gpu, ui.scene(), width, height).expect("pixels");
        image::RgbaImage::from_raw(width, height, pixels)
            .expect("image")
            .save(folder.join(format!("{name}.png")))
            .expect("saved");
    };
    snap(&mut ui, "1-fluxos");
    press(&mut ui, "Conferir pedidos");
    press(&mut ui, "Abrir");
    snap(&mut ui, "2-editor");
    let at = ui
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Aprovar pedidos"))
        .expect("a node")
        .bounds
        .center();
    ui.handle_event(&ravenrs::core::Event::PointerMove { position: at });
    let button = ravenrs::core::PointerButton::Primary;
    ui.handle_event(&ravenrs::core::Event::PointerDown { position: at, button });
    ui.handle_event(&ravenrs::core::Event::PointerUp { position: at, button });
    snap(&mut ui, "2b-parametros");
    press(&mut ui, "Executar fluxo");
    snap(&mut ui, "3-executado");
    press(&mut ui, "Execuções");
    press(&mut ui, "Aguardando");
    snap(&mut ui, "4-execucoes");
    press(&mut ui, "Configurações");
    snap(&mut ui, "5-configuracoes");
    let _ = std::fs::remove_dir_all(&home);
}
