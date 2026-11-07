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
