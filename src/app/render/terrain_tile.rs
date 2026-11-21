//! Material shared by retained terrain fills and shoreline details.
use crate::app::state::Studio;
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
        embedded_asset!(app, "terrain_tile.wgsl");
        load_shader_library!(app, "water_surface.wgsl");
        app.add_plugins(Material2dPlugin::<TerrainTile>::default())
            .add_systems(PostUpdate, sync_motion);
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::app) struct TerrainTile {
    #[uniform(0)]
    pub opacity: f32,
    #[uniform(0)]
    motion: u32,
    #[uniform(0)]
    pub(super) profile: u32,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

impl TerrainTile {
    pub(super) fn new(texture: Option<Handle<Image>>, opacity: f32, reduced: bool) -> Self {
        Self {
            opacity,
            motion: u32::from(!reduced),
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

fn sync_motion(
    studio: Res<Studio>,
    mut previous: Local<bool>,
    mut tiles: ResMut<Assets<TerrainTile>>,
) {
    if *previous == studio.reduced {
        return;
    }
    *previous = studio.reduced;
    let motion = u32::from(!studio.reduced);
    let animated: Vec<_> = tiles
        .iter()
        .filter_map(|(id, tile)| (tile.profile != 0).then_some(id))
        .collect();
    for id in animated {
        tiles.get_mut(id).expect("animated tile").motion = motion;
    }
}
