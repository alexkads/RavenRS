//! The promises the rest of the framework relies on.

use proptest::prelude::*;
use ravenrs_animation::{Animated, CubicBezier, Easing, Spring, Transition, mix_oklab, to_oklab};
use ravenrs_core::Color;

fn integrate(spring: Spring, x0: f32, v0: f32, t: f32) -> f32 {
    let (mut x, mut v) = (f64::from(x0), f64::from(v0));
    let (k, c) = (f64::from(spring.stiffness), f64::from(spring.damping));
    let dt = 1e-5;
    for _ in 0..(f64::from(t) / dt) as u64 {
        v += (-k * x - c * v) * dt;
        x += v * dt;
    }
    x as f32
}

#[test]
fn springs_match_the_differential_equation_in_every_damping_regime() {
    for spring in [
        Spring::new(380.0, 10.0), // under
        Spring::new(100.0, 20.0), // critical
        Spring::new(100.0, 40.0), // over
        Spring::snappy(),
        Spring::gentle(),
    ] {
        for t in [0.01, 0.1, 0.3] {
            let (x, _) = spring.sample(10.0, -30.0, t);
            let expected = integrate(spring, 10.0, -30.0, t);
            assert!((x - expected).abs() < 0.01, "{spring:?} at {t}: {x} vs {expected}");
        }
    }
}

#[test]
fn a_spring_lands_the_same_at_any_frame_rate() {
    let run = |frames: u32| {
        let mut a = Animated::new(0.0_f32);
        a.set_target(100.0, Transition::snappy(), 0.0);
        for f in 0..frames {
            a.step(f64::from(f) * 0.25 / f64::from(frames));
        }
        a.value(0.25)
    };
    assert_eq!(run(15), run(240));
}

#[test]
fn springs_and_tweens_settle_exactly_on_target() {
    for t in [Transition::snappy(), Transition::gentle(), Transition::fast(), Transition::slow()] {
        let mut a = Animated::new(0.0_f32);
        a.set_target(1.0, t, 0.0);
        assert!(a.step(0.0), "{t:?} starts moving");
        assert!(!a.step(5.0), "{t:?} comes to rest");
        assert_eq!(a.value(5.0), 1.0);
    }
}

#[test]
fn instant_transitions_do_not_move() {
    let mut a = Animated::new(0.0_f32);
    a.set_target(1.0, Transition::Instant, 0.0);
    assert_eq!(a.value(0.0), 1.0);
    assert!(!a.step(0.0));
}

#[test]
fn interrupting_a_spring_keeps_position_and_velocity() {
    let mut a = Animated::new(0.0_f32);
    a.set_target(100.0, Transition::snappy(), 0.0);
    let (x, v) = (a.value(0.08), a.velocity(0.08)[0]);
    a.set_target(-50.0, Transition::snappy(), 0.08);
    assert!((a.value(0.08) - x).abs() < 1e-3);
    assert!((a.velocity(0.08)[0] - v).abs() < 1e-2);
}

#[test]
fn additive_tweens_turn_around_without_a_jump() {
    let mut a = Animated::new(0.0_f32);
    a.set_target(1.0, Transition::fast(), 0.0);
    let before = a.value(0.05);
    a.set_target(0.0, Transition::fast(), 0.05);
    assert!((a.value(0.05) - before).abs() < 1e-5, "no jump at the turn");
    let later = a.value(0.06);
    assert!((later - before).abs() < 0.2, "no sudden speed either");
    assert!(!a.step(1.0));
    assert_eq!(a.value(1.0), 0.0);
}

#[test]
fn asking_for_the_same_target_changes_nothing() {
    let mut a = Animated::new(0.0_f32);
    a.set_target(1.0, Transition::snappy(), 0.0);
    let v = a.value(0.1);
    a.set_target(1.0, Transition::snappy(), 0.1);
    assert_eq!(a.value(0.1), v);
}

#[test]
fn css_curves_match_published_values() {
    let ease = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
    assert!((ease.sample(0.5) - 0.8024).abs() < 0.002);
    assert_eq!(Easing::EaseOut.sample(0.0), 0.0);
    assert_eq!(Easing::EaseOut.sample(1.0), 1.0);
}

#[test]
fn oklab_midpoints_stay_lighter_and_saturated_than_srgb() {
    let (blue, yellow) = (Color::hex(0x0000ff), Color::hex(0xffff00));
    let oklab = mix_oklab(blue, yellow, 0.5);
    let srgb = blue.lerp(yellow, 0.5);
    assert!(oklab.luminance() > srgb.luminance() * 0.9);
    let chroma = |c: Color| c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
    assert!(chroma(srgb) < 0.01, "sRGB goes through gray");
    assert!(chroma(oklab) > 0.1, "OKLab does not");
}

#[test]
fn fading_from_transparent_keeps_the_hue() {
    let accent = Color::hex(0x6d5efc);
    let mid = mix_oklab(Color::TRANSPARENT, accent, 0.5);
    assert!((mid.a - 0.5).abs() < 1e-3);
    let (a, b) = (to_oklab(mid), to_oklab(accent));
    assert!((a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01, "{a:?} vs {b:?}");
}

proptest! {
    #[test]
    fn oklab_round_trips(r in 0.0_f32..1.0, g in 0.0_f32..1.0, b in 0.0_f32..1.0, a in 0.01_f32..1.0) {
        let c = Color::rgba(r, g, b, a);
        let back = mix_oklab(c, c, 0.5);
        for (x, y) in c.to_array().iter().zip(back.to_array()) {
            prop_assert!((x - y).abs() < 0.003, "{c:?} became {back:?}");
        }
    }

    #[test]
    fn redirecting_is_always_continuous(
        first in -500.0_f32..500.0,
        second in -500.0_f32..500.0,
        at in 0.0_f64..0.5,
        spring in any::<bool>(),
    ) {
        let t = if spring { Transition::snappy() } else { Transition::base() };
        let mut a = Animated::new(0.0_f32);
        a.set_target(first, t, 0.0);
        let before = a.value(at);
        a.set_target(second, t, at);
        prop_assert!((a.value(at) - before).abs() < 0.05);
        prop_assert!(!a.step(at + 10.0));
        prop_assert_eq!(a.value(at + 10.0), second);
    }

    #[test]
    fn bezier_is_monotonic_for_monotonic_curves(a in 0.0_f32..1.0, b in 0.0_f32..1.0) {
        prop_assume!(a < b);
        let curve = Easing::EaseInOut;
        prop_assert!(curve.sample(a) <= curve.sample(b) + 1e-3);
    }
}
