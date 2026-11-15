//! `ColorWheel`, `CurveEditor`, `Histogram` and `Eyedropper`, every feature: three wheels that
//! tint the shadows, the midtones and the highlights, a tone curve per channel over the
//! histogram of the photo, the histogram itself in both scales, and an eyedropper that picks
//! from the photo. Lines say each tint, the middle of the curve and the color picked.

use ravenrs::prelude::*;

use super::{PHOTO, decimal, heading, landscape, note, photo, picture, shown_size};

/// The control's name, as the session knows it.
pub const ID: &str = "ToneDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Cor e tons";

/// The channels of the curve, in the order of the choice.
pub const CHANNELS: [&str; 4] = ["Luz", "Vermelho", "Verde", "Azul"];
/// The scales of the histogram, in the order of the switch.
pub const SCALES: [&str; 2] = ["Linear", "Logarítmica"];

const BINS: usize = 64;

/// The bins of the photo, worked out from the same landscape it is drawn from: red, green, blue
/// and the light.
fn bins() -> [Vec<u32>; 4] {
    let mut counts = [vec![0_u32; BINS], vec![0_u32; BINS], vec![0_u32; BINS], vec![0_u32; BINS]];
    let bin = |value: f32| ((value.clamp(0.0, 1.0) * (BINS - 1) as f32).round()) as usize;
    for y in 0..80 {
        for x in 0..128 {
            let c = landscape(x as f32 / 128.0, y as f32 / 80.0, 0.0);
            let light = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
            for (channel, value) in [c.r, c.g, c.b, light].into_iter().enumerate() {
                counts[channel][bin(value)] += 1;
            }
        }
    }
    counts
}

/// The histogram's channels: three filled and the light as a line.
fn channels() -> Vec<HistogramChannel> {
    let [red, green, blue, light] = bins();
    vec![
        HistogramChannel::new("Vermelho", Color::rgb(0.9, 0.2, 0.2), red),
        HistogramChannel::new("Verde", Color::rgb(0.2, 0.8, 0.3), green),
        HistogramChannel::new("Azul", Color::rgb(0.25, 0.4, 0.95), blue),
        HistogramChannel::new("Luz", Color::rgb(0.9, 0.9, 0.9), light).line(),
    ]
}

/// A color as `#RRGGBB`.
fn hex(color: Color) -> String {
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", byte(color.r), byte(color.g), byte(color.b))
}

/// A tint the way the line says it.
fn tint(name: &str, value: HueSaturation) -> String {
    format!("{name}: {}°, {}%", value.hue.round(), (value.saturation * 100.0).round())
}

/// The straight curve, which changes nothing.
fn straight() -> Vec<Point> {
    vec![Point::ZERO, Point::new(1.0, 1.0)]
}

/// The page: the wheels, the curve, the histogram and the eyedropper.
#[derive(Debug, Default)]
pub struct TonesPage;

impl View for TonesPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let shown = shown_size(cx, 400.0);
        let said = cx.use_state(|| "Arme o conta-gotas e clique na foto.".to_string());

        // Three wheels, each drawn at the brightness of what it tints.
        let shadows = cx.use_state(|| HueSaturation::new(220.0, 0.25));
        let midtones = cx.use_state(|| HueSaturation::new(0.0, 0.0));
        let highlights = cx.use_state(|| HueSaturation::new(40.0, 0.3));
        let locked = cx.use_state(|| false);
        let diameter = if narrow { 96.0 } else { 140.0 };
        let wheel = |value: &State<HueSaturation>, brightness: f32, name: &str| {
            VStack::new((
                ColorWheel::new(value.binding())
                    .diameter(diameter)
                    .brightness(brightness)
                    .disabled(locked.get())
                    .label(name.to_string()),
                note(tint(name, value.get())),
            ))
            .spacing(4.0)
            .align(Align::Center)
        };
        let wheels = VStack::new((
            heading("Rodas de cor"),
            note("Arraste ou use as setas: esquerda e direita giram o tom, cima e baixo mudam a saturação. Home tira a cor."),
            HStack::new((
                wheel(&shadows, 0.35, "Sombras"),
                wheel(&midtones, 0.7, "Meios-tons"),
                wheel(&highlights, 1.0, "Realces"),
            ))
            .spacing(16.0)
            .wrap(),
            Toggle::new(locked.binding()).label("Travar as rodas"),
        ))
        .spacing(8.0)
        .align(Align::Start);

        // The curve of one channel, over the histogram of the photo.
        let points = cx.use_state(|| vec![Point::ZERO, Point::new(0.5, 0.5), Point::new(1.0, 1.0)]);
        let channel = cx.use_state(|| 0_usize);
        let baseline = cx.use_state(|| true);
        let fine = cx.use_state(|| false);
        let channel_colors = [
            Color::rgb(0.92, 0.92, 0.92),
            Color::rgb(0.9, 0.25, 0.25),
            Color::rgb(0.25, 0.8, 0.35),
            Color::rgb(0.3, 0.45, 0.95),
        ];
        let picker: AnyView = if narrow {
            Box::new(Select::new(channel.binding(), CHANNELS).label("Canal"))
        } else {
            Box::new(SegmentedControl::new(channel.binding(), CHANNELS))
        };
        let (s_curve, flat) = (points.clone(), points.clone());
        let editor = CurveEditor::new(points.binding())
            .color(channel_colors[channel.get().min(3)])
            .baseline(baseline.get())
            .grid(if fine.get() { 8 } else { 4 })
            .histogram(channels(), HistogramScale::Linear)
            .disabled(locked.get())
            .label("Curva de tons");
        let editor = if narrow { editor.size(shown.width, 200.0) } else { editor.height(240.0) };
        let middle = CurveEditor::value_at(&points.get(), 0.5);
        let curve = VStack::new((
            heading("Curva de tons"),
            note("Clique para pôr um ponto e arraste. Clique duplo ou Delete tira; PageUp e PageDown trocam o ponto; Enter põe um no meio."),
            picker,
            HStack::new((
                Button::new("Contraste em S").on_click(move || {
                    s_curve.set(vec![
                        Point::ZERO,
                        Point::new(0.25, 0.15),
                        Point::new(0.75, 0.85),
                        Point::new(1.0, 1.0),
                    ])
                }),
                Button::new("Linha reta").on_click(move || flat.set(straight())),
                Toggle::new(baseline.binding()).label("Linha de referência"),
                Toggle::new(fine.binding()).label("Grade fina"),
            ))
            .spacing(8.0)
            .wrap(),
            editor,
            Text::new(format!(
                "{} · {} pontos · o meio sai em {}",
                CHANNELS[channel.get().min(3)],
                points.get().len(),
                decimal(middle, 2)
            )),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // The histogram, in both scales, with zones and the marks of what piles up at the ends.
        let scale = cx.use_state(|| 0_usize);
        let zoned = cx.use_state(|| true);
        let scaled = if scale.get() == 0 { HistogramScale::Linear } else { HistogramScale::Logarithmic };
        let scales: AnyView = if narrow {
            Box::new(Select::new(scale.binding(), SCALES).label("Escala"))
        } else {
            Box::new(SegmentedControl::new(scale.binding(), SCALES))
        };
        let histogram = VStack::new((
            heading("Histograma"),
            HStack::new((scales, Toggle::new(zoned.binding()).label("Zonas"))).spacing(12.0).wrap(),
            Histogram::new(channels())
                .scale(scaled)
                .clip_share(0.01)
                .zones(if zoned.get() { 5 } else { 0 })
                .height(if narrow { 96.0 } else { 140.0 })
                .label("Histograma da foto"),
            note("Uma marca na ponta acende quando mais de 1% de um canal se acumula ali."),
            Histogram::new(channels().into_iter().take(1))
                .scale(scaled)
                .size(if narrow { 160.0 } else { 240.0 }, 60.0)
                .label("Só o vermelho"),
        ))
        .spacing(8.0)
        .align(Align::Stretch);

        // The eyedropper: buttons that arm it, and the photo it picks from.
        let dropper = cx
            .use_state(|| {
                let line = said.clone();
                EyedropperState::new().on_pick(move |color| line.set(format!("Cor pegada: {}", hex(color))))
            })
            .get();
        let image = cx.use_state(|| photo(PHOTO.width as u32, PHOTO.height as u32, 0.0)).get();
        let armed = dropper.is_armed();
        let picked = dropper.picked().map_or("nenhuma".to_string(), hex);
        let eyedropper = VStack::new((
            heading("Conta-gotas"),
            HStack::new((
                Eyedropper::new(&dropper)
                    .label("Conta-gotas")
                    .hint("Arme e clique num ponto da foto")
                    .size(ButtonSize::Medium),
                Eyedropper::new(&dropper)
                    .icon(IconName::MousePointer)
                    .size(ButtonSize::Small)
                    .label("Conta-gotas pequeno"),
                Eyedropper::new(&dropper).disabled(true).label("Conta-gotas desligado"),
            ))
            .spacing(8.0)
            .align(Align::Center),
            EyedropperArea::new(&dropper, |at| Some(landscape(at.x / PHOTO.width, at.y / PHOTO.height, 0.0)))
                .label("Foto para pegar a cor")
                .stage(picture(image, shown.width, shown.height, "Paisagem para pegar a cor"))
                .space(ContentSpace::Fit(PHOTO)),
            Text::new(format!(
                "{} · última cor: {picked}",
                if armed { "Armado: clique na foto" } else { "Desarmado" }
            )),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((eyedropper, wheels, curve, histogram, Text::new(said.get())))
                .spacing(24.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
