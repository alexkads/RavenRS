//! `RubberBand`, `Marquee` and `Ruler`, every feature: a selection drawn with a drag or the keys
//! that stays or goes, the marching ants around a rectangle, a lasso and the edges of a mask,
//! and rulers that follow a zoom and a pan, with a mark where the selection starts. Lines say the
//! selection and the scale.

use ravenrs::prelude::*;

use super::{PHOTO, decimal, heading, note, photo, picture, shown_size};

/// The control's name, as the session knows it.
pub const ID: &str = "SelectionDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Seleção e medida";

/// A selection the way the line says it.
fn told(rect: Rect) -> String {
    format!(
        "{} × {} em ({}, {})",
        rect.width().round(),
        rect.height().round(),
        rect.x().round(),
        rect.y().round()
    )
}

/// A lasso around the sun, as a closed path in the units of the photo.
fn lasso() -> Vec<Point> {
    (0..24)
        .map(|i| {
            let turn = i as f32 / 24.0 * std::f32::consts::TAU;
            let wobble = 1.0 + 0.15 * (turn * 3.0).sin();
            Point::new(230.0 + 42.0 * wobble * turn.cos(), 52.0 + 30.0 * wobble * turn.sin())
        })
        .collect()
}

/// The edges of a mask of two squares, given loose and out of order, as a mask hands them.
fn mask_edges() -> Vec<(Point, Point)> {
    let square = |x: f32, y: f32, side: f32| {
        let (a, b, c, d) = (
            Point::new(x, y),
            Point::new(x + side, y),
            Point::new(x + side, y + side),
            Point::new(x, y + side),
        );
        [(c, d), (a, b), (d, a), (b, c)]
    };
    square(30.0, 120.0, 60.0).into_iter().chain(square(200.0, 110.0, 70.0)).collect()
}

/// The page: the band over a photo, the three outlines and the rulers.
#[derive(Debug, Default)]
pub struct SelectionPage;

impl View for SelectionPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let shown = shown_size(cx, 480.0);
        let small = shown_size(cx, if narrow { 150.0 } else { 220.0 });
        let image = cx.use_state(|| photo(PHOTO.width as u32, PHOTO.height as u32, 0.3)).get();
        let said = cx.use_state(|| "Arraste sobre a foto para selecionar.".to_string());
        let selection = cx.use_state(|| None::<Rect>);
        let keep = cx.use_state(|| true);
        let off = cx.use_state(|| false);
        let ants = cx.use_state(|| true);

        let chosen = said.clone();
        let band = RubberBand::new(selection.binding())
            .keep(keep.get())
            .disabled(off.get())
            .on_select(move |rect| chosen.set(format!("Selecionado: {}", told(rect))))
            .label("Seleção na foto")
            .stage(picture(image.clone(), shown.width, shown.height, "Paisagem para selecionar"))
            .space(ContentSpace::Fit(PHOTO));
        let current = selection.get();
        let selecting = VStack::new((
            heading("Selecionar com o arrasto"),
            note("Com o teclado: as setas movem, com Alt mudam o tamanho, Enter seleciona, Esc apaga."),
            HStack::new((
                Toggle::new(keep.binding()).label("Manter depois de soltar"),
                Toggle::new(off.binding()).label("Desligar a seleção"),
            ))
            .spacing(12.0)
            .wrap(),
            band,
            Text::new(match current {
                Some(rect) => format!("Seleção: {}", told(rect)),
                None => "Nada selecionado".to_string(),
            }),
        ))
        .spacing(8.0)
        .align(Align::Start);

        // The marching ants: around the selection, a lasso and the edges of a mask.
        let around = current.unwrap_or(Rect::new(60.0, 40.0, 140.0, 90.0));
        let outlined = |caption: &str| picture(image.clone(), small.width, small.height, caption);
        let outlines = VStack::new((
            heading("Contornos que andam"),
            Toggle::new(ants.binding()).label("Formigas andando"),
            HStack::new((
                VStack::new((
                    Marquee::rect(around)
                        .animated(ants.get())
                        .label("Contorno da seleção")
                        .stage(outlined("Foto com a seleção"))
                        .space(ContentSpace::Fit(PHOTO)),
                    note("Retângulo (segue a seleção)"),
                ))
                .spacing(4.0),
                VStack::new((
                    Marquee::path(lasso())
                        .animated(ants.get())
                        .label("Laço em volta do sol")
                        .stage(outlined("Foto com o laço"))
                        .space(ContentSpace::Fit(PHOTO)),
                    note("Laço"),
                ))
                .spacing(4.0),
                VStack::new((
                    Marquee::segments(mask_edges())
                        .animated(ants.get())
                        .label("Borda da máscara")
                        .stage(outlined("Foto com a máscara"))
                        .space(ContentSpace::Fit(PHOTO)),
                    note("Bordas de uma máscara"),
                ))
                .spacing(4.0),
            ))
            .spacing(12.0)
            .wrap(),
        ))
        .spacing(8.0)
        .align(Align::Start);

        // Rulers in pixels of the photo, following a zoom and a pan.
        let scale = cx.use_state(|| 1.5_f32);
        let origin = cx.use_state(|| 0.0_f32);
        let (closer, farther, right, back) = (scale.clone(), scale.clone(), origin.clone(), origin.clone());
        let marker = around.x();
        let ruler_width = shown.width.min(480.0);
        let rulers = VStack::new((
            heading("Réguas"),
            HStack::new((
                Button::new("Aproximar").on_click(move || closer.update(|s| *s = (*s * 2.0).min(16.0))),
                Button::new("Afastar").on_click(move || farther.update(|s| *s = (*s / 2.0).max(0.25))),
                Button::new("Mover para a direita").on_click(move || right.update(|o| *o += 50.0)),
                Button::new("Voltar ao início").on_click(move || back.set(0.0)),
            ))
            .spacing(8.0)
            .wrap(),
            VStack::new((
                Ruler::horizontal()
                    .scale(scale.get())
                    .origin(origin.get())
                    .marker(marker)
                    .label("Régua de cima"),
                HStack::new((
                    Ruler::vertical()
                        .scale(scale.get())
                        .origin(0.0)
                        .marker(around.y())
                        .thickness(24.0)
                        .min_spacing(40.0)
                        .whole_units()
                        .label("Régua do lado"),
                    picture(image.clone(), ruler_width, ruler_width * 0.45, "Área medida"),
                ))
                .spacing(0.0)
                .align(Align::Stretch),
            ))
            .spacing(0.0)
            .align(Align::Stretch),
            Text::new(format!(
                "Escala: {} px por pixel da foto · começa em {}",
                decimal(scale.get(), 2),
                origin.get().round()
            )),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((selecting, outlines, rulers, Text::new(said.get())))
                .spacing(24.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
