#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct Wash { value: vec4<f32>, placement: vec4<f32> };
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> wash: Wash;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var paper_ink: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var ink_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var arrival_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var arrival_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world = mesh.world_position.xy * vec2<f32>(1.0, -1.0);
    let axis = vec2<f32>(0.8, 0.6);
    let along = (dot(world, axis) - wash.value.z) / wash.value.w;
    let across = dot(world, vec2<f32>(-axis.y, axis.x));
    let scallop = 0.022 * sin(across * 0.071) + 0.012 * sin(across * 0.19);
    let front = wash.value.x * 1.42 - 0.21;
    var wet_edge = smoothstep(-0.15, 0.15, front - along + scallop);
    if wash.placement.z > 0.0 {
        let uv = (world - wash.placement.xy) * wash.placement.zw;
        let arrival = textureSampleLevel(arrival_texture, arrival_sampler, uv, 0.0).r;
        let progress = wash.value.x * 1.14 - 0.07;
        wet_edge = smoothstep(-0.045, 0.045, progress - arrival);
    }
    var ink = mesh.color * textureSample(paper_ink, ink_sampler, mesh.uv);
    ink.a *= wash.value.y * wet_edge;
    return ink;
}
