//! Batch all forest specimens on one mesh and deform them on the GPU.
use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::primitives::Aabb,
    mesh::{Indices, MeshTag, PrimitiveTopology},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use elmapa::scene::{FOREST_SPECIMEN_COUNT, ForestTree};

// Keep the uniform array length in forest_wind.wgsl in sync with the atlas.
const _: () = assert!(FOREST_SPECIMEN_COUNT == 19);

pub struct ForestWindPlugin;

impl Plugin for ForestWindPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "forest_wind.wgsl");
        app.add_plugins(Material2dPlugin::<ForestWind>::default());
    }
}

#[derive(Clone, Debug, ShaderType)]
pub(super) struct Specimen {
    /// Bottom-left and top-right in local coordinates relative to the trunk.
    pub bounds: Vec4,
    /// Top-left and bottom-right in atlas UV coordinates.
    pub uv: Vec4,
}

#[derive(Clone, Debug, ShaderType)]
pub(super) struct WindParams {
    pub enabled: u32,
    pub specimens: [Specimen; FOREST_SPECIMEN_COUNT],
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub(super) struct ForestWind {
    #[uniform(0)]
    pub(super) params: WindParams,
    #[texture(1)]
    #[sampler(2)]
    pub(super) atlas: Handle<Image>,
}

impl Material2d for ForestWind {
    fn vertex_shader() -> ShaderRef {
        "embedded://elmapa/app/render/forest_wind.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

pub(super) fn mesh() -> Mesh {
    const STRIPS: u32 = 8;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for row in 0..=STRIPS {
        let y = row as f32 / STRIPS as f32;
        positions.extend([[0., y, 0.], [1., y, 0.]]);
    }
    for row in 0..STRIPS {
        let i = row * 2;
        indices.extend([i, i + 1, i + 2, i + 1, i + 3, i + 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

pub(super) fn instance_tag(tree: &ForestTree) -> MeshTag {
    // The specimen index and normalized opacity travel in the instance data.
    let opacity = (tree.opacity.clamp(0., 1.) * f32::from(u16::MAX)).round() as u32;
    MeshTag((opacity << 16) | tree.variant as u32)
}

pub(super) fn bounds() -> Aabb {
    // The shader maps the unit mesh into the 100-unit specimen viewBox and
    // adds up to 1.2 units of horizontal sway on either side.
    Aabb::from_min_max(Vec3::new(-52., -50., 0.), Vec3::new(52., 50., 0.))
}
