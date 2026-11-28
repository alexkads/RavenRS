//! Pictures of the designer in each theme and as the system changes, for the eye (#290):
//! `RAVEN_RD_TELAS=<pasta> cargo test -p raven-report-designer --test suite telas -- --ignored`.

use raven_report_designer::Designer;
use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, render_to_rgba, wgpu};

#[test]
#[ignore = "draws pictures into the folder given"]
fn pictures_of_the_designer_in_each_theme_and_as_the_system_changes() {
    let folder = std::path::PathBuf::from(std::env::var_os("RAVEN_RD_TELAS").expect("a folder"));
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(GpuContext::new(&instance, None)).expect("a GPU");
    let (width, height) = (1440, 900);
    let mut ui = Ui::new(Designer::new(), Theme::raven_dark());
    ui.set_viewport(Size::new(width as f32, height as f32), 1.0);
    let snap = |ui: &mut Ui, name: &str| {
        for _ in 0..6 {
            ui.update();
        }
        let pixels = render_to_rgba(&gpu, ui.scene(), width, height).expect("pixels");
        image::RgbaImage::from_raw(width, height, pixels)
            .expect("image")
            .save(folder.join(format!("{name}.png")))
            .expect("saved");
    };
    for (theme, name) in [
        (Theme::raven_dark(), "1-escuro"),
        (Theme::raven_light(), "2-claro"),
        (Theme::raven_glass(), "3-vidro"),
    ] {
        ui.set_theme(theme);
        snap(&mut ui, name);
        assert_eq!(ui.theme().name, theme.name);
    }
    // Following the system: the open designer changes with it, with nothing reopened.
    ui.follow_system_theme(true);
    ui.set_system_dark(true);
    snap(&mut ui, "4-sistema-escuro");
    assert_eq!(ui.theme().name, Theme::raven_dark().name);
    ui.set_system_dark(false);
    snap(&mut ui, "5-sistema-claro");
    assert_eq!(ui.theme().name, Theme::raven_light().name);
}
