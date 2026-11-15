// RavenRS cables drawn straight from the simulation's buffer: one instance per
// segment, its quad stretched along the segment and mitered with its neighbours,
// the surface (highlight, braid, coil, stripe, dashes) worked out per pixel.

struct Globals {
    viewport: vec2<f32>,
    scale: f32,
    srgb_target: u32,
};

@group(0) @binding(0) var<uniform> globals: Globals;

const MAX_POINTS: u32 = 32u;
// Pieces each segment is drawn in, along a Catmull-Rom curve: CABLE_SMOOTHING.
const SUB: u32 = 4u;
const SEGMENTS: u32 = 31u * SUB;

struct Look {
    // Slot of the cable in the simulation, its points, its texture
    // (0 smooth, 1 braided, 2 coiled, 3 striped) and whether it is dashed.
    slot: u32,
    n: u32,
    texture: u32,
    dashed: u32,
    color: vec4<f32>,
    stripe: vec4<f32>,
    // thickness, shine, dash, gap
    params: vec4<f32>,
    // opacity, then the view: scale and offset from the cable's units to the scene
    extra: vec4<f32>,
};

@group(1) @binding(0) var<storage, read> shape: array<vec4<f32>>;
@group(1) @binding(1) var<storage, read> looks: array<Look>;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    // Across the cable, in logical pixels from its middle.
    @location(0) across: f32,
    // Along the cable, in logical pixels from its start.
    @location(1) along: f32,
    @location(2) @interpolate(flat) item: u32,
};

// A point of the cable in the scene: its position through the view, and its length
// along the cable in scene pixels.
fn point(look: Look, k: u32) -> vec4<f32> {
    let p = shape[look.slot * MAX_POINTS + min(k, look.n - 1u)];
    return vec4<f32>(p.xy * look.extra.y + look.extra.zw, p.z * look.extra.y, 0.0);
}

fn normal_of(d: vec2<f32>) -> vec2<f32> {
    let length = max(sqrt(d.x * d.x + d.y * d.y), 1e-6);
    return vec2<f32>(-d.y, d.x) / length;
}

fn curve(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return 0.5 * (2.0 * p1 + (-p0 + p2) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
}

fn slope(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    return 0.5 * ((-p0 + p2) + 2.0 * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t
        + 3.0 * (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) instance: u32) -> VsOut {
    let item = instance / SEGMENTS;
    let piece = instance % SEGMENTS;
    let seg = piece / SUB;
    let look = looks[item];
    var out: VsOut;
    out.item = item;
    if seg + 1u >= look.n {
        // Past the cable's last segment: nothing to draw.
        out.position = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        return out;
    }
    let t = (f32(piece % SUB) + f32(vi & 1u)) / f32(SUB);
    let side = select(-1.0, 1.0, (vi >> 1u) == 1u);
    let p0 = point(look, select(seg - 1u, 0u, seg == 0u));
    let p1 = point(look, seg);
    let p2 = point(look, seg + 1u);
    let p3 = point(look, seg + 2u);
    let here = curve(p0.xy, p1.xy, p2.xy, p3.xy, t);
    let n = normal_of(slope(p0.xy, p1.xy, p2.xy, p3.xy, t));
    let half = look.params.x * 0.5;
    // Room for anti-aliasing.
    let reach = half + 1.0 / globals.scale;
    let p = here + n * reach * side;
    out.position = vec4<f32>(p.x / globals.viewport.x * 2.0 - 1.0, 1.0 - p.y / globals.viewport.y * 2.0, 0.0, 1.0);
    out.across = reach * side;
    out.along = mix(p1.z, p2.z, t);
    return out;
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
    let look = looks[in.item];
    let half = look.params.x * 0.5;
    let v = in.across / max(half, 1e-3);
    // Coverage across the edge, one device pixel wide.
    let coverage = clamp((half - abs(in.across)) * globals.scale + 0.5, 0.0, 1.0);
    if look.dashed == 1u {
        let period = look.params.z + look.params.w;
        if fract(in.along / max(period, 1e-3)) * period > look.params.z {
            discard;
        }
    }
    var color = look.color;
    let dark = vec4<f32>(color.rgb * 0.65, color.a);
    let w = look.params.x;
    if look.texture == 1u {
        // A braid: slanted marks every 0.9 widths.
        let t = fract((in.along + v * 0.55 * half) / max(w * 0.9, 2.0));
        color = mix(color, dark, step(t, 0.18));
    } else if look.texture == 2u {
        // A coil: straight marks every 0.6 widths.
        let t = fract(in.along / max(w * 0.6, 2.0));
        color = mix(color, dark, step(t, 0.18));
    } else if look.texture == 3u {
        color = mix(color, look.stripe, step(abs(v), 0.3));
    }
    // The highlight: a band above the middle.
    let shine = clamp(look.params.y, 0.0, 1.0) * 0.7;
    let band = 1.0 - smoothstep(0.0, 0.22, abs(v + 0.38));
    color = vec4<f32>(mix(color.rgb, mix(look.color.rgb, vec3<f32>(1.0), 0.55), band * shine), color.a);
    return premultiply(vec4<f32>(color.rgb, color.a * coverage * look.extra.x));
}
