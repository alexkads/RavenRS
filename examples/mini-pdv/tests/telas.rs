//! Pictures of the screens, drawn with no window, to look at them next to the design. It only runs
//! when asked: `RAVEN_PDV_TELAS=pasta cargo test -p mini-pdv --test telas -- --ignored`.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use mini_pdv::app_com_pos;
use ravenrs::render::{GpuContext, render_to_rgba, wgpu};
use ravenrs_app_pos::Tela;
use ravenrs_core::{Event, Key, Modifiers, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

#[test]
#[ignore = "draws pictures into the folder given"]
fn pictures_of_the_screens() {
    let folder = std::path::PathBuf::from(std::env::var_os("RAVEN_PDV_TELAS").expect("a folder"));
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None)).expect("a GPU");
    let (view, pos) = app_com_pos(None).expect("it builds");
    let mut ui = Ui::new(view, Theme::raven_light());
    let (width, height) = (1366, 768);
    ui.set_viewport(Size::new(width as f32, height as f32), 1.0);
    let settle = |ui: &mut Ui| {
        for _ in 0..6 {
            ui.update();
        }
    };
    let press = |ui: &mut Ui, key: Key| {
        ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
        for _ in 0..4 {
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
    snap(&mut ui, "1-inicio");
    press(&mut ui, Key::Function(2));
    for code in ["7891000053508", "2*7891000244104", "3*7891000100103"] {
        for letter in code.chars() {
            press(&mut ui, Key::Character(letter.to_string()));
        }
        press(&mut ui, Key::Enter);
    }
    snap(&mut ui, "2-caixa");
    press(&mut ui, Key::Function(9));
    pos.caixa().recebido.set("100,00".to_owned());
    snap(&mut ui, "3-pagamento");
    press(&mut ui, Key::Function(9));
    for _ in 0..2000 {
        settle(&mut ui);
        if !matches!(*pos.caixa().tela.peek(), Tela::Enviando) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    snap(&mut ui, "4-resultado");
    press(&mut ui, Key::Function(9));
    press(&mut ui, Key::Escape);
    press(&mut ui, Key::Function(3));
    snap(&mut ui, "5-produtos");
    press(&mut ui, Key::Escape);
    press(&mut ui, Key::Function(4));
    snap(&mut ui, "6-etiquetas");
}
