//! Select visible relief and course artwork from the composed scene.
use super::{Batch, Scene};
use crate::{
    document::{ContentBounds, LinearStrokeId, Point, ReliefKey, SelectionId, Specimen},
    scene::{Shape, geometry::contains},
};
use std::collections::HashSet;

#[derive(Clone, Copy)]
pub struct TerrainHit {
    pub id: SelectionId,
    pub anchor: Point,
    pub bounds: ContentBounds,
}

impl Scene {
    pub(super) fn course_id(&self, batch: &Batch) -> Option<LinearStrokeId> {
        let (layer, index, seed, _) = self
            .path_part(batch)
            .map(|path| path.id)
            .or_else(|| self.river_reach(batch).map(|reach| reach.id))?;
        Some(LinearStrokeId { layer, index, seed })
    }

    fn relief_hit(&self, batch: &Batch) -> Option<TerrainHit> {
        if let Some(tree) = self.forest_tree(batch) {
            let (layer, stroke, dab, candidate) = tree.id;
            return Some(TerrainHit {
                id: SelectionId::Relief {
                    layer,
                    key: ReliefKey {
                        stroke,
                        specimen: Specimen::Tree { dab, candidate },
                    },
                    seed: tree.seed,
                },
                anchor: tree.center,
                bounds: tree.bounds(),
            });
        }
        let peak = self.mountain_part(batch)?;
        let (layer, stroke, seed, ordinal) = peak.id;
        Some(TerrainHit {
            id: SelectionId::Relief {
                layer,
                key: ReliefKey {
                    stroke,
                    specimen: Specimen::Peak(ordinal),
                },
                seed,
            },
            anchor: peak.base,
            bounds: peak.bounds,
        })
    }

    pub fn relief_at(&self, point: Point, tolerance: f32) -> Option<TerrainHit> {
        let pick = ContentBounds {
            min: point,
            max: point,
        };
        self.batches.iter().rev().find_map(|batch| {
            let hit = self.relief_hit(batch)?;
            if !hit.bounds.padded(tolerance).intersects(pick) {
                return None;
            }
            let inside = match batch.first() {
                Some(Shape::Polygon { points, .. }) => contains(points, point),
                Some(Shape::Sprite { points, .. }) => contains(points, point),
                _ => self
                    .forest_tree(batch)
                    .is_some_and(|tree| tree.contains(point, tolerance)),
            };
            inside.then_some(hit)
        })
    }

    pub fn terrain_in(&self, rect: ContentBounds) -> Vec<TerrainHit> {
        let mut seen = HashSet::new();
        let mut hits = Vec::new();
        for batch in self.batches.iter().rev() {
            if let Some(hit) = self.relief_hit(batch) {
                if hit.bounds.intersects(rect) && seen.insert(hit.id) {
                    hits.push(hit);
                }
            } else if let Some(course) = self.course_id(batch) {
                let id = SelectionId::Course(course);
                if seen.contains(&id) {
                    continue;
                }
                if let Some(bounds) = batch.iter().find_map(|shape| match shape {
                    Shape::Polygon { points, .. } => {
                        let bounds = shape.bounds()?;
                        (bounds.intersects(rect) && polygon_intersects(points, rect))
                            .then_some(bounds)
                    }
                    _ => None,
                }) {
                    seen.insert(id);
                    hits.push(TerrainHit {
                        id,
                        anchor: bounds.min,
                        bounds,
                    });
                }
            }
        }
        hits
    }
}

fn polygon_intersects(points: &[Point], rect: ContentBounds) -> bool {
    let in_rect = |point: Point| {
        rect.intersects(ContentBounds {
            min: point,
            max: point,
        })
    };
    let edge_crosses = |a: Point, b: Point| {
        let crosses = |v, from: f32, to: f32, other_from, other_to, low, high| {
            if (from < v) == (to < v) {
                return false;
            }
            let other = other_from + (other_to - other_from) * ((v - from) / (to - from));
            (low..=high).contains(&other)
        };
        [rect.min.x, rect.max.x]
            .into_iter()
            .any(|x| crosses(x, a.x, b.x, a.y, b.y, rect.min.y, rect.max.y))
            || [rect.min.y, rect.max.y]
                .into_iter()
                .any(|y| crosses(y, a.y, b.y, a.x, b.x, rect.min.x, rect.max.x))
    };
    contains(points, rect.min)
        || points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .any(|(&a, &b)| in_rect(a) || edge_crosses(a, b))
}
