#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::globals,
}
#import elmapa::dune_wind::dune_light
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
    if tile.profile != 0u {
        let point = mesh.world_position.xy * vec2(1.0, -1.0);
        let footprint = max(length(dpdx(point)), length(dpdy(point)));
        let moving = tile.motion != 0u;
        let time = select(0.0, globals.time, moving);
        switch tile.profile {
            case 1u: {
                color += water_surface::surface_light(tile.profile, point, time, footprint);
            }
            case 101u: {
                color *= grass_shade(point, footprint, time, false);
            }
            case 103u: {
                color += dune_light(point, time, footprint);
            }
            case 2u, 3u, 4u, 5u: {
                if moving {
                    color += water_surface::surface_light(tile.profile, point, time, footprint);
                }
            }
            case 102u: {
                if moving {
                    color *= grass_shade(point, footprint, time, true);
                }
            }
            default: {}
        }
    }
    return vec4(color, ink.a * tile.opacity);
}
