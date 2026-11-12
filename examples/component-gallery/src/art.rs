use ravenrs::prelude::*;

fn mix(a: Color, b: Color, t: f32) -> Color {
    a.lerp(b, t)
}

pub(crate) fn all() -> Vec<ImageData> {
    let (w, h) = (480, 300);
    let aurora = ImageData::from_fn(w, h, |x, y| {
        let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
        let sky = mix(Color::hex(0x0b1030), Color::hex(0x2a1b4f), v);
        let band =
            ((u * 9.0 + (v * 4.0).sin() * 1.5).sin() * 0.5 + 0.5) * (1.0 - (v - 0.35).abs() * 2.6).max(0.0);
        mix(sky, Color::hex(0x3cf2b0), band * 0.85)
    });
    let dunes = ImageData::from_fn(w, h, |x, y| {
        let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
        let mut c = mix(Color::hex(0xffb36b), Color::hex(0xff6f61), v);
        for (k, color) in [(0.55, 0xe2734a), (0.68, 0xc4553a), (0.82, 0x8f3b2e)] {
            let line = k + (u * 6.0 + k * 9.0).sin() * 0.05;
            if v > line {
                c = mix(c, Color::hex(color), 0.9);
            }
        }
        c
    });
    let ocean = ImageData::from_fn(w, h, |x, y| {
        let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
        let wave = ((u * 14.0 + v * 6.0).sin() + (u * 5.0 - v * 11.0).sin()) * 0.25 + 0.5;
        mix(mix(Color::hex(0x0a3d62), Color::hex(0x38ada9), v), Color::hex(0xd6f5ff), wave * 0.35)
    });
    let bloom = ImageData::from_fn(256, 256, |x, y| {
        let (u, v) = (x as f32 / 255.0 - 0.5, y as f32 / 255.0 - 0.5);
        let r = (u * u + v * v).sqrt() * 2.0;
        let a = v.atan2(u);
        let petals = ((a * 5.0).sin() * 0.5 + 0.5) * (1.0 - r).max(0.0);
        mix(mix(Color::hex(0x4c66ee), Color::hex(0x161a2e), r.min(1.0)), Color::hex(0xff8fd8), petals)
    });
    [aurora, dunes, ocean, bloom].into_iter().flatten().collect()
}
