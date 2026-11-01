// RavenRS image shader: a textured rectangle with an optional rounded-corner
// mask from the same signed-distance function as quads.

struct Globals {
    viewport: vec2<f32>,
    scale: f32,
    srgb_target: u32,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,    // x, y, w, h (logical px)
    @location(1) uv: vec4<f32>,      // u0, v0, u1, v1
    @location(2) radii: vec4<f32>,   // tl, tr, br, bl
    @location(3) params: vec4<f32>,  // opacity
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) t: vec2<f32>,
    @location(3) uv: vec4<f32>,
    @location(4) radii: vec4<f32>,
    @location(5) opacity: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let pad = 1.0;
    let p = inst.rect.xy - vec2<f32>(pad) + corner * (inst.rect.zw + vec2<f32>(pad * 2.0));
    var out: VsOut;
    let ndc = p / globals.viewport * 2.0 - 1.0;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.half_size = inst.rect.zw * 0.5;
    out.local = p - (inst.rect.xy + out.half_size);
    out.t = (p - inst.rect.xy) / max(inst.rect.zw, vec2<f32>(1e-4));
    out.uv = inst.uv;
    out.radii = min(inst.radii, vec4<f32>(min(out.half_size.x, out.half_size.y)));
    out.opacity = inst.params.x;
    return out;
}

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

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = sd_rounded_box(in.local, in.half_size, in.radii);
    let coverage = clamp(0.5 - d * globals.scale, 0.0, 1.0);
    let t = clamp(in.t, vec2<f32>(0.0), vec2<f32>(1.0));
    let uv = mix(in.uv.xy, in.uv.zw, t);
    let texel = textureSample(image, image_sampler, uv);
    var rgb = texel.rgb;
    if globals.srgb_target == 1u {
        rgb = srgb_to_linear(rgb);
    }
    let a = texel.a * in.opacity * coverage;
    return vec4<f32>(rgb * a, a);
}
