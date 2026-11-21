#import bevy_sprite::mesh2d_vertex_output::VertexOutput
#import bevy_sprite::mesh2d_view_bindings::globals
#import elmapa::grass_wind::grass_shade

struct Tile {
    opacity: f32,
    motion: u32,
    profile: u32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> tile: Tile;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var pigment: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var pigment_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let ink = textureSample(pigment, pigment_sampler, mesh.uv) * mesh.color;
    var shade = 1.0;
    if tile.motion != 0u && (tile.profile == 101u || tile.profile == 102u) {
        let world = mesh.world_position.xy * vec2<f32>(1.0, -1.0);
        let pixel = max(length(dpdx(world)), length(dpdy(world)));
        shade = grass_shade(world, pixel, globals.time, tile.profile == 102u);
    }
    return vec4(ink.rgb * shade, ink.a * tile.opacity);
}
