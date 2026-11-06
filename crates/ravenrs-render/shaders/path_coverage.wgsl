// RavenRS path coverage: each straight edge adds to every pixel on its right the
// exact part of that pixel it covers, signed by its direction. Adding them all up
// gives each pixel's winding, with the true area at the edges of the shape.

struct Target {
    size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> target_size: Target;

struct Edge {
    @location(0) line: vec4<f32>,   // x0, y0, x1, y1 (atlas px)
    @location(1) region: vec4<f32>, // left, top, right, bottom (atlas px)
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) line: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, edge: Edge) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let x0 = clamp(floor(min(edge.line.x, edge.line.z)), edge.region.x, edge.region.z);
    let y0 = clamp(floor(min(edge.line.y, edge.line.w)), edge.region.y, edge.region.w);
    let y1 = clamp(ceil(max(edge.line.y, edge.line.w)), edge.region.y, edge.region.w);
    let p = vec2<f32>(mix(x0, edge.region.z, corner.x), mix(y0, y1, corner.y));
    var out: VsOut;
    let ndc = p / target_size.size * 2.0 - 1.0;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.line = edge.line;
    return out;
}

// The integral of clamp(1 - u, 0, 1), the part of a pixel to the right of u.
fn right_area(u: f32) -> f32 {
    let s = clamp(u, 0.0, 1.0);
    return min(u, 0.0) + s - s * s * 0.5;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let px = floor(in.position.x);
    let py = floor(in.position.y);
    var a = in.line.xy;
    var b = in.line.zw;
    var sign = 1.0;
    if a.y > b.y {
        let swap = a;
        a = b;
        b = swap;
        sign = -1.0;
    }
    let height = b.y - a.y;
    let from_y = max(a.y, py);
    let to_y = min(b.y, py + 1.0);
    if height <= 0.0 || to_y <= from_y {
        return vec4<f32>(0.0);
    }
    let run = (b.x - a.x) / height;
    let xa = a.x + (from_y - a.y) * run;
    let xb = a.x + (to_y - a.y) * run;
    let dy = to_y - from_y;
    // Next to this pixel. Atlas coordinates are large, and subtracting them loses the
    // digits that a nearly vertical edge is all about.
    let ua = xa - px;
    let ub = xb - px;
    var area: f32;
    if max(ua, ub) <= 0.0 {
        area = dy;
    } else if min(ua, ub) >= 1.0 {
        area = 0.0;
    } else if abs(ub - ua) < 1e-3 {
        area = clamp(1.0 - (ua + ub) * 0.5, 0.0, 1.0) * dy;
    } else {
        area = (right_area(ub) - right_area(ua)) / (ub - ua) * dy;
    }
    return vec4<f32>(sign * area, 0.0, 0.0, 0.0);
}
