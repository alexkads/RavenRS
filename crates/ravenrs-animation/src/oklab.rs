use ravenrs_core::Color;

/// Converts a color to OKLab: `[L, a, b, alpha]`. Equal steps in this space look
/// like equal steps to the eye, which is not true of sRGB.
pub fn to_oklab(color: Color) -> [f32; 4] {
    let [r, g, b, alpha] = color.to_linear();
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_84 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        alpha,
    ]
}

/// Converts `[L, a, b, alpha]` back to a color, clamped to the sRGB gamut.
pub fn from_oklab(lab: [f32; 4]) -> Color {
    let [l, a, b, alpha] = lab;
    let lp = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let mp = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let sp = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (lc, mc, sc) = (lp * lp * lp, mp * mp * mp, sp * sp * sp);
    let channel = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
    };
    Color::rgba(
        channel(4.076_741_7 * lc - 3.307_711_6 * mc + 0.230_969_94 * sc),
        channel(-1.268_438 * lc + 2.609_757_4 * mc - 0.341_319_38 * sc),
        channel(-0.004_196_086_3 * lc - 0.703_418_6 * mc + 1.707_614_7 * sc),
        alpha.clamp(0.0, 1.0),
    )
}

/// Mixes two colors in OKLab with premultiplied alpha, so a fade from
/// transparent keeps the destination's hue instead of passing through gray.
pub fn mix_oklab(from: Color, to: Color, t: f32) -> Color {
    let (a, b) = (premultiplied(from), premultiplied(to));
    unpremultiplied(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t))
}

pub(crate) fn premultiplied(color: Color) -> [f32; 4] {
    let [l, a, b, alpha] = to_oklab(color);
    [l * alpha, a * alpha, b * alpha, alpha]
}

pub(crate) fn unpremultiplied(v: [f32; 4]) -> Color {
    let alpha = v[3];
    if alpha <= 1e-4 {
        return Color::TRANSPARENT;
    }
    from_oklab([v[0] / alpha, v[1] / alpha, v[2] / alpha, alpha])
}
