// RavenRS glyph shader: draws coverage masks from the R8 glyph atlas. Glyph
// quads arrive in device pixels, already snapped to the pixel grid.

struct Globals {
    viewport: vec2<f32>,
    scale: f32,
    srgb_target: u32,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,  // x, y, w, h in device px
    @location(1) uv: vec4<f32>,    // u0, v0, u1, v1
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = inst.rect.xy + corner * inst.rect.zw;
    let ndc = p / (globals.viewport * globals.scale) * 2.0 - 1.0;
    var out: VsOut;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.uv = mix(inst.uv.xy, inst.uv.zw, corner);
    out.color = inst.color;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let coverage = textureSampleLevel(atlas, atlas_sampler, in.uv, 0.0).r;
    var rgb = in.color.rgb;
    if globals.srgb_target == 1u {
        rgb = srgb_to_linear(rgb);
    }
    let a = in.color.a * coverage;
    return vec4<f32>(rgb * a, a);
}
