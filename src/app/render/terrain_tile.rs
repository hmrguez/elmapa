//! Material shared by retained terrain fills and shoreline details.
use crate::app::state::{ShaderMode, Studio};
use bevy::{
    asset::embedded_asset,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::{ShaderRef, load_shader_library},
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};

pub(super) struct TerrainTilePlugin;

impl Plugin for TerrainTilePlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "dune_surface.wgsl");
        load_shader_library!(app, "grass_wind.wgsl");
        embedded_asset!(app, "terrain_tile.wgsl");
        load_shader_library!(app, "water_surface.wgsl");
        app.add_plugins(Material2dPlugin::<TerrainTile>::default())
            .add_systems(PostUpdate, sync_shader_mode);
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::app) struct TerrainTile {
    #[uniform(0)]
    pub opacity: f32,
    #[uniform(0)]
    shader_mode: u32,
    #[uniform(0)]
    pub(super) profile: u32,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

impl TerrainTile {
    pub(super) fn new(
        texture: Option<Handle<Image>>,
        opacity: f32,
        shader_mode: ShaderMode,
    ) -> Self {
        Self {
            opacity,
            shader_mode: shader_mode as u32,
            profile: 0,
            texture,
        }
    }
}

impl Material2d for TerrainTile {
    fn fragment_shader() -> ShaderRef {
        "embedded://elmapa/app/render/terrain_tile.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

fn sync_shader_mode(
    studio: Res<Studio>,
    mut previous: Local<Option<ShaderMode>>,
    mut tiles: ResMut<Assets<TerrainTile>>,
) {
    let mode = studio.shader_mode();
    if *previous == Some(mode) {
        return;
    }
    *previous = Some(mode);
    let procedural: Vec<_> = tiles
        .iter()
        .filter_map(|(id, tile)| (tile.profile != 0).then_some(id))
        .collect();
    for id in procedural {
        tiles.get_mut(id).expect("procedural tile").shader_mode = mode as u32;
    }
}
