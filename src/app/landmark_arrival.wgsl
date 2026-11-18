#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct ArrivalInk {
    progress: f32,
    opacity: f32,
    seed: f32,
    direction: f32,
    motion: u32,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ink: ArrivalInk;

fn bell(p: vec2<f32>, radius: vec2<f32>) -> f32 {
    let q = p / radius;
    return exp(-dot(q, q));
}

fn hairline(distance: f32, width: f32) -> f32 {
    let feather = clamp(fwidth(distance), 0.006, 0.09);
    return width / (width + feather) *
        (1.0 - smoothstep(0.0, width + feather, abs(distance)));
}

fn broken_ring(p: vec2<f32>, radius: f32, width: f32) -> f32 {
    let r = length(p);
    let angle = atan2(p.y, p.x + 0.00001);
    let grain = 0.65 + 0.35 * sin(5.0 * angle + ink.seed) * sin(11.0 * angle);
    return hairline(r - radius - 0.015 * sin(9.0 * angle), width) * grain;
}

fn rise(p: vec2<f32>, t: f32) -> f32 {
    var strokes = 0.0;
    for (var i = 0; i < 7; i += 1) {
        let n = f32(i);
        let x = (n - 3.0) * 0.20 * (0.85 + 0.25 * t);
        let lift = 0.22 + 0.38 * fract(n * 0.618 + ink.seed);
        let center = vec2<f32>(x, 0.12 - t * lift);
        strokes += bell(p - center, vec2<f32>(0.023, 0.068 + lift * 0.12));
    }
    let roots = bell(p, vec2<f32>(0.62, 0.18));
    return strokes * 0.19 + roots * 0.08;
}

fn impact(p: vec2<f32>, t: f32) -> f32 {
    let spread = 1.0 - (1.0 - t) * (1.0 - t);
    let q = p * vec2<f32>(1.0, 1.25);
    let edge = broken_ring(q, 0.28 + 0.57 * spread, 0.056);
    let echo = broken_ring(q, 0.17 + 0.48 * spread, 0.035);
    let shadow = bell(p, vec2<f32>(0.35 + 0.23 * spread, 0.25));
    var flecks = 0.0;
    for (var i = 0; i < 8; i += 1) {
        let n = f32(i);
        let angle = n * 2.39996 + ink.seed;
        let radius = 0.29 + spread * (0.40 + 0.19 * fract(n * 0.618));
        let center = vec2<f32>(cos(angle), sin(angle) * 0.65) * radius
            - vec2<f32>(0.0, 0.52 * t * (1.0 - t));
        flecks += bell(p - center, vec2<f32>(0.037, 0.052));
    }
    return edge * 0.43 + echo * 0.14 + shadow * 0.24 + flecks * 0.33;
}

fn settle(p: vec2<f32>, t: f32) -> f32 {
    let wash = bell(p, vec2<f32>(0.42 + 0.36 * t, 0.27 + 0.12 * t));
    let ripple = broken_ring(p * vec2<f32>(1.0, 1.48), 0.32 + 0.54 * t, 0.053);
    return wash * 0.20 + ripple * 0.19;
}

fn turn(p: vec2<f32>, t: f32) -> f32 {
    let angle = atan2(p.y, p.x + 0.00001);
    let sweep = cos(angle - (ink.seed + t * 0.95));
    let taper = smoothstep(0.22, 0.98, abs(sweep));
    let radius = 0.64 + 0.10 * t + 0.035 * sweep;
    return hairline(length(p) - radius, 0.049) * taper * 0.33;
}

fn unfurl(p: vec2<f32>, t: f32) -> f32 {
    let ahead = p.y * ink.direction;
    let crest = -0.56 + 1.30 * t - 0.25 * p.x * p.x;
    let taper = 1.0 - smoothstep(0.30, 0.91, abs(p.x));
    let line = hairline(ahead - crest, 0.055);
    let wake = hairline(ahead - crest + 0.15, 0.038);
    return taper * (line * 0.27 + wake * 0.12);
}

fn stamp(p: vec2<f32>, t: f32) -> f32 {
    let q = p * vec2<f32>(1.0, 1.13);
    let radius = 0.42 + 0.40 * sqrt(t);
    let edge = broken_ring(q, radius, 0.045);
    let inner = broken_ring(q, radius - 0.13, 0.031);
    return edge * 0.34 + inner * 0.17;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let p = (mesh.uv - vec2<f32>(0.5)) * 2.0;
    let t = ink.progress;
    var pigment: f32;
    switch ink.motion {
        case 0u: { pigment = rise(p, t); }
        case 1u: { pigment = impact(p, t); }
        case 2u: { pigment = settle(p, t); }
        case 3u: { pigment = turn(p, t); }
        case 4u: { pigment = unfurl(p, t); }
        default: { pigment = stamp(p, t); }
    }
    let fade = smoothstep(0.0, 0.11, t) * (1.0 - smoothstep(0.18, 1.0, t));
    let edge = 1.0 - smoothstep(0.83, 1.0, max(abs(p.x), abs(p.y)));
    let color = mix(vec3<f32>(0.52, 0.40, 0.28), vec3<f32>(0.40, 0.29, 0.19),
        clamp(pigment, 0.0, 1.0));
    return vec4<f32>(color, clamp(pigment * fade * edge * ink.opacity, 0.0, 1.0));
}
