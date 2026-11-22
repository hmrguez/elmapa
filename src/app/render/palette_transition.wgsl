#define_import_path elmapa::palette_transition

struct PaletteTransition {
    uv: vec4<f32>,
    pigment: vec4<f32>,
};

fn blend(
    color: vec3<f32>,
    world: vec2<f32>,
    transition: PaletteTransition,
    palette: texture_2d<f32>,
    palette_sampler: sampler,
) -> vec3<f32> {
    if transition.uv.z == 0.0 {
        return color;
    }
    let uv = (world - transition.uv.xy) * transition.uv.zw;
    let ink = textureSampleLevel(palette, palette_sampler, uv, 0.0);
    if ink.a == 0.0 {
        return color;
    }
    // Shift the underlying pigment and keep the texture's local variation.
    // Normalize coverage so bilinear samples along the edge never pick up black.
    return color + (ink.rgb / ink.a - transition.pigment.rgb);
}
