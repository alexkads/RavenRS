//! Every icon of the RavenIcons font must rasterize centered in its em box.
//!
//! Lucide's font shipped every left side bearing as 0 while the outlines start
//! further right; rasterizers place the outline by the bearing, so a check mark
//! drew 1.75px left of center inside a 14px check box.

#![allow(clippy::expect_used, reason = "test fails loudly")]

use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;

static RAVEN_ICONS: &[u8] = include_bytes!("../../ravenrs-text/fonts/RavenIcons.ttf");

#[test]
fn every_icon_ink_is_centered_in_the_em_box() {
    const SIZE: f32 = 20.0;
    let font = FontRef::from_index(RAVEN_ICONS, 0).expect("RavenIcons parses");
    let count = font.metrics(&[]).glyph_count;
    let mut context = ScaleContext::new();
    let mut scaler = context.builder(font).size(SIZE).hint(false).build();
    let mut checked = 0;
    for id in 1..count {
        let Some(image) = Render::new(&[Source::Outline]).format(Format::Alpha).render(&mut scaler, id)
        else {
            continue;
        };
        let (w, h) = (image.placement.width as usize, image.placement.height as usize);
        if w == 0 || h == 0 {
            continue;
        }
        let ink_cols: Vec<usize> = (0..w).filter(|&x| (0..h).any(|y| image.data[y * w + x] > 32)).collect();
        let (Some(&first), Some(&last)) = (ink_cols.first(), ink_cols.last()) else { continue };
        let center = image.placement.left as f32 + (first + last + 1) as f32 / 2.0;
        assert!(
            (center - SIZE / 2.0).abs() <= 1.0,
            "glyph {id}: ink center at {center:.2}px, em center at {:.2}px",
            SIZE / 2.0
        );
        checked += 1;
    }
    assert!(checked > 150, "expected the whole icon set, rasterized {checked}");
}

/// Every `IconName` draws a glyph of the font: an icon added to the enum without its glyph, or
/// a glyph left out of the font, would draw nothing (#748 added `brain`, `route`, `bot` and
/// `workflow`).
#[test]
fn every_icon_name_has_its_glyph_in_the_font() {
    let icon_set = include_str!("../../ravenrs-widgets-base/src/icon_set.rs");
    let font = FontRef::from_index(RAVEN_ICONS, 0).expect("RavenIcons parses");
    let charmap = font.charmap();
    let points: Vec<u32> = icon_set
        .split("'\\u{")
        .skip(1)
        .filter_map(|rest| u32::from_str_radix(rest.split('}').next()?, 16).ok())
        .collect();
    assert!(points.len() > 200, "read {} icons from icon_set.rs", points.len());
    for point in &points {
        assert_ne!(charmap.map(*point), 0, "U+{point:X} has no glyph in RavenIcons.ttf");
    }
    for name in ["\"brain\"", "\"route\"", "\"bot\"", "\"workflow\""] {
        assert!(icon_set.contains(name), "{name} is missing from IconName");
    }
}
