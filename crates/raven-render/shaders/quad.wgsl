// RavenRS quad shader: rounded rectangles with gradient fill, border and soft
// shadow, all from one signed-distance function. One instance per quad; shadows
// are separate instances drawn just before their shape.

struct Globals {
    // Viewport size in logical pixels.
    viewport: vec2<f32>,
    // Device pixels per logical pixel.
    scale: f32,
    // 1 when the render target is *Srgb and colors must be linearized.
    srgb_target: u32,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct Instance {
    @location(0) rect: vec4<f32>,          // x, y, w, h (logical px)
    @location(1) radii: vec4<f32>,         // tl, tr, br, bl
    @location(2) color0: vec4<f32>,        // fill (gradient start), or shadow color
    @location(3) color1: vec4<f32>,        // gradient end
    @location(4) gradient: vec4<f32>,      // start.xy, end.xy relative to rect
    @location(5) border_color: vec4<f32>,
    @location(6) params: vec4<f32>,        // border width, shadow sigma, kind (0 shape, 1 shadow), gradient on
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,         // position relative to the rect center (logical px)
    @location(1) half_size: vec2<f32>,
    @location(2) uv: vec2<f32>,            // position relative to the rect, 0..1
    @location(3) radii: vec4<f32>,
    @location(4) color0: vec4<f32>,
    @location(5) color1: vec4<f32>,
    @location(6) gradient: vec4<f32>,
    @location(7) border_color: vec4<f32>,
    @location(8) params: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let is_shadow = inst.params.z > 0.5;
    // Room for anti-aliasing, and for the blur tail of shadows.
    let pad = select(1.0, inst.params.y * 3.0 + 1.0, is_shadow);
    let min_p = inst.rect.xy - vec2<f32>(pad);
    let size = inst.rect.zw + vec2<f32>(pad * 2.0);
    let p = min_p + corner * size;

    var out: VsOut;
    let ndc = p / globals.viewport * 2.0 - 1.0;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.half_size = inst.rect.zw * 0.5;
    out.local = p - (inst.rect.xy + out.half_size);
    out.uv = (p - inst.rect.xy) / max(inst.rect.zw, vec2<f32>(1e-4));
    let max_r = min(out.half_size.x, out.half_size.y);
    out.radii = min(inst.radii, vec4<f32>(max_r));
    out.color0 = inst.color0;
    out.color1 = inst.color1;
    out.gradient = inst.gradient;
    out.border_color = inst.border_color;
    out.params = inst.params;
    return out;
}

// Signed distance to a rounded box centered at the origin; radii per quadrant.
fn sd_rounded_box(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    var r: f32;
    if p.x < 0.0 {
        r = select(radii.w, radii.x, p.y < 0.0);
    } else {
        r = select(radii.z, radii.y, p.y < 0.0);
    }
    let q = abs(p) - half_size + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

// Abramowitz-Stegun approximation of erf, accurate to ~5e-4.
fn erf_approx(x: f32) -> f32 {
    let s = sign(x);
    let a = abs(x);
    let t = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    let r = t * t;
    return s - s / (r * r);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn premultiply(c: vec4<f32>) -> vec4<f32> {
    var rgb = c.rgb;
    if globals.srgb_target == 1u {
        rgb = srgb_to_linear(rgb);
    }
    return vec4<f32>(rgb * c.a, c.a);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = sd_rounded_box(in.local, in.half_size, in.radii);

    if in.params.z > 0.5 {
        // Gaussian-blurred box approximated by erf over the rounded-box distance.
        let sigma = max(in.params.y, 0.5);
        let coverage = 0.5 - 0.5 * erf_approx(d / (sigma * 1.41421356));
        return premultiply(vec4<f32>(in.color0.rgb, in.color0.a * coverage));
    }

    // One device pixel of anti-aliasing.
    let aa = 1.0 / globals.scale;
    let coverage = clamp(0.5 - d / aa, 0.0, 1.0);

    var fill = in.color0;
    if in.params.w > 0.5 {
        let dir = in.gradient.zw - in.gradient.xy;
        let t = clamp(dot(in.uv - in.gradient.xy, dir) / max(dot(dir, dir), 1e-6), 0.0, 1.0);
        fill = mix(in.color0, in.color1, t);
    }

    let border_width = in.params.x;
    if border_width > 0.0 {
        let border_mix = clamp(0.5 + (d + border_width) / aa, 0.0, 1.0);
        fill = mix(fill, in.border_color, border_mix);
    }

    return premultiply(vec4<f32>(fill.rgb, fill.a * coverage));
}
