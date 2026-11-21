#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::globals,
}
#import elmapa::grass_wind::grass_shade
#import elmapa::water_surface

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
    var color = ink.rgb;
    if tile.motion != 0u && tile.profile != 0u {
        let point = mesh.world_position.xy * vec2(1.0, -1.0);
        let footprint = max(length(dpdx(point)), length(dpdy(point)));
        if tile.profile >= 1u && tile.profile <= 5u {
            color += water_surface::surface_light(tile.profile, point, globals.time, footprint);
        } else if tile.profile == 101u || tile.profile == 102u {
            color *= grass_shade(point, footprint, globals.time, tile.profile == 102u);
        }
    }
    return vec4(color, ink.a * tile.opacity);
}
