//! Pictures of the screens, drawn with no window over the server of the tests, to look at them
//! next to the design (#803, D-01). It only runs when asked:
//! `RAVEN_CI_TELAS=pasta cargo test -p raven-ci-painel --test telas -- --ignored`.
#![allow(clippy::expect_used, reason = "tests fail loudly")]
#![cfg(not(target_arch = "wasm32"))]

use ravenrs::core::AccessAction;
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, render_to_rgba, wgpu};

#[test]
#[ignore = "draws pictures into the folder given"]
fn pictures_of_the_screens() {
    let folder = std::path::PathBuf::from(std::env::var_os("RAVEN_CI_TELAS").expect("a folder"));
    let home = raven_ci_painel::e2e::folder("telas");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None)).expect("a GPU");
    let (_ci, painel) = raven_ci_painel::e2e::panel(&home).expect("the server of the tests");
    let (width, height) = (1440, 900);
    let mut door = Ui::new(
        Localized::new(Labels::pt_br(), raven_ci_painel::Painel::new(painel_client(&painel), false)),
        Theme::raven_dark(),
    );
    door.set_viewport(Size::new(width as f32, height as f32), 1.0);
    painel.sign_in("ana", raven_ci_painel::e2e::PASSWORD);
    let mut ui = Ui::new(Localized::new(Labels::pt_br(), painel), Theme::raven_dark());
    ui.set_viewport(Size::new(width as f32, height as f32), 1.0);
    let settle = |ui: &mut Ui| {
        for _ in 0..6 {
            ui.update();
        }
    };
    let snap = |ui: &mut Ui, name: &str| {
        settle(ui);
        let pixels = render_to_rgba(&gpu, ui.scene(), width, height).expect("pixels");
        image::RgbaImage::from_raw(width, height, pixels)
            .expect("image")
            .save(folder.join(format!("{name}.png")))
            .expect("saved");
    };
    let press = |ui: &mut Ui, name: &str| {
        settle(ui);
        let id = ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some(name)).expect(name).id;
        ui.perform_action(id, AccessAction::Click);
        settle(ui);
    };
    snap(&mut door, "0-entrar");
    snap(&mut ui, "1-jobs");
    press(&mut ui, "Abrir o último build apps/site");
    snap(&mut ui, "2-build");
    press(&mut ui, "Fila e agentes");
    snap(&mut ui, "3-fila");
    press(&mut ui, "Plugins");
    snap(&mut ui, "4-plugins");
    press(&mut ui, "Configuração");
    snap(&mut ui, "5-configuracao");
    press(&mut ui, "Travas");
    snap(&mut ui, "6-pagina-de-plugin");
    press(&mut ui, "Usuários e papéis");
    snap(&mut ui, "7-usuarios");
    press(&mut ui, "Histórico");
    snap(&mut ui, "8-historico");
    press(&mut ui, "Jobs");
    press(&mut ui, "Disparar api-pedidos");
    snap(&mut ui, "11-disparar");
    press(&mut ui, "Cancelar");
    // The same build in a narrow window and on a phone (I7, T7).
    press(&mut ui, "Jobs");
    press(&mut ui, "Abrir o último build apps/site");
    for (name, wide) in [("9-build-estreito", 1000u32), ("10-build-celular", 390u32)] {
        ui.set_viewport(Size::new(wide as f32, height as f32), 1.0);
        settle(&mut ui);
        let pixels = render_to_rgba(&gpu, ui.scene(), wide, height).expect("pixels");
        image::RgbaImage::from_raw(wide, height, pixels)
            .expect("image")
            .save(folder.join(format!("{name}.png")))
            .expect("saved");
    }
    let _ = std::fs::remove_dir_all(&home);
}

/// A client of the same server as `painel`, for a second panel that has not signed in.
fn painel_client(painel: &raven_ci_painel::Painel) -> raven_ci_painel::RemoteClient {
    painel.client()
}
