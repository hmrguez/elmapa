//! Keep the orthographic camera and world-anchored paper in sync with the viewport.
use super::{Canvas, WorldCamera, WorldContent, mesh::tessellate};
use crate::app::state::Studio;
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, block_on, poll_once},
};
use elmapa::document::{ContentBounds, Point};
use std::sync::Arc;

pub fn camera(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    mut canvas: ResMut<Canvas>,
    mut camera: Single<(&mut Transform, &mut Projection), With<WorldCamera>>,
    window: Single<&Window>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let size = Vec2::new(window.width(), window.height());
    if studio.bounds.size != size {
        studio.bounds.size = size;
        studio.ui_dirty = true;
    }
    let bounds = studio.bounds;
    studio.viewport.resolve(bounds);
    let view = studio.viewport.transform(bounds);
    let center = view.map_point(bounds.center());
    let desired = Vec3::new(center.x, -center.y, camera.0.translation.z);
    if camera.0.translation != desired {
        camera.0.translation = desired;
    }
    if let Projection::Orthographic(ref mut p) = *camera.1 {
        let scale = 1. / (view.scale * window.scale_factor());
        if p.scale != scale {
            p.scale = scale;
        }
    }
    if studio.home {
        return;
    }
    if let Some(mut job) = canvas.paper_job.take() {
        if let Some(result) = block_on(poll_once(&mut job)) {
            match result {
                Ok(cpu) => {
                    if let Some(entity) = canvas.paper.take() {
                        commands.entity(entity).despawn();
                    }
                    canvas.paper = Some(
                        commands
                            .spawn((
                                WorldContent,
                                Mesh2d(meshes.add(cpu.mesh)),
                                MeshMaterial2d(canvas.paper_material.clone()),
                                Transform::from_xyz(0., 0., 101.),
                            ))
                            .id(),
                    );
                }
                Err(error) => {
                    studio.status = format!("Could not render parchment: {error}");
                    studio.ui_dirty = true;
                }
            }
        } else {
            canvas.paper_job = Some(job);
        }
    }
    let min = view.map_point(Vec2::ZERO);
    let max = view.map_point(size);
    let snapped = (
        (min.x / 64.).floor() as i32,
        (min.y / 64.).floor() as i32,
        (max.x / 64.).ceil() as i32,
        (max.y / 64.).ceil() as i32,
    );
    if canvas.paper_job.is_none() && canvas.paper_bounds != Some(snapped) {
        canvas.paper_bounds = Some(snapped);
        canvas.paper_job = Some(AsyncComputeTaskPool::get().spawn(async move {
            tessellate(Arc::new(elmapa::scene::paper_detail(ContentBounds {
                min: Point::new(snapped.0 as f32 * 64., snapped.1 as f32 * 64.),
                max: Point::new(snapped.2 as f32 * 64., snapped.3 as f32 * 64.),
            })))
        }));
    }
}
