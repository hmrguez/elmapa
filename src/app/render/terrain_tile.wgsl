#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::globals,
}
#import elmapa::dune_surface::dune_light
#import elmapa::grass_wind::grass_shade
#import elmapa::water_surface
#import elmapa::palette_transition::{PaletteTransition, blend}

// ShaderMode in state.rs
const SHADER_OFF = 0u;
const SHADER_ANIMATED = 2u;

struct Tile {
    opacity: f32,
    shader_mode: u32,
    profile: u32,
    transition: PaletteTransition,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> tile: Tile;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var pigment: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var pigment_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(3) var palette: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var palette_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let ink = textureSample(pigment, pigment_sampler, mesh.uv) * mesh.color;
    var color = ink.rgb;
    let point = mesh.world_position.xy * vec2(1.0, -1.0);
    if tile.shader_mode != SHADER_OFF {
        color = blend(color, point, tile.transition, palette, palette_sampler);
    }
    let moving = tile.shader_mode == SHADER_ANIMATED;
    if tile.shader_mode != SHADER_OFF && tile.profile != 0u
        && (moving || tile.profile == 101u || tile.profile == 103u) {
        let footprint = max(length(dpdx(point)), length(dpdy(point)));
        if tile.profile >= 1u && tile.profile <= 4u {
            color += water_surface::surface_light(tile.profile, point, globals.time, footprint);
        } else if tile.profile == 101u || tile.profile == 102u {
            let time = select(0.0, globals.time, moving);
            color *= grass_shade(point, footprint, time, tile.profile == 102u);
        } else if tile.profile == 103u {
            color += dune_light(point, footprint);
        }
    }
    return vec4(color, ink.a * tile.opacity);
}
