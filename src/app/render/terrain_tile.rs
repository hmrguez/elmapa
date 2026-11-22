//! Material shared by retained terrain fills and shoreline details.
use crate::app::{
    state::{ShaderMode, Studio},
    ui::color,
};
use bevy::{
    asset::embedded_asset,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::{ShaderRef, load_shader_library},
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use elmapa::document::ContentBounds;

pub(super) struct TerrainTilePlugin;

impl Plugin for TerrainTilePlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "dune_surface.wgsl");
        load_shader_library!(app, "grass_wind.wgsl");
        load_shader_library!(app, "palette_transition.wgsl");
        embedded_asset!(app, "terrain_tile.wgsl");
        load_shader_library!(app, "water_surface.wgsl");
        app.add_plugins(Material2dPlugin::<TerrainTile>::default())
            .add_systems(PostUpdate, sync_shader_mode);
    }
}

#[derive(ShaderType, Default, Debug, Clone, Copy)]
pub(super) struct PaletteTransition {
    uv: Vec4,
    pigment: Vec4,
}

impl PaletteTransition {
    pub(super) fn new(bounds: ContentBounds, pigment: u32) -> Self {
        Self {
            uv: Vec4::new(
                bounds.min.x,
                bounds.min.y,
                (bounds.max.x - bounds.min.x).recip(),
                (bounds.max.y - bounds.min.y).recip(),
            ),
            pigment: Vec4::from_array(color(pigment, 1.).to_linear().to_f32_array()),
        }
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
    #[uniform(0)]
    pub(super) transition: PaletteTransition,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    pub(super) palette: Option<Handle<Image>>,
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
            transition: PaletteTransition::default(),
            texture,
            palette: None,
        }
    }
    pub(super) fn palette_transition(&self) -> PaletteTransition {
        if self.shader_mode == ShaderMode::Off as u32 {
            PaletteTransition::default()
        } else {
            self.transition
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
    let affected: Vec<_> = tiles
        .iter()
        .filter_map(|(id, tile)| (tile.profile != 0 || tile.palette.is_some()).then_some(id))
        .collect();
    for id in affected {
        tiles.get_mut(id).expect("shader tile").shader_mode = mode as u32;
    }
}
