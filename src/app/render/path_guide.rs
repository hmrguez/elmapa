//! Keep the selected course legible under its screen-sized nodes.
use super::{Graphics, mesh::tessellate};
use crate::app::state::Studio;
use bevy::prelude::*;
use elmapa::{
    document::{Point, SharedVec},
    scene::Shape,
};
use std::sync::Arc;

#[derive(Default)]
pub struct PathGuide {
    source: Option<(SharedVec<Point>, f32)>,
    entity: Option<Entity>,
    material: Handle<ColorMaterial>,
}

pub fn path_guide(mut graphics: Graphics, studio: Res<Studio>, mut guide: Local<PathGuide>) {
    let scale = studio.viewport.transform(studio.bounds).scale;
    let points = studio
        .path_selection
        .as_ref()
        .filter(|_| !studio.home && !studio.installing && studio.replay_view.is_none())
        .map(|selection| selection.points())
        .filter(|points| points.len() >= 2);
    if let Some(points) = points {
        if guide
            .source
            .as_ref()
            .is_some_and(|(old, zoom)| *zoom == scale && old.ptr_eq(points))
        {
            return;
        }
    } else if guide.source.is_none() {
        return;
    }
    guide.source = points.map(|points| (points.clone(), scale));
    if let Some(entity) = guide.entity.take() {
        graphics.commands.entity(entity).despawn();
    }
    let Some(points) = points else {
        return;
    };

    // A subpixel filter keeps tightly spaced freehand samples cheap to draw.
    let mut trace = vec![points[0]];
    for &point in points.iter().skip(1).take(points.len() - 2) {
        let last = trace.last().unwrap();
        if (point.x - last.x).hypot(point.y - last.y) * scale >= 1. {
            trace.push(point);
        }
    }
    trace.push(*points.last().unwrap());
    let shapes = [(2.4, 0x385749, 0.64), (1., 0xf9faf4, 0.88)]
        .into_iter()
        .map(|(width, color, opacity)| Shape::Line {
            points: trace.clone(),
            width: width / scale,
            color,
            opacity,
        })
        .collect();
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
                Transform::from_xyz(0., 0., 125.),
            ))
            .id(),
    );
}
