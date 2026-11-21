//! Retained world-space meshes and atlas-backed sprites for the native canvas.
mod camera;
mod dune_wind;
mod forest_atlas;
mod forest_wind;
mod grass_wind;
mod mesh;
mod motion;
mod opening;
mod path_guide;
mod regions;
mod retained;
pub(super) mod terrain_tile;
pub(super) mod tile_upload;
pub(super) mod tree_leaves;
pub(super) mod view_replay;
mod water_surface;
pub(super) mod wet_edge;

use super::river_material::RiverMaterial;
use bevy::{
    asset::{RenderAssetUsages, StrongHandle},
    ecs::system::SystemParam,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::Task,
};
use elmapa::{
    brush_materials,
    document::AtlasStyle,
    scene::{Shape, TreeId, granulation},
};
use forest_atlas::ForestAtlas;
use forest_wind::ForestWind;
use retained::{RetainedMesh, RetainedTree};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
};
use terrain_tile::TerrainTile;

pub use camera::camera;
pub use forest_wind::ForestWindPlugin;
pub use mesh::{CpuMesh, tessellate_material};
pub use motion::{drop_text, flow, grow, open_path, rise};
pub use opening::opening;
pub use path_guide::path_guide;
pub use regions::region_overlay;
pub use retained::sync;

#[derive(Component)]
pub struct WorldCamera;

#[derive(Component)]
pub struct WorldContent;

#[derive(Resource, Default)]
pub struct Canvas {
    meshes: HashMap<usize, RetainedMesh>,
    trees: HashMap<TreeId, RetainedTree>,
    forest_atlas: ForestAtlas,
    tree_mesh: Handle<Mesh>,
    paper_bounds: Option<(i32, i32, i32, i32)>,
    paper_job: Option<Task<Result<CpuMesh, String>>>,
    paper: Option<Entity>,
    paper_material: Handle<ColorMaterial>,
    pub brush_textures: HashMap<usize, Handle<Image>>,
    granulation: [Handle<Image>; 2],
    tile_materials: HashMap<(Option<AssetId<Image>>, u32), Weak<StrongHandle>>,
    view_replay: Option<view_replay::Playback>,
}
impl Canvas {
    pub fn mesh_keys(&self) -> HashSet<usize> {
        self.meshes.keys().copied().collect()
    }
    fn batch_texture(&self, batch: &[Shape], style: AtlasStyle) -> Option<Handle<Image>> {
        if let Some(index) = batch.first().and_then(Shape::texture_index) {
            return self.brush_textures.get(&index).cloned();
        }
        batch
            .iter()
            .any(Shape::granulated)
            .then(|| self.granulation[usize::from(!style.is_classic())].clone())
    }

    fn tile_material(
        &mut self,
        batch: &[Shape],
        style: AtlasStyle,
        opacity: f32,
        materials: &mut Assets<TerrainTile>,
        reduced: bool,
    ) -> Handle<TerrainTile> {
        let texture = batch.first().and_then(Shape::texture_index);
        let image = self.batch_texture(batch, style);
        let key = (image.as_ref().map(Handle::id), opacity.to_bits());
        if let Some(handle) = self.tile_materials.get(&key).and_then(Weak::upgrade) {
            return Handle::Strong(handle);
        }
        self.tile_materials
            .retain(|_, handle| handle.strong_count() != 0);
        let mut material = TerrainTile::new(image, opacity, reduced);
        material.profile = grass_wind::profile(texture)
            .or_else(|| dune_wind::profile(texture))
            .unwrap_or_else(|| water_surface::profile(texture));
        let handle = materials.add(material);
        if let Handle::Strong(strong) = &handle {
            self.tile_materials.insert(key, Arc::downgrade(strong));
        }
        handle
    }

    pub fn ready(&self) -> bool {
        self.forest_atlas.ready() && self.paper_job.is_none()
    }
}
pub fn setup(
    mut canvas: ResMut<Canvas>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    canvas.forest_atlas.initialize();
    canvas.tree_mesh = meshes.add(forest_wind::mesh());
    canvas.paper_material = materials.add(ColorMaterial::default());
    canvas.granulation = [AtlasStyle::Classic, AtlasStyle::Woodland].map(|style| {
        let size = granulation::TEXTURE_SIZE;
        let mut image = Image::new_uninit(
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.texture_descriptor.mip_level_count = size.ilog2() + 1;
        image.data = Some(granulation::texture_rgba(style));
        let mut sampler = ImageSamplerDescriptor::linear();
        sampler.set_address_mode(ImageAddressMode::Repeat);
        image.sampler = ImageSampler::Descriptor(sampler);
        images.add(image)
    });
    for (index, material) in brush_materials::artwork_catalog().iter().enumerate() {
        let Some(texture) = material.texture else {
            continue;
        };
        let pixels = image::load_from_memory_with_format(texture, image::ImageFormat::Png)
            .expect("valid brush texture PNG")
            .into_rgba8();
        let (width, height) = pixels.dimensions();
        // The atlas camera composites authored sRGB values before its final
        // presentation pass, just like the solid terrain pigments.
        let mut image = rgba_image(width, height, pixels.into_raw());
        let mut sampler = ImageSamplerDescriptor::linear();
        if material.sprites.is_none() {
            sampler.set_address_mode(ImageAddressMode::Repeat);
        }
        image.sampler = ImageSampler::Descriptor(sampler);
        canvas.brush_textures.insert(index, images.add(image));
    }
}

#[derive(SystemParam)]
pub struct Graphics<'w, 's> {
    commands: Commands<'w, 's>,
    images: ResMut<'w, Assets<Image>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
    tiles: ResMut<'w, Assets<TerrainTile>>,
    rivers: ResMut<'w, Assets<RiverMaterial>>,
    forest_materials: ResMut<'w, Assets<ForestWind>>,
}
pub fn rgba_image(width: u32, height: u32, pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

pub(super) fn straight_alpha(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha > 0 {
            for channel in &mut pixel[..3] {
                *channel = (u32::from(*channel) * 255 + alpha / 2)
                    .checked_div(alpha)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
    }
}
