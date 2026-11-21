//! Prepare one scene at a time and publish only results for the current edit epoch.
use super::{
    render::{
        self,
        terrain_tile::TerrainTile,
        tile_upload::{SceneMesh, TileUploads},
        wet_edge::WetMeshes,
    },
    state::Studio,
};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use elmapa::{
    document::{Document, Stroke},
    scene::{Scene, SceneCache, WetEdge, generation},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct PreparedScene {
    scene: Scene,
    meshes: Vec<render::CpuMesh>,
    wet: Option<WetMeshes>,
    animate: bool,
}

struct StagedScene {
    epoch: u64,
    scene: Scene,
    meshes: Vec<SceneMesh>,
    wet: Option<WetMeshes>,
    animate: bool,
    ready: Arc<AtomicBool>,
}

struct Generated {
    cache: SceneCache,
    result: Result<PreparedScene, String>,
    epoch: u64,
}

pub(super) struct SceneWork {
    snapshot: Scene,
    meshes: Vec<SceneMesh>,
    pub(super) wet: Option<WetMeshes>,
    staged: Option<StagedScene>,
    cache: Option<SceneCache>,
    task: Option<Task<Generated>>,
    epoch: u64,
    dirty: bool,
    needs_sync: bool,
}

impl Default for SceneWork {
    fn default() -> Self {
        Self {
            snapshot: Scene::default(),
            meshes: Vec::new(),
            wet: None,
            staged: None,
            cache: Some(SceneCache::default()),
            task: None,
            epoch: 0,
            dirty: false,
            needs_sync: true,
        }
    }
}

impl SceneWork {
    #[cfg(test)]
    pub(super) fn set_test_snapshot(&mut self, snapshot: Scene) {
        self.snapshot = snapshot;
    }

    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(super) fn snapshot(&self) -> &Scene {
        &self.snapshot
    }

    pub(super) fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub(super) fn invalidate_preview(&mut self) {
        self.epoch += 1;
        self.staged = None;
        self.invalidate();
    }

    pub(super) fn is_pending(&self) -> bool {
        self.dirty || self.task.is_some() || self.staged.is_some() || self.needs_sync
    }

    pub(super) fn needs_sync(&self) -> bool {
        self.needs_sync
    }

    pub(super) fn request_sync(&mut self) {
        self.needs_sync = true;
    }

    pub(super) fn finish_sync(&mut self) {
        self.needs_sync = false;
    }

    pub(super) fn drain_meshes(&mut self) -> impl Iterator<Item = SceneMesh> + '_ {
        self.meshes.drain(..)
    }

    fn poll(&mut self) -> Option<Result<PreparedScene, String>> {
        let mut task = self.task.take()?;
        let Some(generated) = block_on(poll_once(&mut task)) else {
            self.task = Some(task);
            return None;
        };
        self.cache = Some(generated.cache);
        (generated.epoch == self.epoch).then_some(generated.result)
    }

    pub(super) fn publish(&mut self, scene: Scene, meshes: Vec<SceneMesh>) {
        self.snapshot = scene;
        self.meshes = meshes;
        self.request_sync();
    }

    fn start(
        &mut self,
        document: Document,
        live: Option<(u64, Stroke)>,
        animate: bool,
        wet_paint: Option<(u64, u64)>,
        known: HashSet<usize>,
    ) {
        let mut cache = self.cache.take().expect("ready scene worker");
        let epoch = self.epoch;
        self.dirty = false;
        self.task = Some(AsyncComputeTaskPool::get().spawn(async move {
            let _span = elmapa::performance::Span::new("scene_job");
            cache.begin_generation();
            let result = generation::bounded(generation::Limits::INTERACTIVE, None, || {
                let live = live.as_ref().map(|(layer, stroke)| (*layer, stroke));
                let (scene, wet) = if let Some((layer, id)) = wet_paint {
                    let (scene, edges) = cache.render_with_wet_edges(&document, live, layer);
                    let wet = prepare_wet(&scene, edges, (layer, id));
                    (scene, Some(wet))
                } else {
                    (cache.render(&document, live), None)
                };
                let meshes = scene
                    .batches()
                    .filter(|batch| {
                        scene.forest_tree(batch).is_none()
                            && !batch.is_empty()
                            && !known.contains(&(Arc::as_ptr(batch) as usize))
                    })
                    .map(|batch| {
                        render::tessellate_material(
                            batch.clone(),
                            scene.path_part(batch),
                            scene.river_reach(batch),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(PreparedScene {
                    scene,
                    meshes,
                    wet,
                    animate,
                })
            })
            .map_err(|e| e.to_string())
            .and_then(|result| result);
            cache.finish_generation(result.is_ok());
            Generated {
                cache,
                result,
                epoch,
            }
        }));
    }
}

fn prepare_wet(scene: &Scene, edges: Vec<WetEdge>, paint: (u64, u64)) -> WetMeshes {
    let mut placements: HashMap<_, _> = edges.iter().map(|edge| (edge.region, None)).collect();
    if !placements.is_empty() {
        for (order, batch) in scene.batches().enumerate() {
            if let Some(region) = scene.region(batch)
                && let Some(place) = placements.get_mut(&region)
            {
                *place = Some((order, scene.batch_opacity(batch)));
            }
        }
    }
    let meshes = edges
        .into_iter()
        .filter_map(|edge| {
            let (order, opacity) = placements[&edge.region]?;
            let mut cpu = render::tessellate_material(Arc::new(edge.shapes), None, None).ok()?;
            if let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
                cpu.mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
            {
                for color in colors {
                    color[3] *= opacity;
                }
            }
            Some((order, cpu.mesh))
        })
        .collect();
    WetMeshes { paint, meshes }
}

pub fn resolve_scene(
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    uploads: Res<TileUploads>,
    mut canvas: ResMut<render::Canvas>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainTile>>,
) {
    let published = publish_ready(&mut studio, time.elapsed_secs());
    if studio.scene.staged.is_some() {
        return;
    }
    if let Some(result) = studio.scene.poll() {
        match result {
            Ok(PreparedScene {
                scene,
                meshes,
                wet,
                animate,
            }) => {
                let (meshes, ready) = uploads.stage(
                    &scene,
                    meshes,
                    &mut canvas,
                    &mut mesh_assets,
                    &mut materials,
                    studio.reduced,
                );
                studio.scene.staged = Some(StagedScene {
                    epoch: studio.scene.epoch,
                    scene,
                    meshes,
                    wet,
                    animate,
                    ready,
                });
            }
            Err(error) => {
                if studio.installing {
                    studio.installing = false;
                    studio.scene.snapshot = Scene::default();
                    studio.scene.request_sync();
                }
                studio.status = format!("Could not prepare terrain: {error}");
                studio.ui_dirty = true;
            }
        }
    }
    if !published {
        publish_ready(&mut studio, time.elapsed_secs());
    }
}

fn publish_ready(studio: &mut Studio, now: f32) -> bool {
    let Some(staged) = studio
        .scene
        .staged
        .take_if(|staged| staged.ready.load(Ordering::Acquire))
    else {
        return false;
    };
    debug_assert_eq!(staged.epoch, studio.scene.epoch);
    if studio.installing {
        studio.installing = false;
        studio.status = "Welcome back to your world.".into();
        studio.ui_dirty = true;
    }
    let animate = staged.animate && !studio.reduced;
    studio.motion.sync(&staged.scene, animate, now);
    studio.mountain_motion.sync(&staged.scene, animate, now);
    studio.path_motion.sync(&staged.scene, animate, now);
    studio.river_motion.sync(&staged.scene, animate, now);
    studio.scene.wet = staged.wet;
    studio.scene.publish(staged.scene, staged.meshes);
    true
}

pub fn schedule_scene(mut studio: ResMut<Studio>, canvas: Res<render::Canvas>) {
    // Stage at most one snapshot ahead of what is displayed. The next CPU
    // job can use its mesh keys while the render world processes the upload.
    if studio.home || !studio.scene.dirty || studio.scene.task.is_some() || studio.scene.needs_sync
    {
        return;
    }
    let known = studio.scene.staged.as_ref().map_or_else(
        || canvas.mesh_keys(),
        |staged| {
            staged
                .scene
                .batches()
                .map(|batch| Arc::as_ptr(batch) as usize)
                .collect()
        },
    );
    let document = preview_document(&studio);
    let live = studio
        .stroke
        .clone()
        .map(|stroke| (studio.editor.active_layer(), stroke));
    let animate = studio.animate_new;
    let wet_paint = studio.wet_paint();
    studio
        .scene
        .start(document, live, animate, wet_paint, known);
}

fn preview_document(studio: &Studio) -> Document {
    let mut document = studio
        .preview_selection()
        .unwrap_or_else(|| studio.editor.document().clone());
    studio.preview_path(&mut document);
    if let Some(label) = &studio.text.preview {
        for layer in &mut document.layers {
            if studio.text.landmark_name {
                if let Some(mark) = layer.landmarks.iter_mut().find(|mark| mark.id == label.id) {
                    mark.set_name_label(label.clone());
                    break;
                }
            } else if let Some(target) = layer.labels.iter_mut().find(|l| l.id == label.id) {
                *target = label.clone();
                break;
            }
        }
    }
    if let Some(draft) = studio.text.draft
        && let Some(label) = studio
            .draft_preview_label()
            .filter(|label| label.validate().is_ok())
        && let Some(layer) = document
            .layers
            .iter_mut()
            .find(|layer| layer.id == draft.layer)
    {
        // ID zero is reserved for the uncommitted preview in this scene snapshot.
        layer.labels.push(label);
    }
    document
}
