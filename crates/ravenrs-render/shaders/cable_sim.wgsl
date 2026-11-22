// RavenRS cables: every cable of the window simulated in one dispatch. One
// workgroup per cable, one thread per point. This is `cable_step` of
// ravenrs-core/src/cable.rs, step for step: Verlet with friction, then distance
// constraints solved red then black, so no two threads move the same point.

const MAX_POINTS: u32 = 32u;
const ITERATIONS: u32 = 12u;
const MAX_PINS: u32 = 8u;

struct Params {
    // Points of the cable; 0 for a free slot.
    n: u32,
    // Steps to run this frame.
    steps: u32,
    // Pins in use.
    pins: u32,
    _pad0: u32,
    rest: f32,
    fall: f32,
    keep: f32,
    stiffness: f32,
    friction: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    // x, y, point index, unused.
    pin: array<vec4<f32>, 8>,
};

// Per point: position in xy, last position in zw.
@group(0) @binding(0) var<storage, read_write> state: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> params: array<Params>;
// Per point, for drawing: position in xy, length along the cable in z.
@group(0) @binding(2) var<storage, read_write> shape: array<vec4<f32>>;
// The farthest any point moved in the last step, in thousandths of a pixel.
@group(0) @binding(3) var<storage, read_write> moved: atomic<u32>;

var<workgroup> pos: array<vec2<f32>, 32>;
var<workgroup> prv: array<vec2<f32>, 32>;
var<workgroup> held: array<u32, 32>;
var<workgroup> hold: array<vec2<f32>, 32>;

fn solve(k: u32, rest: f32, stiffness: f32) {
    let wa = select(1.0, 0.0, held[k] == 1u);
    let wb = select(1.0, 0.0, held[k + 1u] == 1u);
    let total = wa + wb;
    if total <= 0.0 {
        return;
    }
    let a = pos[k];
    let b = pos[k + 1u];
    let d = b - a;
    let length = sqrt(d.x * d.x + d.y * d.y);
    // A rope only resists being stretched.
    if length > rest && length > 1.1920929e-7 {
        let push = (length - rest) / length * stiffness / total;
        pos[k] = a + d * push * wa;
        pos[k + 1u] = b - d * push * wb;
    }
}

@compute @workgroup_size(32)
fn step(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) i: u32) {
    let slot = group.x;
    let p = params[slot];
    let n = p.n;
    let base = slot * MAX_POINTS;
    let alive = i < n;
    if alive {
        let s = state[base + i];
        pos[i] = s.xy;
        prv[i] = s.zw;
    }
    held[i] = 0u;
    workgroupBarrier();
    if i < min(p.pins, MAX_PINS) {
        let pin = p.pin[i];
        let k = u32(pin.z);
        if k < MAX_POINTS {
            held[k] = 1u;
            hold[k] = pin.xy;
        }
    }
    workgroupBarrier();
    for (var s = 0u; s < p.steps; s = s + 1u) {
        if alive {
            let q = pos[i];
            if held[i] == 1u {
                prv[i] = hold[i];
                pos[i] = hold[i];
            } else {
                let v = q - prv[i];
                let speed = sqrt(v.x * v.x + v.y * v.y);
                let keep = select(p.keep, min(p.keep, 0.85), speed < p.friction);
                prv[i] = q;
                pos[i] = q + v * keep + vec2<f32>(0.0, p.fall);
            }
        }
        workgroupBarrier();
        for (var it = 0u; it < ITERATIONS; it = it + 1u) {
            for (var parity = 0u; parity < 2u; parity = parity + 1u) {
                let k = i * 2u + parity;
                if k + 1u < n {
                    solve(k, p.rest, p.stiffness);
                }
                workgroupBarrier();
            }
        }
    }
    if alive && p.steps > 0u {
        state[base + i] = vec4<f32>(pos[i], prv[i]);
        let d = pos[i] - prv[i];
        atomicMax(&moved, u32(sqrt(d.x * d.x + d.y * d.y) * 1000.0));
    }
    workgroupBarrier();
    // The length along the cable, for dashes and textures.
    if i == 0u && n > 0u {
        var along = 0.0;
        for (var k = 0u; k < n; k = k + 1u) {
            if k > 0u {
                along = along + distance(pos[k], pos[k - 1u]);
            }
            shape[base + k] = vec4<f32>(pos[k], along, 0.0);
        }
    }
}
