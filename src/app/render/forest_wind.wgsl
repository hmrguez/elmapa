#import bevy_sprite::{
    mesh2d_functions as mesh,
    mesh2d_view_bindings::globals,
}

struct Specimen {
    bounds: vec4<f32>,
    uv: vec4<f32>,
};

struct WindParams {
    enabled: u32,
    specimens: array<Specimen, 19>, // FOREST_SPECIMEN_COUNT
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> wind: WindParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var atlas_sampler: sampler;

struct TreeVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) opacity: f32,
};

@vertex
fn vertex(
    @builtin(instance_index) instance: u32,
    @location(0) unit_position: vec3<f32>,
) -> TreeVertex {
    let tag = mesh::get_tag(instance);
    let specimen = wind.specimens[tag & 0xffffu];
    let world_from_local = mesh::get_world_from_local(instance);
    var point = mix(specimen.bounds.xy, specimen.bounds.zw, unit_position.xy);

    if wind.enabled != 0u {
        let trunk = world_from_local[3].xy;
        // Two waves travel through the grove. Whole cycles across the global
        // clock's one-hour wrap keep the sway continuous in long sessions.
        let cycles = globals.time / 3600.0;
        let breeze = sin(6.2831853 * (cycles * 547.0 - dot(trunk, vec2<f32>(0.0019, 0.0011))));
        let gust = sin(6.2831853 * (cycles * 853.0 - dot(trunk, vec2<f32>(0.0032, -0.0017))));
        // The roots and ground wash stay fixed. Sway increases up the crown.
        let height = clamp((point.y - 9.0) / 34.0, 0.0, 1.0);
        point.x += (0.92 * breeze + 0.28 * gust) * height * height;
    }

    var out: TreeVertex;
    out.position = mesh::mesh2d_position_local_to_clip(world_from_local, vec4(point, 0.0, 1.0));
    out.uv = mix(specimen.uv.xy, specimen.uv.zw, vec2(unit_position.x, 1.0 - unit_position.y));
    out.opacity = f32(tag >> 16u) / 65535.0;
    return out;
}

@fragment
fn fragment(tree: TreeVertex) -> @location(0) vec4<f32> {
    let ink = textureSample(atlas, atlas_sampler, tree.uv);
    return vec4(ink.rgb, ink.a * tree.opacity);
}
