//! `CropOverlay`, `Protractor`, `RuleOfThirdsGrid` and `TransformHandles`, every feature: a crop
//! with its proportions, guides and limit of turn, straightened by a protractor that shares its
//! angle, the composition guides side by side, and the handles of a layer with each of their
//! rules. Every commit writes what it did in a line.

use ravenrs::prelude::*;

use super::{PHOTO, decimal, heading, note, photo, picture, shown_size};

/// The control's name, as the session knows it.
pub const ID: &str = "CropDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Recortar e girar";

/// The proportions of the crop, in the order of the choice.
pub const ASPECTS: [&str; 4] = ["Livre", "3:2", "1:1", "16:9"];
const ASPECT_VALUES: [Option<f32>; 4] = [None, Some(3.0 / 2.0), Some(1.0), Some(16.0 / 9.0)];

/// The lines inside the crop, in the order of the choice.
pub const GUIDES: [&str; 5] = ["Terços", "Proporção áurea", "Grade de 6", "Quadrados", "Nenhuma"];
const GUIDE_VALUES: [Option<CompositionGuide>; 5] = [
    Some(CompositionGuide::Thirds),
    Some(CompositionGuide::GoldenRatio),
    Some(CompositionGuide::Divisions(6)),
    Some(CompositionGuide::Squares(4)),
    None,
];

/// How far the crop can turn, in the order of the choice.
pub const LIMITS: [&str; 3] = ["Até 45°", "Até 15°", "Sem girar"];
const LIMIT_VALUES: [f32; 3] = [45.0, 15.0, 0.0];

/// What a crop is, the way the line says it.
fn told(area: CropArea) -> String {
    format!(
        "Recorte: {} × {} a {}°",
        area.rect.width().round(),
        area.rect.height().round(),
        decimal(area.angle, 1)
    )
}

/// A choice that is a switch on a wide screen and a list on a phone.
fn choice(narrow: bool, value: Binding<usize>, options: &'static [&'static str], label: &str) -> AnyView {
    if narrow {
        Box::new(Select::new(value, options.iter().copied()).label(label.to_string()))
    } else {
        Box::new(
            HStack::new((
                Text::new(label.to_string()),
                SegmentedControl::new(value, options.iter().copied()),
            ))
            .spacing(8.0)
            .align(Align::Center),
        )
    }
}

/// The page: the crop and its protractor, the guides, and the handles of a layer.
#[derive(Debug, Default)]
pub struct CropPage;

impl View for CropPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let shown = shown_size(cx, 480.0);
        let small = shown_size(cx, if narrow { 150.0 } else { 200.0 });
        let image = cx.use_state(|| photo(PHOTO.width as u32, PHOTO.height as u32, 0.2)).get();
        let said = cx.use_state(|| "Arraste uma alça do recorte ou gire por fora dele.".to_string());
        let crop = cx.use_state(|| CropArea::new(Rect::new(40.0, 30.0, 240.0, 140.0)));
        let aspect = cx.use_state(|| 0_usize);
        let guide = cx.use_state(|| 0_usize);
        let limit = cx.use_state(|| 0_usize);
        let fine = cx.use_state(|| true);
        let locked = cx.use_state(|| false);

        let (cropped, straightened) = (said.clone(), said.clone());
        let limit_degrees = LIMIT_VALUES[limit.get().min(2)];
        let overlay = CropOverlay::new(crop.binding())
            .aspect(ASPECT_VALUES[aspect.get().min(3)])
            .angle_limit(limit_degrees)
            .guide(GUIDE_VALUES[guide.get().min(4)])
            .turning_guide(if fine.get() { Some(CompositionGuide::Divisions(8)) } else { None })
            .min_size(32.0)
            .on_commit(move |area| cropped.set(told(area)))
            .disabled(locked.get())
            .label("Recorte da foto")
            .stage(picture(image.clone(), shown.width, shown.height, "Paisagem com sol"))
            .space(ContentSpace::Fit(PHOTO));
        // The protractor turns the same crop through its angle.
        let reach = if limit_degrees > 0.0 { limit_degrees } else { 45.0 };
        let protractor = Protractor::new(crop.binding().map(|c| c.angle, |c, angle| c.angle = angle))
            .range(-reach, reach)
            .step(0.5)
            .width(if narrow { shown.width } else { 320.0 })
            .on_commit(move |angle| straightened.set(format!("Endireitado em {}°", decimal(angle, 1))))
            .disabled(locked.get() || limit_degrees == 0.0)
            .label("Endireitar");
        let cropping = VStack::new((
            heading("Recortar e endireitar"),
            note("As alças mudam o tamanho, arrastar dentro move, arrastar fora gira. Com o teclado: setas, Alt e [ ]."),
            choice(narrow, aspect.binding(), &ASPECTS, "Proporção"),
            choice(narrow, guide.binding(), &GUIDES, "Linhas"),
            choice(narrow, limit.binding(), &LIMITS, "Giro"),
            HStack::new((
                Toggle::new(fine.binding()).label("Grade fina ao girar"),
                Toggle::new(locked.binding()).label("Travar o recorte"),
            ))
            .spacing(12.0)
            .wrap(),
            overlay,
            protractor,
            Text::new(told(crop.get())),
        ))
        .spacing(8.0)
        .align(Align::Start);

        // Each guide over the same picture, and one that follows the crop.
        let grid = |guide: CompositionGuide, caption: &str| {
            VStack::new((
                RuleOfThirdsGrid::new()
                    .guide(guide)
                    .stage(picture(image.clone(), small.width, small.height, caption))
                    .space(ContentSpace::Fit(PHOTO)),
                note(caption.to_string()),
            ))
            .spacing(4.0)
            .align(Align::Start)
        };
        let guides = VStack::new((
            heading("Guias de composição"),
            HStack::new((
                grid(CompositionGuide::Thirds, "Terços"),
                grid(CompositionGuide::GoldenRatio, "Proporção áurea"),
                grid(CompositionGuide::Divisions(4), "Quatro partes"),
                grid(CompositionGuide::Squares(3), "Três quadrados"),
                VStack::new((
                    RuleOfThirdsGrid::new()
                        .area(crop.get().rect)
                        .color(Color::hex(0xFACC15))
                        .stage(picture(image.clone(), small.width, small.height, "Terços do recorte"))
                        .space(ContentSpace::Fit(PHOTO)),
                    note("Terços só no recorte, em amarelo"),
                ))
                .spacing(4.0)
                .align(Align::Start),
            ))
            .spacing(12.0)
            .wrap(),
        ))
        .spacing(8.0)
        .align(Align::Start);

        // The handles of a layer, with each rule as a switch.
        let layer = cx.use_state(|| BoxTransform::new(Rect::new(90.0, 50.0, 120.0, 80.0)));
        let keep = cx.use_state(|| true);
        let square = cx.use_state(|| false);
        let confined = cx.use_state(|| true);
        let rotatable = cx.use_state(|| true);
        let outside = cx.use_state(|| false);
        let pivot = cx.use_state(|| true);
        let frozen = cx.use_state(|| false);
        let moved = said.clone();
        let now = layer.get();
        let layer_line = format!(
            "Camada: {} × {} em ({}, {}) a {}°",
            now.rect.width().round(),
            now.rect.height().round(),
            now.rect.x().round(),
            now.rect.y().round(),
            decimal(now.angle, 0)
        );
        let turned = said.clone();
        let turning = Protractor::new(layer.binding().map(|t| t.angle, |t, angle| t.angle = angle))
            .range(-180.0, 180.0)
            .step(15.0)
            .width(if narrow { shown.width } else { 320.0 })
            .on_commit(move |angle| turned.set(format!("Camada girada para {}°", decimal(angle, 0))))
            .label("Giro da camada");
        let handles = VStack::new((
            heading("Alças de uma camada"),
            note("Um canto mantém a proporção (Shift solta), Alt muda em volta do pivô, Shift gira de 15 em 15°."),
            HStack::new((
                Toggle::new(keep.binding()).label("Cantos mantêm a proporção"),
                Toggle::new(square.binding()).label("Sempre quadrada"),
                Toggle::new(confined.binding()).label("Dentro da foto"),
                Toggle::new(rotatable.binding()).label("Alça de giro"),
                Toggle::new(outside.binding()).label("Girar por fora"),
                Toggle::new(pivot.binding()).label("Pivô"),
                Toggle::new(frozen.binding()).label("Travar a camada"),
            ))
            .spacing(12.0)
            .wrap(),
            TransformHandles::new(layer.binding())
                .keep_proportions(keep.get())
                .aspect(square.get().then_some(1.0))
                .confined(confined.get())
                .min_size(16.0)
                .rotatable(rotatable.get())
                .rotate_outside(outside.get())
                .pivot_handle(pivot.get())
                .on_commit(move |t| {
                    moved.set(format!(
                        "Camada solta: {} × {} a {}°",
                        t.rect.width().round(),
                        t.rect.height().round(),
                        decimal(t.angle, 0)
                    ))
                })
                .disabled(frozen.get())
                .label("Camada do logotipo")
                .stage(picture(image, shown.width, shown.height, "Paisagem sob a camada"))
                .space(ContentSpace::Fit(PHOTO)),
            turning,
            Text::new(layer_line),
        ))
        .spacing(8.0)
        .align(Align::Start);

        cx.child(
            &VStack::new((cropping, guides, handles, Text::new(said.get())))
                .spacing(24.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
