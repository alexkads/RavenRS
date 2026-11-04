// RavenRS paths: paints a shape from the coverage its edges left in the atlas.

struct Globals {
    viewport: vec2<f32>,
    scale: f32,
    srgb_target: u32,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var coverage: texture_2d<f32>;
@group(1) @binding(1) var ramps: texture_2d<f32>;
@group(1) @binding(2) var ramp_sampler: sampler;
@group(1) @binding(3) var images: texture_2d<f32>;

struct Instance {
    @location(0) rect: vec4<f32>,     // x, y, w, h (device px)
    @location(1) atlas: vec4<f32>,    // x, y, even-odd, paint kind
    @location(2) color: vec4<f32>,    // or, for an image, its place in the atlas (0 to 1)
    @location(3) gradient: vec4<f32>, // start and end, center and radius, or from pixels to u, and v's first
    @location(4) params: vec4<f32>,   // ramp row (0 to 1), opacity, the rest of v
    @location(5) clip: vec4<f32>,     // x, y (below zero for none), even-odd, inverted
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) origin: vec2<f32>,
    @location(1) @interpolate(flat) atlas: vec4<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) gradient: vec4<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
    @location(5) @interpolate(flat) clip: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = inst.rect.xy + corner * inst.rect.zw;
    let ndc = p / (globals.viewport * globals.scale) * 2.0 - 1.0;
    var out: VsOut;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.origin = inst.rect.xy;
    out.atlas = inst.atlas;
    out.color = inst.color;
    out.gradient = inst.gradient;
    out.params = inst.params;
    out.clip = inst.clip;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn covered(winding: f32, even_odd: f32) -> f32 {
    if even_odd > 0.5 {
        let folded = abs(winding) % 2.0;
        return select(folded, 2.0 - folded, folded > 1.0);
    }
    // What rounding left behind where nothing covers the pixel is not a shape.
    let amount = min(abs(winding), 1.0);
    return select(amount, 0.0, amount < 0.003);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let pixel = floor(in.position.xy);
    let texel = vec2<i32>(in.atlas.xy + pixel - in.origin);
    var inside = covered(textureLoad(coverage, texel, 0).r, in.atlas.z);
    if in.clip.x >= 0.0 {
        let clip_texel = vec2<i32>(in.clip.xy + pixel - in.origin);
        let seen = covered(textureLoad(coverage, clip_texel, 0).r, in.clip.z);
        inside *= select(seen, 1.0 - seen, in.clip.w > 0.5);
    }
    var color = in.color;
    if in.atlas.w > 2.5 {
        let p = in.position.xy;
        let u = in.gradient.x * p.x + in.gradient.y * p.y + in.gradient.z;
        let v = in.gradient.w * p.x + in.params.z * p.y + in.params.w;
        let seen = in.color.xy + clamp(vec2<f32>(u, v), vec2<f32>(0.0), vec2<f32>(1.0)) * in.color.zw;
        let texel_color = textureSampleLevel(images, ramp_sampler, seen, 0.0);
        color = vec4<f32>(texel_color.rgb / max(texel_color.a, 1e-4), texel_color.a);
    } else if in.atlas.w > 0.5 {
        var t: f32;
        let p = in.position.xy;
        if in.atlas.w < 1.5 {
            let along = in.gradient.zw - in.gradient.xy;
            t = dot(p - in.gradient.xy, along) / max(dot(along, along), 1e-6);
        } else {
            t = length(p - in.gradient.xy) / max(in.gradient.z, 1e-6);
        }
        // Inner pixels of the ramp, so that the sampler cannot read outside it.
        let u = clamp(t, 0.0, 1.0) * 255.0 / 256.0 + 0.5 / 256.0;
        color = textureSampleLevel(ramps, ramp_sampler, vec2<f32>(u, in.params.x), 0.0);
    }
    var rgb = color.rgb;
    if globals.srgb_target == 1u {
        rgb = srgb_to_linear(rgb);
    }
    let a = color.a * in.params.y * inside;
    return vec4<f32>(rgb * a, a);
}
