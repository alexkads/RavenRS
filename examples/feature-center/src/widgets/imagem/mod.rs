//! The family `ravenrs-widgets-imaging`, group "Imagem e animação": viewing, cropping, selecting,
//! measuring and grading pictures, and the Lottie player. The pictures are drawn in code, so the
//! pages need no files.

pub mod animacao;
pub mod recorte;
pub mod selecao;
pub mod tons;
pub mod ver;

use ravenrs::prelude::*;

use super::Page;

/// The group of the navigation for this family.
pub const GROUP: &str = "Imagem e animação";

/// The pages of this family, in the order of the navigation.
pub const PAGES: &[Page] = &[
    Page {
        id: ver::ID,
        caption: ver::PAGE,
        group: GROUP,
        icon: IconName::Image,
        make: || Box::new(ver::ImageViewingPage),
    },
    Page {
        id: recorte::ID,
        caption: recorte::PAGE,
        group: GROUP,
        icon: IconName::Scissors,
        make: || Box::new(recorte::CropPage),
    },
    Page {
        id: selecao::ID,
        caption: selecao::PAGE,
        group: GROUP,
        icon: IconName::Ruler,
        make: || Box::new(selecao::SelectionPage),
    },
    Page {
        id: tons::ID,
        caption: tons::PAGE,
        group: GROUP,
        icon: IconName::Palette,
        make: || Box::new(tons::TonesPage),
    },
    Page {
        id: animacao::ID,
        caption: animacao::PAGE,
        group: GROUP,
        icon: IconName::Play,
        make: || Box::new(animacao::AnimationPage),
    },
];

/// The size of the pictures of these pages, in their own pixels: small, as they are made in memory.
pub const PHOTO: Size = Size::new(320.0, 200.0);

/// The color of a landscape at a point, from 0 to 1 on each axis: a sky, a sun and hills.
/// `warmth` shifts it toward a sunset, for a second version of the same picture.
pub fn landscape(u: f32, v: f32, warmth: f32) -> Color {
    let horizon = 0.58 + 0.07 * (u * 7.0).sin() + 0.04 * (u * 17.0).cos();
    let sun = (u - 0.72).hypot((v - 0.26) * 1.6);
    let color = if sun < 0.07 {
        Color::rgb(1.0, 0.86 - warmth * 0.2, 0.45 - warmth * 0.2)
    } else if v < horizon {
        let glow = (0.25 - sun).max(0.0) * 1.4;
        Color::rgb(
            0.32 + 0.35 * v + warmth * 0.45 + glow,
            0.52 + 0.25 * v + glow * 0.6,
            0.88 - 0.25 * v - warmth * 0.45,
        )
    } else {
        let depth = (v - horizon) / (1.0 - horizon).max(0.01);
        let rows = if ((u * 40.0 + depth * 9.0) as i32) % 2 == 0 { 0.04 } else { 0.0 };
        Color::rgb(0.18 + warmth * 0.25 + rows, 0.5 - 0.28 * depth + rows, 0.16 + 0.05 * depth)
    };
    Color::rgb(color.r.clamp(0.0, 1.0), color.g.clamp(0.0, 1.0), color.b.clamp(0.0, 1.0))
}

/// The landscape as a picture of this many pixels.
pub fn photo(width: u32, height: u32, warmth: f32) -> Option<ImageData> {
    ImageData::from_fn(width, height, move |x, y| {
        landscape(x as f32 / width as f32, y as f32 / height as f32, warmth)
    })
}

/// The picture shown, or a line saying it could not be made.
pub fn picture(image: Option<ImageData>, width: f32, height: f32, alt: &str) -> AnyView {
    match image {
        Some(image) => Box::new(Image::new(image).size(width, height).alt(alt.to_string())),
        None => Box::new(Text::new("Sem imagem")),
    }
}

/// A heading of a section.
pub fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// A note under a section, in the muted ink.
pub fn note(text: impl Into<String>) -> Text {
    Text::new(text.into()).style(TextStyle::Caption).foreground(Semantic::InkMuted)
}

/// A number the way it is written in Portuguese, with a comma.
pub fn decimal(value: f32, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// How large the pictures are drawn on this screen, keeping their proportions.
pub fn shown_size(cx: &BuildCx<'_>, widest: f32) -> Size {
    let room = (cx.viewport().width - 64.0).max(200.0);
    let width = widest.min(room);
    Size::new(width, width * PHOTO.height / PHOTO.width)
}
