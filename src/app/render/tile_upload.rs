//! Retain the displayed terrain until all its replacement buffers and materials
//! have been prepared by the render world.
use super::{
    Canvas, CpuMesh,
    terrain_tile::{TerrainTile, TerrainTilePlugin},
};
use bevy::{
    camera::primitives::{Aabb, MeshAabb},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        mesh::{RenderMesh, allocator::MeshAllocator},
        render_asset::RenderAssets,
    },
    sprite_render::PreparedMaterial2d,
};
use elmapa::scene::{Scene, Shape};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

#[derive(Resource, Clone, Default)]
pub(in crate::app) struct TileUploads(Arc<Mutex<Option<Upload>>>);

struct Upload {
    assets: Vec<(AssetId<Mesh>, AssetId<TerrainTile>)>,
    ready: Weak<AtomicBool>,
}

pub(in crate::app) struct SceneMesh {
    pub source: Arc<Vec<Shape>>,
    pub mesh: Handle<Mesh>,
    pub bounds: Option<Aabb>,
    pub tile: Option<Handle<TerrainTile>>,
}

pub(in crate::app) fn install(app: &mut App) {
    app.add_plugins(TerrainTilePlugin);
    let uploads = TileUploads::default();
    app.sub_app_mut(RenderApp)
        .insert_resource(uploads.clone())
        .add_systems(Render, check.after(RenderSystems::PrepareAssets));
    app.insert_resource(uploads);
}

impl TileUploads {
    pub(in crate::app) fn stage(
        &self,
        scene: &Scene,
        prepared: Vec<CpuMesh>,
        canvas: &mut Canvas,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<TerrainTile>,
        reduced: bool,
    ) -> (Vec<SceneMesh>, Arc<AtomicBool>) {
        let mut assets = Vec::new();
        let mut bytes = 0;
        let staged = prepared
            .into_iter()
            .filter_map(|cpu| {
                let key = cpu.source.first().and_then(Shape::texture_index);
                let bounds = cpu.mesh.compute_aabb();
                let index_count = cpu.mesh.indices().map_or(0, |indices| indices.len());
                if index_count == 0 {
                    return None;
                }
                let size = (cpu.mesh.count_vertices() * (3 + 4 + 2) + index_count)
                    * std::mem::size_of::<u32>();
                let mesh = meshes.add(cpu.mesh);
                let tile = scene.region(&cpu.source).map(|_| {
                    let material = canvas.tile_material(
                        key,
                        scene.batch_opacity(&cpu.source),
                        materials,
                        reduced,
                    );
                    assets.push((mesh.id(), material.id()));
                    bytes += size;
                    material
                });
                Some(SceneMesh {
                    source: cpu.source,
                    mesh,
                    bounds,
                    tile,
                })
            })
            .collect();
        let ready = Arc::new(AtomicBool::new(assets.is_empty()));
        if !assets.is_empty() {
            elmapa::performance::counter("terrain_meshes_staged", assets.len() as u64);
            elmapa::performance::counter("terrain_mesh_bytes_staged", bytes as u64);
            *self.0.lock().expect("tile upload mailbox") = Some(Upload {
                assets,
                ready: Arc::downgrade(&ready),
            });
        }
        (staged, ready)
    }
}

fn check(
    uploads: Res<TileUploads>,
    meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    materials: Res<RenderAssets<PreparedMaterial2d<TerrainTile>>>,
) {
    let mut request = uploads.0.lock().expect("tile upload mailbox");
    let Some(upload) = &*request else { return };
    let Some(ready) = upload.ready.upgrade() else {
        *request = None;
        return;
    };
    if upload.assets.iter().all(|(mesh, material)| {
        meshes.get(*mesh).is_some()
            && allocator.mesh_vertex_slice(mesh).is_some()
            && allocator.mesh_index_slice(mesh).is_some()
            && materials.get(*material).is_some()
    }) {
        ready.store(true, Ordering::Release);
        *request = None;
    }
}
