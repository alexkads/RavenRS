//! Property tests: invariants of geometry and color over random inputs.

use proptest::prelude::*;
use ravenrs_core::{Color, Point, Rect};

fn rect() -> impl Strategy<Value = Rect> {
    (-500.0f32..500.0, -500.0f32..500.0, 0.0f32..400.0, 0.0f32..400.0)
        .prop_map(|(x, y, w, h)| Rect::new(x, y, w, h))
}

fn color() -> impl Strategy<Value = Color> {
    (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0).prop_map(|(r, g, b, a)| Color::rgba(r, g, b, a))
}

proptest! {
    #[test]
    fn intersection_is_commutative_and_inside_both(a in rect(), b in rect()) {
        let (i, j) = (a.intersect(&b), b.intersect(&a));
        prop_assert_eq!(i.is_empty(), j.is_empty());
        if !i.is_empty() {
            prop_assert_eq!(i, j);
            for r in [a, b] {
                prop_assert!(i.x() >= r.x() - 1e-3 && i.max_x() <= r.max_x() + 1e-3);
                prop_assert!(i.y() >= r.y() - 1e-3 && i.max_y() <= r.max_y() + 1e-3);
            }
        }
    }

    #[test]
    fn a_point_is_in_the_intersection_iff_it_is_in_both(a in rect(), b in rect(), x in -600.0f32..600.0, y in -600.0f32..600.0) {
        let p = Point::new(x, y);
        prop_assert_eq!(a.intersect(&b).contains(p), a.contains(p) && b.contains(p));
    }

    #[test]
    fn inflate_then_deflate_restores_non_degenerate_rects(r in rect(), d in 0.0f32..50.0) {
        let back = r.inflate(d).inflate(-d);
        prop_assert!((back.x() - r.x()).abs() < 1e-3 && (back.width() - r.width()).abs() < 1e-3);
    }

    #[test]
    fn lerp_hits_both_ends_and_stays_in_range(a in color(), b in color(), t in -1.0f32..2.0) {
        prop_assert_eq!(a.lerp(b, 0.0), a);
        let end = a.lerp(b, 1.0);
        prop_assert!((end.r - b.r).abs() < 1e-6 && (end.a - b.a).abs() < 1e-6);
        let mid = a.lerp(b, t);
        for (m, lo, hi) in [(mid.r, a.r.min(b.r), a.r.max(b.r)), (mid.a, a.a.min(b.a), a.a.max(b.a))] {
            prop_assert!(m >= lo - 1e-6 && m <= hi + 1e-6);
        }
    }

    #[test]
    fn contrast_is_symmetric_and_between_1_and_21(a in color(), b in color()) {
        let (x, y) = (a.contrast_ratio(b), b.contrast_ratio(a));
        prop_assert!((x - y).abs() < 1e-5);
        prop_assert!((1.0..=21.0 + 1e-3).contains(&x));
    }

    #[test]
    fn hex_round_trips(v in 0u32..=0xff_ffff) {
        let c = Color::hex(v);
        let back = ((c.r * 255.0).round() as u32) << 16 | ((c.g * 255.0).round() as u32) << 8 | (c.b * 255.0).round() as u32;
        prop_assert_eq!(back, v);
    }
}
