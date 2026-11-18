#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Normalized age, layer opacity and a per-tree phase.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    var alpha = 0.0;
    for (var i = 0; i < 3; i += 1) {
        let index = f32(i);
        let phase = params.z * 0.73 + index * 2.4;
        let t = clamp((params.x - 0.12 - 0.10 * index) / (0.60 + 0.03 * index), 0.0, 1.0);
        let side = select(1.0, -1.0, (i + i32(params.z)) % 2 == 0);
        let center = vec2<f32>(
            0.5 + side * (0.18 + 0.038 * index + 0.14 * t)
                + 0.039 * sin(phase + 9.0 * t) * t,
            0.27 + 0.06 * sin(phase) + 0.57 * t * (0.7 + 0.3 * t),
        );
        let angle = phase + 5.0 * t;
        let p = mesh.uv - center;
        let along = dot(p, vec2<f32>(cos(angle), sin(angle))) / (0.023 + 0.003 * index);
        let width = 0.011 * (0.45 + 0.55 * abs(cos(phase + 8.0 * t)));
        let across = dot(p, vec2<f32>(-sin(angle), cos(angle))) / width;
        let shape = abs(across) + 0.70 * (along * along - 1.0);
        let edge = max(0.5 * fwidth(shape), 0.001);
        let coverage = 1.0 - smoothstep(-edge, edge, shape);
        let fade = smoothstep(0.0, 0.2, t) * (1.0 - smoothstep(0.58, 1.0, t));
        alpha = max(alpha, coverage * fade);
    }
    return vec4<f32>(0.49, 0.37, 0.24, alpha * 0.62 * params.y);
}
