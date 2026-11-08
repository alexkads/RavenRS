//! The images and icons pages.

use ravenrs::prelude::*;

use crate::{Gallery, columns, demo, page_header, sample_demos};

impl Gallery {
    pub(crate) fn images(&self, width: f32) -> impl View + use<> {
        let [aurora, dunes, ocean, bloom] = [0, 1, 2, 3].map(|i| self.images[i].clone());
        let card = |image: ImageData, title: &'static str, caption: &'static str| -> AnyView {
            Box::new(
                Card::new(
                    VStack::new((
                        Image::new(image).stretch().height(170.0).corner_radius(12.0),
                        Text::new(title).style(TextStyle::Headline),
                        Text::new(caption).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                    ))
                    .spacing(12.0)
                    .align(Align::Stretch),
                )
                .padding(12.0)
                .corner_radius(18.0),
            )
        };
        let mode = |mode: ContentMode, label: &'static str, image: ImageData| -> AnyView {
            Box::new(
                VStack::new((
                    Image::new(image)
                        .content_mode(mode)
                        .size(180.0, 130.0)
                        .corner_radius(10.0)
                        .background(Semantic::Control)
                        .corner_radius(10.0),
                    Text::new(label).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                ))
                .spacing(8.0),
            )
        };
        VStack::new((
            page_header(
                "Images",
                "Textures with mipmaps, rounded corners and cover, fit or fill. PNG and JPEG decode with Image::from_bytes; these are generated in Rust.",
            ),
            sample_demos(&[
                // imaging widgets
                ("Filmstrip", "A strip of thumbnails of any length, whose cells come from the app."),
                ("PagedDocumentView", "A bar with page and zoom over pages that are built only when they show."),
                ("ImageLightbox", "A picture over the whole app, with zoom, arrows, caption and counter."),
                ("CompareView", "Two pictures side by side or across a divider you drag."),
                ("Eyedropper", "A button that arms, takes the color you supply for the point clicked, and disarms."),
                ("CropOverlay", "A crop with a veil, a fixed proportion and an angle, which hands back geometry."),
                ("Protractor", "A band of ticks that sets an angle, with a double click back to zero."),
                ("TransformHandles", "Eight handles, a turn and a pivot, which hand back geometry."),
                ("Marquee", "The animated outline of a selection, which stops with reduced motion."),
                ("RubberBand", "A rectangle drawn with a drag, selected on release."),
                ("RuleOfThirdsGrid", "Thirds, a fine grid, squares or the golden ratio over a picture."),
                ("Minimap", "A small copy with a rectangle around what is in view, which moves the view."),
                ("ZoomPanView", "Zoom and pan over an image or drawing: pinch, wheel, drag, double click and keys."),
                ("ZoomControl", "Fit, 1:1, the percentage and the buttons that zoom in and out."),
                ("CurveEditor", "Points you drag over a smooth curve, with the keyboard too."),
                ("ColorWheel", "Hue and saturation on a disc, by a drag or the arrow keys."),
                ("Ruler", "A ruler in your units, with a round step for any zoom."),
                ("Histogram", "Overlapping channels from ready-made bins, with marks when an end clips."),
            ]),
            Grid::new(
                columns(width, 4),
                vec![
                    card(aurora.clone(), "Aurora", "Sine bands over a night gradient"),
                    card(dunes.clone(), "Dunes", "Layered curves at sunset"),
                    card(ocean.clone(), "Ocean", "Interfering waves"),
                    card(bloom.clone(), "Bloom", "A radial color field"),
                ],
            ),
            demo(
                "Content modes",
                "The same wide image in a 180 × 130 frame.",
                HStack::new((
                    mode(ContentMode::Cover, "Cover: fills and crops", dunes.clone()),
                    mode(ContentMode::Fit, "Fit: shows everything", dunes.clone()),
                    mode(ContentMode::Fill, "Fill: stretches", dunes),
                ))
                .spacing(20.0)
                .wrap(),
            ),
            demo(
                "Shapes and opacity",
                "Any corner radius, and opacity for disabled or decorative images.",
                HStack::new((
                    Image::new(bloom.clone()).size(64.0, 64.0).corner_radius(32.0),
                    Image::new(ocean.clone()).size(64.0, 64.0).corner_radius(16.0),
                    Image::new(aurora.clone()).size(64.0, 64.0),
                    Image::new(bloom).size(64.0, 64.0).corner_radius(32.0).opacity(0.4),
                ))
                .spacing(16.0)
                .align(Align::Center)
                .wrap(),
            ),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }

    pub(crate) fn icons(&self, width: f32) -> impl View + use<> {
        let tiles: Vec<AnyView> = IconName::ALL
            .iter()
            .map(|icon| -> AnyView {
                Box::new(
                    VStack::new((
                        Icon::new(*icon).size(22.0),
                        Text::new(icon.name()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                    ))
                    .spacing(12.0)
                    .align(Align::Center)
                    .padding((16.0, 8.0))
                    .background(Semantic::Surface)
                    .corner_radius(12.0)
                    .border(1.0, Semantic::Border),
                )
            })
            .collect();
        VStack::new((
            page_header(
                "Icons",
                &format!("{} Lucide icons (ISC license) built in, drawn as font glyphs so they take any size and color.", IconName::ALL.len()),
            ),
            Grid::new(columns(width, 6), tiles).spacing(12.0),
        ))
        .spacing(24.0)
        .align(Align::Stretch)
    }
}
