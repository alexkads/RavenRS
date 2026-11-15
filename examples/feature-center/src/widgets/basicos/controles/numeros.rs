//! `Slider`, `Stepper`, `ZoomControl` and `Pagination`, every feature: volume, a balance that rests
//! in the middle, a color temperature painted on the track, chapters marked on a video, seats on a
//! booking, the zoom of a picture and the pages of a list. A line sums up every value.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "NumbersDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Números e faixas";

/// How many pages the list has.
pub const PAGES_IN_LIST: usize = 12;

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// The page: sliders, steppers, the zoom and the pages, and a line with every value.
#[derive(Debug, Default)]
pub struct NumbersPage;

impl View for NumbersPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let volume = cx.use_state(|| 40.0_f32);
        let balance = cx.use_state(|| 0.0_f32);
        let warmth = cx.use_state(|| 5500.0_f32);
        let time = cx.use_state(|| 0.25_f32);
        let seats = cx.use_state(|| 2_i32);
        let fixed = cx.use_state(|| 4_i32);
        let zoom = cx.use_state(|| ZoomView::at(1.0));
        let page = cx.use_state(|| 0_usize);
        let other = cx.use_state(|| 3_usize);
        let width = if narrow { 220.0 } else { 320.0 };

        let sliders = VStack::new((
            Slider::new(volume.binding())
                .range(0.0, 100.0)
                .step(5.0)
                .width(width)
                .label("Volume")
                .show_value(),
            Slider::new(balance.binding())
                .range(-50.0, 50.0)
                .step(1.0)
                .neutral(0.0)
                .width(width)
                .label("Equilíbrio")
                .value_format(|v| match v {
                    v if v < 0.0 => format!("{:.0} à esquerda", -v),
                    v if v > 0.0 => format!("{v:.0} à direita"),
                    _ => "no meio".to_string(),
                }),
            Slider::new(warmth.binding())
                .range(2000.0, 9000.0)
                .step(100.0)
                .track_colors([Color::hex(0xF59E0B), Color::hex(0xFFFFFF), Color::hex(0x60A5FA)])
                .width(width)
                .label("Temperatura da cor")
                .value_format(|v| format!("{v:.0} K")),
            Slider::new(time.binding())
                .marks([0.2, 0.45, 0.8])
                .label("Posição do vídeo")
                .value_format(|v| format!("{:.0}%", v * 100.0)),
        ))
        .spacing(12.0)
        .align(Align::Stretch);

        let steppers = HStack::new((
            Stepper::new(seats.binding()).range(1, 9).label("Lugares"),
            Stepper::new(fixed.binding()).label("Lugares do plano (só leitura)").read_only(true),
            Stepper::new(fixed.binding()).label("Mesas extras (desligado)").disabled(true),
        ))
        .spacing(16.0)
        .wrap()
        .align(Align::Center);

        let zooms = HStack::new((
            ZoomControl::new(zoom.binding()).range(0.25, 4.0).label("Zoom da foto"),
            ZoomControl::new(zoom.binding())
                .range(0.25, 4.0)
                .size(ButtonSize::Medium)
                .label("Zoom da foto, maior"),
        ))
        .spacing(16.0)
        .wrap();
        let scale = zoom.get();
        let zoom_line = if scale.fit {
            "Zoom: cabe na tela".to_string()
        } else {
            format!("Zoom: {:.0}%", scale.scale * 100.0)
        };

        let pages = VStack::new((
            Pagination::new(page.binding(), PAGES_IN_LIST),
            Pagination::new(other.binding(), PAGES_IN_LIST).compact().labels("Voltar", "Avançar", "Folha"),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let summary = format!(
            "Volume {:.0} · equilíbrio {:.0} · {:.0} K · vídeo em {:.0}% · {} lugares · página {} de {PAGES_IN_LIST} · folha {}",
            volume.get(),
            balance.get(),
            warmth.get(),
            time.get() * 100.0,
            seats.get(),
            page.get() + 1,
            other.get() + 1,
        );

        cx.child(
            &VStack::new((
                VStack::new((heading("Faixas"), sliders)).spacing(8.0).align(Align::Stretch),
                VStack::new((heading("Quantidades"), steppers)).spacing(8.0),
                VStack::new((heading("Zoom"), zooms, Text::new(zoom_line))).spacing(8.0),
                VStack::new((heading("Páginas"), pages)).spacing(8.0),
                Text::new(summary),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
