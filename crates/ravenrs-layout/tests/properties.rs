//! Property tests: stacks place children in order, without overlap, inside padding.

use proptest::prelude::*;
use ravenrs_core::{Insets, Size};
use ravenrs_layout::{Axis, LayoutStyle, LayoutTree};

proptest! {
    #[test]
    fn vertical_stack_orders_children_with_exact_spacing(
        sizes in proptest::collection::vec((1.0f32..200.0, 1.0f32..80.0), 1..12),
        spacing in 0.0f32..40.0,
        padding in 0.0f32..40.0,
    ) {
        let mut tree = LayoutTree::<()>::new();
        let kids: Vec<_> = sizes
            .iter()
            .map(|&(w, h)| tree.node(LayoutStyle { width: Some(w), height: Some(h), ..LayoutStyle::leaf() }, &[]).unwrap())
            .collect();
        let style = LayoutStyle { spacing, padding: Insets::uniform(padding), ..LayoutStyle::stack(Axis::Vertical) };
        let root = tree.node(style, &kids).unwrap();
        tree.compute(root, Size::new(10_000.0, 10_000.0), |_, _| Size::ZERO).unwrap();
        let frames: Vec<_> = kids.iter().map(|&k| tree.frame(k).unwrap()).collect();
        // Taffy rounds to whole pixels, so allow one pixel of slack per edge.
        prop_assert!((frames[0].y() - padding).abs() <= 1.0);
        for w in frames.windows(2) {
            let gap = w[1].y() - w[0].max_y();
            prop_assert!((gap - spacing).abs() <= 1.0, "gap {gap} vs spacing {spacing}");
        }
        for (f, &(w, h)) in frames.iter().zip(&sizes) {
            prop_assert!((f.x() - padding).abs() <= 1.0);
            prop_assert!((f.width() - w).abs() <= 1.0 && (f.height() - h).abs() <= 1.0);
        }
        let root_frame = tree.frame(root).unwrap();
        let last = frames.last().unwrap();
        prop_assert!((root_frame.height() - (last.max_y() + padding)).abs() <= 1.5);
    }
}
