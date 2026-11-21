//! Presentation-only transforms and clocks for arriving artwork.
use crate::app::{landmark_render, river_material::RiverMaterial, state::Studio};
use bevy::prelude::*;
use elmapa::{
    document::Point,
    scene::{ForestTree, MountainPart, PathPart, RiverReach, Shape},
};
use std::sync::Arc;

#[derive(Component)]
pub struct Growing {
    pub(super) tree: ForestTree,
}
#[derive(Component)]
pub struct Rising(pub(super) MountainPart);
#[derive(Component)]
pub struct OpeningPath(pub(super) PathPart);
#[derive(Component)]
pub struct Flowing {
    pub(super) reach: RiverReach,
    pub(super) material: Handle<RiverMaterial>,
}

pub(super) fn tree_transform(tree: &ForestTree, z: f32, factor: f32) -> Transform {
    Transform::from_xyz(tree.center.x, -tree.center.y, z)
        .with_rotation(Quat::from_rotation_z(-tree.rotation.to_radians()))
        .with_scale(Vec3::splat(tree.scale * factor))
}
pub fn grow(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    mut query: Query<(Entity, &Growing, &mut Transform)>,
) {
    if studio.home {
        return;
    }
    for (entity, growth, mut transform) in &mut query {
        let factor = studio.motion.factor(growth.tree.id, time.elapsed_secs());
        *transform = tree_transform(&growth.tree, transform.translation.z, factor);
        if factor == 1. {
            commands.entity(entity).remove::<Growing>();
        }
    }
    studio.motion.advance(time.elapsed_secs());
}

// Mesh positions remain in world coordinates. Translate after scaling so the
// shared ground pivot stays fixed while the two flanks converge and rise.
pub(super) fn mountain_transform(
    part: &MountainPart,
    z: f32,
    (height, shift): (f32, f32),
) -> Transform {
    Transform::from_xyz(shift, -part.base.y * (1. - height), z)
        .with_scale(Vec3::new(1., height, 1.))
}
pub fn rise(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    mut query: Query<(Entity, &Rising, &mut Transform)>,
) {
    if studio.reduced {
        studio.mountain_motion.reset();
    }
    if studio.home {
        return;
    }
    for (entity, rising, mut transform) in &mut query {
        let pose = studio.mountain_motion.pose(&rising.0, time.elapsed_secs());
        *transform = mountain_transform(&rising.0, transform.translation.z, pose);
        if pose == (1., 0.) {
            commands.entity(entity).remove::<Rising>();
        }
    }
    studio.mountain_motion.advance(time.elapsed_secs());
}

pub(super) fn path_transform(part: &PathPart, z: f32, (forward, across): (f32, f32)) -> Transform {
    let angle = -(part.end.y - part.start.y).atan2(part.end.x - part.start.x);
    Transform::from_xyz(part.start.x, -part.start.y, z)
        .with_rotation(Quat::from_rotation_z(angle))
        .with_scale(Vec3::new(forward, across, 1.))
}
pub fn open_path(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    mut query: Query<(Entity, &OpeningPath, &mut Transform)>,
) {
    if studio.reduced {
        studio.path_motion.reset();
    }
    if studio.home {
        return;
    }
    for (entity, opening, mut transform) in &mut query {
        let pose = studio.path_motion.pose(&opening.0, time.elapsed_secs());
        *transform = path_transform(&opening.0, transform.translation.z, pose);
        if pose == (1., 1.) {
            commands.entity(entity).remove::<OpeningPath>();
        }
    }
    studio.path_motion.advance(time.elapsed_secs());
}

pub fn flow(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    mut materials: ResMut<Assets<RiverMaterial>>,
    query: Query<(Entity, &Flowing)>,
) {
    if !studio.shader_motion() {
        studio.river_motion.reset();
    }
    if studio.home {
        return;
    }
    for (entity, flowing) in &query {
        let pose = studio
            .river_motion
            .pose(&flowing.reach, time.elapsed_secs());
        if let Some(mut material) = materials.get_mut(&flowing.material) {
            material.set_flow(pose, time.elapsed_secs());
        }
        if pose.is_none() {
            commands.entity(entity).remove::<Flowing>();
        }
    }
    studio.river_motion.advance(time.elapsed_secs());
}

#[derive(Component)]
pub struct Dropping {
    pub(super) center: Point,
    pub(super) started: f32,
}
/// A name lands like a small ink stamp: 8 units of descent and a quiet settle.
/// Only mesh presentation moves; the authoritative scene is always final.
pub(super) fn drop_transform(center: Point, elapsed: f32, z: f32) -> Transform {
    let t = (elapsed / 0.3).clamp(0., 1.);
    let ease = 1. - (1. - t).powi(3);
    let scale = 0.96 + 0.04 * ease + (std::f32::consts::PI * t).sin() * 0.012;
    Transform::from_xyz(
        center.x * (1. - scale),
        -center.y * (1. - scale) + 8. * (1. - ease),
        z,
    )
    .with_scale(Vec3::new(scale, scale, 1.))
}
pub(super) fn label_transform(
    scene: &elmapa::scene::Scene,
    source: &Arc<Vec<Shape>>,
    studio: &mut Studio,
    now: f32,
    z: f32,
) -> Transform {
    if let Some(part) = scene.landmark_part(source) {
        let reduced = studio.reduced;
        let elapsed = if let Some(start) = studio.landmarks.arrivals.get_mut(&part.landmark.id) {
            now - *start.get_or_insert(now)
        } else {
            2.
        };
        return landmark_render::transform(part, elapsed, reduced, z);
    }
    let Some(label) = scene.label(source) else {
        return Transform::from_xyz(0., 0., z);
    };
    if studio.text.animate == Some(label.id) {
        studio.text.animate = None;
        studio.text.drop = (!studio.reduced).then_some((label.id, now));
    }
    if !studio.reduced
        && let Some((id, start)) = studio.text.drop
        && id == label.id
    {
        return drop_transform(label.position, now - start, z);
    }
    Transform::from_xyz(0., 0., z)
}
pub fn drop_text(
    mut commands: Commands,
    studio: Res<Studio>,
    time: Res<Time<Real>>,
    mut labels: Query<(Entity, &Dropping, &mut Transform)>,
) {
    for (entity, drop, mut transform) in &mut labels {
        let elapsed = if studio.reduced {
            0.3
        } else {
            time.elapsed_secs() - drop.started
        };
        *transform = drop_transform(drop.center, elapsed, transform.translation.z);
        if elapsed >= 0.3 {
            commands.entity(entity).remove::<Dropping>();
        }
    }
}

#[cfg(test)]
mod text_animation_tests {
    use super::*;
    #[test]
    fn text_drop_settles_exactly_and_keeps_its_center_anchored() {
        let center = Point::new(-300., 700.);
        let start = drop_transform(center, 0., 2.);
        assert_eq!(start.scale.x, 0.96);
        let point = start.transform_point(Vec3::new(center.x, -center.y, 0.));
        assert!((point.x - center.x).abs() < 0.001);
        assert!((point.y + center.y - 8.).abs() < 0.001);
        for i in 0..=300 {
            let pose = drop_transform(center, i as f32 / 1000., 2.);
            assert!((0.96..=1.012).contains(&pose.scale.x));
        }
        assert_eq!(
            drop_transform(center, 0.3, 2.),
            Transform::from_xyz(0., 0., 2.)
        );
    }
}
