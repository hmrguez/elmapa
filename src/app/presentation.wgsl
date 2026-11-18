#import bevy_sprite::mesh2d_vertex_output::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var canvas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var canvas_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let encoded = textureSample(canvas, canvas_sampler, in.uv).rgb;
    let linear = select(encoded / 12.92, pow((encoded + 0.055) / 1.055, vec3<f32>(2.4)), encoded > vec3<f32>(0.04045));
    return vec4<f32>(linear, 1.0);
}
