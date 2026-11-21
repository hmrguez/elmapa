#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> strength: f32;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let world = mesh.world_position.xy;
    let thread = world.x * 0.37 - world.y * 0.23;
    let grain = sin(thread) * sin(world.x * 0.17 + world.y * 0.48);
    let detail = 1.0 - smoothstep(0.4, 1.9, fwidth(thread));
    let evenness = 0.92 + 0.14 * grain * detail;
    let drying = smoothstep(0.0, 1.0, strength);
    return vec4<f32>(mesh.color.rgb, mesh.color.a * evenness * drying);
}
