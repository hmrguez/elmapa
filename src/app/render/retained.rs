//! Reconcile Bevy entities with the latest published scene.
use super::{
    Canvas, Graphics, WorldContent, forest_wind,
    motion::{
        Dropping, Flowing, Growing, OpeningPath, Rising, label_transform, mountain_transform,
        path_transform, tree_transform,
    },
    terrain_tile::TerrainTile,
};
use crate::app::{landmark_render, river_material::RiverMaterial, state::Studio};
use bevy::prelude::*;
use elmapa::scene::{ForestTree, Shape};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub(super) enum TerrainMaterial {
    Tile {
        handle: Handle<TerrainTile>,
        opacity: f32,
    },
    Ink(Handle<ColorMaterial>),
    River(Handle<RiverMaterial>),
}
pub(super) struct RetainedTree {
    pub(super) entity: Entity,
    pub(super) tree: ForestTree,
    pub(super) order: usize,
}
pub(super) struct RetainedMesh {
    pub(super) entity: Entity,
    pub(super) _source: Arc<Vec<Shape>>,
    pub(super) material: TerrainMaterial,
}

pub fn sync(
    graphics: Graphics,
    mut studio: ResMut<Studio>,
    mut canvas: ResMut<Canvas>,
    time: Res<Time<Real>>,
    mut tiles: ResMut<Assets<TerrainTile>>,
    window: Single<&Window>,
    mut transforms: Query<&mut Transform, With<WorldContent>>,
    mut visibility: Query<&mut Visibility, With<WorldContent>>,
) {
    let Graphics {
        mut commands,
        mut images,
        meshes: _,
        mut materials,
        mut rivers,
        mut forest_materials,
    } = graphics;
    let _span = elmapa::performance::Span::new("bevy_scene_sync");
    let atlas_changed =
        canvas
            .forest_atlas
            .poll(&mut images, &mut forest_materials, !studio.reduced);
    if atlas_changed {
        studio.scene.request_sync();
    }
    if canvas.view_replay.is_none() {
        for mut visible in &mut visibility {
            let desired = if studio.home || studio.installing || studio.opening.is_some() {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            if *visible != desired {
                *visible = desired;
            }
        }
    }
    if !studio.home {
        canvas.forest_atlas.request(
            studio.viewport,
            studio.bounds,
            window.scale_factor(),
            studio.scene.snapshot(),
            studio.scene.needs_sync(),
        );
    }
    if studio.home || !studio.scene.needs_sync() {
        return;
    }
    if !canvas.forest_atlas.matches(studio.scene.snapshot().style()) {
        return;
    }
    let Some(forest_material) = canvas.forest_atlas.material().cloned() else {
        return;
    };
    let new_meshes: HashMap<_, _> = studio
        .scene
        .drain_meshes()
        .map(|mesh| (Arc::as_ptr(&mesh.source) as usize, mesh))
        .collect();
    let mut new_meshes = new_meshes;
    let mut tree_ids = HashSet::new();
    let mut mesh_ids = HashSet::new();
    let scene = studio.scene.snapshot().clone();
    for (order, source) in scene.batches().enumerate() {
        let z = order as f32 * 0.0001;
        if let Some(tree) = scene.forest_tree(source) {
            tree_ids.insert(tree.id);
            let factor = studio.motion.factor(tree.id, time.elapsed_secs());
            let transform = tree_transform(tree, z, factor);
            if let Some(retained) = canvas.trees.get_mut(&tree.id) {
                let changed = retained.tree != *tree || retained.order != order;
                if changed {
                    let entity = retained.entity;
                    if let Ok(mut old) = transforms.get_mut(entity)
                        && *old != transform
                    {
                        *old = transform;
                    }
                    if retained.tree.variant != tree.variant
                        || retained.tree.opacity != tree.opacity
                    {
                        commands
                            .entity(entity)
                            .insert(forest_wind::instance_tag(tree));
                    }
                    if factor != 1. {
                        commands
                            .entity(entity)
                            .insert(Growing { tree: tree.clone() });
                    }
                    retained.tree = tree.clone();
                    retained.order = order;
                }
            } else {
                let entity = commands
                    .spawn((
                        WorldContent,
                        Mesh2d(canvas.tree_mesh.clone()),
                        MeshMaterial2d(forest_material.clone()),
                        forest_wind::instance_tag(tree),
                        forest_wind::bounds(),
                        transform,
                    ))
                    .id();
                if factor != 1. {
                    commands
                        .entity(entity)
                        .insert(Growing { tree: tree.clone() });
                }
                canvas.trees.insert(
                    tree.id,
                    RetainedTree {
                        entity,
                        tree: tree.clone(),
                        order,
                    },
                );
            }
        } else if !source.is_empty() {
            let key = Arc::as_ptr(source) as usize;
            mesh_ids.insert(key);
            if canvas.meshes.contains_key(&key) {
                let tile_material = match &canvas.meshes[&key].material {
                    TerrainMaterial::Tile { opacity, .. }
                        if *opacity != scene.batch_opacity(source) =>
                    {
                        let texture = source.first().and_then(Shape::texture_index);
                        Some(canvas.tile_material(
                            texture,
                            scene.batch_opacity(source),
                            &mut tiles,
                            studio.reduced,
                        ))
                    }
                    _ => None,
                };
                let retained = canvas.meshes.get_mut(&key).expect("retained mesh");
                if let Ok(mut transform) = transforms.get_mut(retained.entity) {
                    let desired = if let Some(part) = scene.mountain_part(source) {
                        mountain_transform(
                            part,
                            z,
                            studio.mountain_motion.pose(part, time.elapsed_secs()),
                        )
                    } else if let Some(part) = scene.path_part(source) {
                        path_transform(part, z, studio.path_motion.pose(part, time.elapsed_secs()))
                    } else {
                        label_transform(&scene, source, &mut studio, time.elapsed_secs(), z)
                    };
                    if *transform != desired {
                        *transform = desired;
                    }
                }
                if let Some(part) = scene.path_part(source) {
                    commands
                        .entity(retained.entity)
                        .insert(OpeningPath(part.clone()));
                }
                match &mut retained.material {
                    TerrainMaterial::Tile { handle, opacity } => {
                        if let Some(new) = tile_material {
                            commands
                                .entity(retained.entity)
                                .insert(MeshMaterial2d(new.clone()));
                            *handle = new;
                            *opacity = scene.batch_opacity(source);
                        }
                    }
                    TerrainMaterial::Ink(handle) => {
                        let factor = scene.landmark_part(source).map_or(1., |part| {
                            let elapsed = studio
                                .landmarks
                                .arrivals
                                .get(&part.landmark.id)
                                .and_then(|s| *s)
                                .map_or(2., |s| time.elapsed_secs() - s);
                            landmark_render::alpha(part, elapsed, studio.reduced)
                        });
                        let desired =
                            Color::srgba(1., 1., 1., scene.batch_opacity(source) * factor);
                        if let Some(part) = scene.landmark_part(source)
                            && let Some(Some(started)) =
                                studio.landmarks.arrivals.get(&part.landmark.id)
                        {
                            commands
                                .entity(retained.entity)
                                .insert(landmark_render::Assembling {
                                    part: part.clone(),
                                    material: handle.clone(),
                                    opacity: scene.batch_opacity(source),
                                    started: *started,
                                });
                        }
                        if let Some(mut material) = materials.get_mut(&*handle)
                            && material.color != desired
                        {
                            material.color = desired;
                        }
                    }
                    TerrainMaterial::River(handle) => {
                        if let Some(mut material) = rivers.get_mut(&*handle)
                            && material.flow.z != scene.batch_opacity(source)
                        {
                            material.flow.z = scene.batch_opacity(source);
                        }
                        if let Some(reach) = scene.river_reach(source)
                            && studio
                                .river_motion
                                .pose(reach, time.elapsed_secs())
                                .is_some()
                        {
                            commands.entity(retained.entity).insert(Flowing {
                                reach: reach.clone(),
                                material: handle.clone(),
                            });
                        }
                    }
                }
            } else if let Some(prepared) = new_meshes.remove(&key) {
                let entity = commands
                    .spawn((
                        WorldContent,
                        Mesh2d(prepared.mesh),
                        if let Some(part) = scene.mountain_part(source) {
                            mountain_transform(
                                part,
                                z,
                                studio.mountain_motion.pose(part, time.elapsed_secs()),
                            )
                        } else if let Some(part) = scene.path_part(source) {
                            path_transform(
                                part,
                                z,
                                studio.path_motion.pose(part, time.elapsed_secs()),
                            )
                        } else {
                            label_transform(&scene, source, &mut studio, time.elapsed_secs(), z)
                        },
                    ))
                    .id();
                if let Some(bounds) = prepared.bounds {
                    commands.entity(entity).insert(bounds);
                }
                let material = if let Some(material) = prepared.tile {
                    commands
                        .entity(entity)
                        .insert(MeshMaterial2d(material.clone()));
                    TerrainMaterial::Tile {
                        handle: material,
                        opacity: scene.batch_opacity(source),
                    }
                } else if let Some(reach) = scene.river_reach(source) {
                    let pose = studio.river_motion.pose(reach, time.elapsed_secs());
                    let handle = rivers.add(RiverMaterial::new(
                        reach,
                        pose,
                        time.elapsed_secs(),
                        scene.batch_opacity(source),
                    ));
                    commands
                        .entity(entity)
                        .insert(MeshMaterial2d(handle.clone()));
                    if pose.is_some() {
                        commands.entity(entity).insert(Flowing {
                            reach: reach.clone(),
                            material: handle.clone(),
                        });
                    }
                    TerrainMaterial::River(handle)
                } else {
                    let texture = source
                        .first()
                        .and_then(Shape::texture_index)
                        .and_then(|index| canvas.brush_textures.get(&index).cloned());
                    let handle = materials.add(ColorMaterial {
                        color: Color::srgba(1., 1., 1., scene.batch_opacity(source)),
                        texture,
                        ..default()
                    });
                    commands
                        .entity(entity)
                        .insert(MeshMaterial2d(handle.clone()));
                    TerrainMaterial::Ink(handle)
                };
                if let Some(part) = scene.mountain_part(source)
                    && studio.mountain_motion.pose(part, time.elapsed_secs()) != (1., 0.)
                {
                    commands.entity(entity).insert(Rising(part.clone()));
                }
                if let Some(label) = scene.label(source)
                    && let Some((id, started)) = studio.text.drop
                    && id == label.id
                    && time.elapsed_secs() - started < 0.3
                {
                    commands.entity(entity).insert(Dropping {
                        center: label.position,
                        started,
                    });
                }
                if let Some(part) = scene.path_part(source) {
                    commands.entity(entity).insert(OpeningPath(part.clone()));
                }
                let landmark_part = scene.landmark_part(source);
                if let Some(part) = landmark_part {
                    commands
                        .entity(entity)
                        .insert(landmark_render::PartPosition::new(&part.landmark));
                } else if let Some(label) = scene.label(source)
                    && studio.editor.landmark(label.id).is_some()
                {
                    commands
                        .entity(entity)
                        .insert(landmark_render::PartPosition::for_name(label));
                }
                if let Some(part) = landmark_part
                    && let Some(Some(started)) = studio.landmarks.arrivals.get(&part.landmark.id)
                    && let TerrainMaterial::Ink(handle) = &material
                {
                    if let Some(mut m) = materials.get_mut(handle) {
                        m.color = Color::srgba(
                            1.,
                            1.,
                            1.,
                            scene.batch_opacity(source)
                                * landmark_render::alpha(
                                    part,
                                    time.elapsed_secs() - started,
                                    studio.reduced,
                                ),
                        );
                    }
                    commands.entity(entity).insert(landmark_render::Assembling {
                        part: part.clone(),
                        material: handle.clone(),
                        opacity: scene.batch_opacity(source),
                        started: *started,
                    });
                }
                canvas.meshes.insert(
                    key,
                    RetainedMesh {
                        entity,
                        _source: prepared.source,
                        material,
                    },
                );
            }
        }
    }
    canvas.trees.retain(|id, retained| {
        if tree_ids.contains(id) {
            true
        } else {
            commands.entity(retained.entity).despawn();
            false
        }
    });
    canvas.meshes.retain(|id, mesh| {
        if mesh_ids.contains(id) {
            true
        } else {
            commands.entity(mesh.entity).despawn();
            false
        }
    });
    studio.scene.finish_sync();
    elmapa::performance::counter("bevy_tree_entities", canvas.trees.len() as u64);
    elmapa::performance::counter("bevy_mesh_entities", canvas.meshes.len() as u64);
}
