//! Colors: picking, mixing and measuring them.
// ravenrs-lint: allow-file(raw_color) every example here is about a color: the values picked, offered or measured are the content

use ravenrs::prelude::*;

samples! {
    "ColorSwatch" => color_swatch, "ColorPicker" => color_picker, "SwatchGrid" => swatch_grid,
    "ColorPair" => color_pair, "Histogram" => histogram, "ColorWheel" => color_wheel,
    "CurveEditor" => curve_editor, "Eyedropper" => eyedropper,
}

// sample: ColorPicker, 420
fn color_picker() -> impl View {
    let color = State::new(Some(Color::hex(0x2563EB)));
    ColorPicker::new(color.binding())
        .alpha(true)
        .palette([
            (Color::hex(0x2563EB), "Blue"),
            (Color::hex(0x16A34A), "Green"),
            (Color::hex(0xDC2626), "Red"),
            (Color::hex(0xF59E0B), "Amber"),
        ])
        .label("Category color")
}
// end

// sample: ColorSwatch, 90
fn color_swatch() -> impl View {
    let current = State::new(0);
    view_fn(move || {
        let theme = Theme::raven_light();
        let options =
            [(Semantic::Accent, "Accent"), (Semantic::Success, "Success"), (Semantic::Danger, "Danger")];
        let swatches: Vec<AnyView> = options
            .iter()
            .enumerate()
            .map(|(i, (role, name))| {
                let pick = current.clone();
                Box::new(
                    ColorSwatch::new(theme.color(*role), *name)
                        .selected(current.get() == i)
                        .on_click(move || pick.set(i)),
                ) as AnyView
            })
            .collect();
        HStack::new(swatches)
    })
}
// end

// sample: Histogram, 130
fn histogram() -> impl View {
    let hill = |center: f32| -> Vec<f32> {
        (0..256).map(|i| 400.0 * (-((i as f32 - center) / 40.0).powi(2)).exp() + 10.0).collect()
    };
    Histogram::new([
        HistogramChannel::new("Red", Color::rgb(0.9, 0.15, 0.15), hill(90.0)),
        HistogramChannel::new("Green", Color::rgb(0.15, 0.8, 0.2), hill(128.0)),
        HistogramChannel::new("Blue", Color::rgb(0.2, 0.3, 0.95), hill(170.0)),
    ])
    .zones(5)
    .height(110.0)
}
// end

// sample: ColorWheel, 180
fn color_wheel() -> impl View {
    let tint = State::new(HueSaturation::new(30.0, 0.4));
    let shown = tint.clone();
    HStack::new((
        ColorWheel::new(tint.binding()).diameter(160.0).label("Shadows"),
        Text::bound(&shown, |v| format!("{:.0}°, {:.0}%", v.hue, v.saturation * 100.0))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted),
    ))
    .spacing(16.0)
}
// end

// sample: CurveEditor, 210
fn curve_editor() -> impl View {
    let points = State::new(vec![
        Point::new(0.0, 0.0),
        Point::new(0.3, 0.2),
        Point::new(0.7, 0.85),
        Point::new(1.0, 1.0),
    ]);
    let hill: Vec<f32> = (0..128).map(|i| 50.0 * (-((i as f32 - 60.0) / 30.0).powi(2)).exp()).collect();
    CurveEditor::new(points.binding())
        .height(190.0)
        .histogram([HistogramChannel::new("Light", Color::rgb(0.6, 0.6, 0.6), hill)], HistogramScale::Linear)
        .label("Tone curve")
}
// end

// sample: Eyedropper, 190
fn eyedropper() -> impl View {
    let shown = State::new(Color::rgb(0.5, 0.5, 0.5));
    let told = shown.clone();
    let dropper = EyedropperState::new().on_pick(move |color| told.set(color));
    let ramp = |x: f32, y: f32| Color::rgb(x / 300.0, y / 130.0, 0.5);
    let picture = Canvas::new(move |frame, scene| {
        for i in 0..30 {
            let x = i as f32 * 10.0;
            let band = Rect::new(frame.x() + x, frame.y(), 10.0, frame.height());
            scene.push_quad(Quad::new(band, ramp(x, 65.0)));
        }
    })
    .size(300.0, 130.0);
    let swatch = Canvas::new(move |frame, scene| {
        scene.push_quad(Quad::new(frame, shown.get()).radius(4.0));
    })
    .size(28.0, 28.0);
    VStack::new((
        HStack::new((Eyedropper::new(&dropper), swatch)).spacing(8.0),
        EyedropperArea::new(&dropper, move |at| Some(ramp(at.x, at.y))).stage(picture),
    ))
    .spacing(8.0)
}
// end

// sample: SwatchGrid, 160
fn swatch_grid() -> impl View {
    let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::hex(0x2563eb)));
    let (for_front, for_back, shown) = (front.clone(), back.clone(), front.clone());
    VStack::new((
        view_fn(move || {
            let (for_front, for_back) = (for_front.clone(), for_back.clone());
            SwatchGrid::new([
                (Color::hex(0xe11d48), "Rose"),
                (Color::hex(0xf59e0b), "Amber"),
                (Color::hex(0x16a34a), "Green"),
                (Color::hex(0x2563eb), "Blue"),
                (Color::hex(0x7c3aed), "Violet"),
                (Color::hex(0x111827), "Ink"),
            ])
            .columns(3)
            .selected(Some(shown.get()))
            .on_pick(move |color, held| {
                // With Cmd or Ctrl, the color goes to the background.
                if held.meta || held.ctrl { for_back.set(color) } else { for_front.set(color) }
            })
        }),
        Text::new("Click: foreground. Cmd or Ctrl and click: background.").style(TextStyle::Caption),
    ))
    .spacing(8.0)
}
// end

// sample: ColorPair, 90
fn color_pair() -> impl View {
    let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::WHITE));
    ColorPair::new(front.binding(), back.binding()).defaults(Color::BLACK, Color::WHITE)
}
// end
