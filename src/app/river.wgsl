#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct Params { value: vec4<f32> };
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> flow: Params;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> span: Params;

fn bell(value: f32, width: f32) -> f32 {
    let x = value / width;
    return exp(-x * x);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // UV.y packs a surface class and a small signed transverse coordinate.
    // 0: water, 1: current ink, 2: bank, 3: native-only crest and spray.
    let surface = round(mesh.uv.y);
    let across = (mesh.uv.y - surface) * 8.0;
    let along = mesh.uv.x;
    var ink = mesh.color;
    ink.a *= flow.value.z;
    if flow.value.x < -100.0 {
        if surface > 2.5 { return vec4<f32>(0.0); }
        return ink;
    }
    let energy = flow.value.y;
    let front = flow.value.x;
    // The center noses forward; the sides curl back, like a small stream head.
    let bow = 2.8 * (1.0 - min(across * across, 1.6));
    let edge = front + bow + sin(across * 6.0 + front * 0.12) * 0.55;
    let behind = edge - along;
    let crest = bell(behind - 1.0, 1.55) * energy;
    if surface > 2.5 {
        let start = select(span.value.x, -8.0, span.value.x < 0.01);
        let end = span.value.y + select(0.0, 8.0, span.value.y >= span.value.z - 0.1);
        if along < start || along >= end { return vec4<f32>(0.0); }
        let broken = smoothstep(-0.2, 0.5, sin(across * 18.0 + front * 0.09));
        let rim = bell(behind, 0.60)
            * (1.0 - smoothstep(0.72, 1.12, abs(across))) * broken * 0.28;
        var drops = 0.0;
        for (var i = 0; i < 3; i += 1) {
            for (var j = 0; j < 2; j += 1) {
                let side = select(-1.0, 1.0, j == 1);
                let phase = fract(flow.value.w * 2.8 + f32(i) * 0.31 + f32(j) * 0.17);
                let advance = 1.2 + phase * 6.0;
                let lane = side * (0.45 + f32(i) * 0.14 + phase * 0.55);
                let delta = vec2<f32>((along - front - advance) / 0.82, (across - lane) / 0.055);
                let drop = (1.0 - smoothstep(0.45, 1.0, length(delta)))
                    * sin(phase * 3.14159) * 0.30;
                drops = max(drops, drop);
            }
        }
        // A pale crest sits on water; a faint mineral tint keeps pin-sized
        // droplets readable when they land briefly against bare parchment.
        let tint = select(ink.rgb, vec3<f32>(0.64, 0.68, 0.59), drops > rim);
        return vec4<f32>(tint, ink.a * max(rim, drops) * energy);
    }
    ink.a *= smoothstep(-1.2, 1.4, behind);
    if surface < 0.5 {
        let shoulder = bell(behind - 4.3, 2.6) * energy;
        let water = mix(ink.rgb, vec3<f32>(0.70, 0.75, 0.66), shoulder * 0.12);
        ink = vec4<f32>(mix(water, vec3<f32>(0.95, 0.93, 0.84), crest * 0.48), ink.a);
    } else if surface < 1.5 {
        // A short wake, not repeating highlights along the whole painted river.
        ink.a *= 1.0 - crest * 0.45;
    }
    return ink;
}
