//! Keep the selected course legible under its screen-sized nodes.
use super::{Graphics, mesh::tessellate};
use crate::app::state::Studio;
use bevy::prelude::*;
use elmapa::{
    document::{Point, SelectionId, SharedVec},
    scene::Shape,
};
use std::sync::Arc;

#[derive(Default)]
pub struct PathGuide {
    source: Vec<SharedVec<Point>>,
    scale: f32,
    delta: Vec2,
    entity: Option<Entity>,
    material: Handle<ColorMaterial>,
}

pub fn path_guide(mut graphics: Graphics, studio: Res<Studio>, mut guide: Local<PathGuide>) {
    let scale = studio.viewport.transform(studio.bounds).scale;
    let mut courses = Vec::new();
    if !studio.home && !studio.installing && studio.replay_view.is_none() {
        if let Some(path) = &studio.path_selection {
            courses.push(path.points());
        } else {
            courses.extend(studio.selection.items.iter().filter_map(|item| {
                if let SelectionId::Course(id) = item.id {
                    return Some(&studio.editor.linear_stroke(id)?.points);
                }
                None
            }));
        }
    }
    courses.retain(|points| points.len() >= 2);
    let offset = studio.selection.delta();
    let delta = Vec2::new(offset.x, offset.y);
    let transform = Transform::from_xyz(delta.x, -delta.y, 125.);
    let same = courses.len() == guide.source.len()
        && courses.iter().zip(&guide.source).all(|(a, b)| a.ptr_eq(b));
    if same && (courses.is_empty() || guide.scale == scale) {
        if guide.delta != delta {
            guide.delta = delta;
            if let Some(entity) = guide.entity {
                graphics.commands.entity(entity).insert(transform);
            }
        }
        return;
    }
    guide.scale = scale;
    guide.delta = delta;
    guide.source = courses.iter().map(|points| (*points).clone()).collect();
    if let Some(entity) = guide.entity.take() {
        graphics.commands.entity(entity).despawn();
    }
    if courses.is_empty() {
        return;
    }
    let mut shapes = Vec::with_capacity(courses.len() * 2);
    for points in courses {
        // A subpixel filter keeps tightly spaced freehand samples cheap to draw.
        let mut trace = vec![points[0]];
        for &point in points.iter().skip(1).take(points.len() - 2) {
            let last = trace.last().unwrap();
            if (point.x - last.x).hypot(point.y - last.y) * scale >= 1. {
                trace.push(point);
            }
        }
        trace.push(*points.last().unwrap());
        for (width, color, opacity) in [(2.8, 0x354b41, 0.76), (1.15, 0xfff6df, 0.95)] {
            shapes.push(Shape::Line {
                points: trace.clone(),
                width: width / scale,
                color,
                opacity,
            });
        }
    }
    let Ok(mesh) = tessellate(Arc::new(shapes)) else {
        return;
    };
    if guide.material == Handle::default() {
        guide.material = graphics.materials.add(ColorMaterial::default());
    }
    guide.entity = Some(
        graphics
            .commands
            .spawn((
                Mesh2d(graphics.meshes.add(mesh.mesh)),
                MeshMaterial2d(guide.material.clone()),
                transform,
            ))
            .id(),
    );
}
