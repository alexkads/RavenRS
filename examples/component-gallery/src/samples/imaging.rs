//! Tools for working on a picture.

use ravenrs::prelude::*;

samples! {
    "NinePointAnchor" => nine_point_anchor, "Ruler" => ruler, "ZoomControl" => zoom_control,
    "ZoomPanView" => zoom_pan_view, "Minimap" => minimap, "RuleOfThirdsGrid" => rule_of_thirds_grid,
    "RubberBand" => rubber_band, "Marquee" => marquee, "TransformHandles" => transform_handles,
    "Protractor" => protractor, "CropOverlay" => crop_overlay,
}

// sample: Ruler, 70
fn ruler() -> impl View {
    VStack::new((
        Ruler::horizontal().scale(2.0).origin(-20.0).marker(60.0),
        Ruler::horizontal().scale(0.2).whole_units(),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
// end

// sample: ZoomControl, 60
fn zoom_control() -> impl View {
    let view = State::new(ZoomView::at(1.5));
    ZoomControl::new(view.binding())
}
// end

// sample: ZoomPanView, 260
fn zoom_pan_view() -> impl View {
    let view = State::new(ZoomView::fit());
    let content = Size::new(1200.0, 800.0);
    let picture = |frame: ZoomPanPaint, scene: &mut Scene| {
        // ravenrs-lint: allow(raw_color) the stand-in photograph being zoomed: its colors are the content
        scene.push_quad(Quad::new(frame.placed, Color::rgb(0.16, 0.3, 0.5)));
        let side = 120.0 * frame.scale;
        // ravenrs-lint: allow(raw_color) the stand-in photograph being zoomed: its colors are the content
        for (i, color) in [Color::rgb(0.9, 0.45, 0.3), Color::rgb(0.95, 0.8, 0.3)].into_iter().enumerate() {
            let x = frame.placed.x() + frame.placed.width() * (0.2 + 0.4 * i as f32);
            let y = frame.placed.y() + frame.placed.height() * 0.35;
            scene.push_quad(Quad::new(Rect::new(x, y, side, side), color).radius(side / 2.0));
        }
    };
    VStack::new((
        ZoomControl::new(view.binding()),
        ZoomPanView::custom(view.binding(), content, picture).size(420.0, 200.0).label("Sample picture"),
    ))
    .spacing(8.0)
}
// end

// sample: Minimap, 170
fn minimap() -> impl View {
    let view = State::new(ZoomView::at(1.0));
    let picture = |frame: ZoomPanPaint, scene: &mut Scene| {
        // ravenrs-lint: allow(raw_color) the stand-in photograph the minimap shows: its colors are the content
        scene.push_quad(Quad::new(frame.placed, Color::rgb(0.2, 0.45, 0.7)));
        let sun = Rect::new(
            frame.placed.x() + frame.placed.width() * 0.6,
            frame.placed.y() + frame.placed.height() * 0.2,
            80.0 * frame.scale,
            80.0 * frame.scale,
        );
        // ravenrs-lint: allow(raw_color) the stand-in photograph the minimap shows: its colors are the content
        scene.push_quad(Quad::new(sun, Color::rgb(0.95, 0.8, 0.3)).radius(40.0 * frame.scale));
    };
    let content = Size::new(800.0, 500.0);
    HStack::new((
        Minimap::custom(view.binding(), content, picture),
        ZoomPanView::custom(view.binding(), content, picture).size(240.0, 150.0),
    ))
    .spacing(12.0)
}
// end

// sample: RuleOfThirdsGrid, 190
fn rule_of_thirds_grid() -> impl View {
    let picture = Canvas::new(|frame, scene| {
        // ravenrs-lint: allow(raw_color) the sky of the stand-in photograph the grid is laid over
        let sky = LinearGradient::vertical(Color::rgb(0.3, 0.45, 0.7), Color::rgb(0.85, 0.6, 0.45));
        scene.push_quad(Quad::new(frame, sky));
    })
    .size(300.0, 170.0);
    RuleOfThirdsGrid::new().guide(CompositionGuide::GoldenRatio).stage(picture)
}
// end

// sample: RubberBand, 190
fn rubber_band() -> impl View {
    let selection = State::new(None::<Rect>);
    let chosen = State::new(String::from("Drag over the box"));
    let told = chosen.clone();
    VStack::new((
        Text::bound(&chosen, Clone::clone),
        RubberBand::new(selection.binding())
            .on_select(move |r| told.set(format!("{:.0} × {:.0}", r.width(), r.height())))
            .size(320.0, 130.0),
    ))
    .spacing(8.0)
}
// end

// sample: Marquee, 170
fn marquee() -> impl View {
    let lasso =
        [Point::new(20.0, 30.0), Point::new(120.0, 40.0), Point::new(100.0, 120.0), Point::new(30.0, 110.0)];
    HStack::new((
        Marquee::rect(Rect::new(20.0, 20.0, 110.0, 100.0)).size(150.0, 140.0),
        Marquee::path(lasso).size(150.0, 140.0),
    ))
    .spacing(12.0)
}
// end

// sample: TransformHandles, 240
fn transform_handles() -> impl View {
    let layer = State::new(BoxTransform::new(Rect::new(110.0, 70.0, 180.0, 100.0)).rotated(-8.0));
    let shown = layer.clone();
    let picture = Canvas::new(move |frame, scene| {
        let p = shown.get();
        let at = p.rect.translate(frame.origin);
        // ravenrs-lint: allow(raw_color) the stand-in canvas behind the layer being transformed
        scene.push_quad(Quad::new(frame, Color::rgb(0.2, 0.27, 0.35)));
        // ravenrs-lint: allow(raw_color) the layer being transformed, as the user painted it
        scene.push_quad(Quad::new(at, Color::rgb(0.3, 0.55, 0.75).with_alpha(0.6)));
    })
    .size(420.0, 230.0);
    TransformHandles::new(layer.binding()).stage(picture)
}
// end

// sample: Protractor, 70
fn protractor() -> impl View {
    let angle = State::new(7.5_f32);
    Protractor::new(angle.binding()).label("Straighten")
}
// end

// sample: CropOverlay, 300
fn crop_overlay() -> impl View {
    let crop = State::new(CropArea::new(Rect::new(70.0, 30.0, 240.0, 150.0)));
    let picture = Canvas::new(|frame, scene| {
        // ravenrs-lint: allow(raw_color) the sky of the stand-in photograph being cropped
        let sky = LinearGradient::vertical(Color::rgb(0.3, 0.45, 0.7), Color::rgb(0.85, 0.6, 0.45));
        scene.push_quad(Quad::new(frame, sky));
    })
    .size(400.0, 220.0);
    VStack::new((
        CropOverlay::new(crop.binding()).stage(picture),
        Protractor::new(crop.binding().map(|c| c.angle, |c, angle| c.angle = angle)).width(400.0),
    ))
    .spacing(8.0)
}
// end

// sample: NinePointAnchor, 70
fn nine_point_anchor() -> impl View {
    let point = State::new(AnchorPoint::Center);
    NinePointAnchor::new(point.binding()).cell_size(12.0)
}
// end
