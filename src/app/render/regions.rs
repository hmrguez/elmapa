//! Immediate selection and draft ink above the retained map.
use super::{Graphics, mesh::tessellate};
use crate::app::state::Studio;
use bevy::prelude::*;
use elmapa::{document::Point, scene::Shape};
use std::sync::Arc;

#[derive(Default)]
pub struct Overlay {
    key: Option<(u64, u32, bool)>,
    entity: Option<Entity>,
    material: Handle<ColorMaterial>,
}

pub fn region_overlay(mut graphics: Graphics, studio: Res<Studio>, mut overlay: Local<Overlay>) {
    let show = studio.regions.active
        && !studio.home
        && !studio.installing
        && studio.opening.is_none()
        && !studio.replay_view;
    let scale = studio.viewport.zoom.max(0.0001);
    let key = show.then_some((
        studio.regions.overlay_revision,
        scale.to_bits(),
        studio.dark,
    ));
    if overlay.key == key {
        return;
    }
    overlay.key = key;
    if let Some(entity) = overlay.entity.take() {
        graphics.commands.entity(entity).despawn();
    }
    if !show {
        return;
    }
    let ink = 0x23614e;
    let split = studio.regions.split_preview();
    let mut shapes = split.map_or_else(Vec::new, |(_, washes)| washes.to_vec());
    if let Some(selection) = &studio.regions.selection {
        for ring in &selection.outline {
            let path: Vec<_> = ring.iter().chain(ring.first()).copied().collect();
            shapes.push(Shape::Line {
                points: path.clone(),
                width: 4. / scale,
                color: 0xf9ecd0,
                opacity: 0.85,
            });
            shapes.push(Shape::Line {
                points: path,
                width: 1.6 / scale,
                color: ink,
                opacity: 1.,
            });
        }
    }
    if let Some((view, _)) = split {
        for span in view.borders.iter().flat_map(|border| &border.visible) {
            dashes(&mut shapes, span, scale, ink);
        }
    }
    if let Some(path) = studio.regions.draft() {
        dashes(&mut shapes, path, scale, ink);
    }
    if let Some(point) = studio
        .regions
        .handle(studio.pointer.map(|p| studio.map_point(p)))
    {
        for (radius, fill) in [(5., ink), (3., 0xf9ecd0)] {
            let points = (0..16)
                .map(|index| {
                    let angle = index as f32 * std::f32::consts::TAU / 16.;
                    Point::new(
                        point.x + angle.cos() * radius / scale,
                        point.y + angle.sin() * radius / scale,
                    )
                })
                .collect();
            shapes.push(Shape::Polygon {
                points,
                fill,
                opacity: 1.,
            });
        }
    }
    if shapes.is_empty() {
        return;
    }
    let Ok(cpu) = tessellate(Arc::new(shapes)) else {
        return;
    };
    if overlay.material == Handle::default() {
        overlay.material = graphics.materials.add(ColorMaterial::default());
    }
    overlay.entity = Some(
        graphics
            .commands
            .spawn((
                Mesh2d(graphics.meshes.add(cpu.mesh)),
                MeshMaterial2d(overlay.material.clone()),
                Transform::from_xyz(0., 0., 125.),
            ))
            .id(),
    );
}

fn dashes(out: &mut Vec<Shape>, path: &[Point], scale: f32, color: u32) {
    let mut along: f32 = 0.;
    for pair in path.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let length = (b.x - a.x).hypot(b.y - a.y);
        let period = (10. / scale).max(length / 96.);
        let mut pos = 0.;
        while pos < length {
            let phase = along % period;
            let visible = phase < period * 0.62;
            let remaining = if visible {
                period * 0.62 - phase
            } else {
                period - phase
            };
            let end = (pos + remaining.max(period * 0.001)).min(length);
            if end <= pos {
                break;
            }
            if visible {
                let at = |d: f32| {
                    Point::new(
                        a.x + (b.x - a.x) * d / length,
                        a.y + (b.y - a.y) * d / length,
                    )
                };
                out.push(Shape::Line {
                    points: vec![at(pos), at(end)],
                    color,
                    width: 2.7 / scale,
                    opacity: 1.,
                });
            }
            along += end - pos;
            pos = end;
        }
    }
}
