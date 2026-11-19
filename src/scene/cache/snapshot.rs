//! Ordered scene geometry and metadata shared by the renderer and exporters.
use crate::{
    document::{Brush, ContentBounds, Label, LinearStrokeId, Point, RegionLayer},
    scene::{
        LandmarkPart, MountainPart, PathPart, RiverReach, Shape,
        terrain::forest::{self, ForestTree},
        territories::{RegionView, Shoreline},
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

pub(super) type Batch = Arc<Vec<Shape>>;
pub(super) type RegionSource = (u64, Brush, usize);
pub(super) type TerritoryView = (Arc<RegionLayer>, Arc<RegionView>);

/// Cheaply cloned, ordered material batches shared by successive canvas frames.
#[derive(Clone, Default)]
pub struct Scene {
    pub(super) batches: Arc<Vec<Batch>>,
    pub(super) trees: Arc<HashMap<usize, ForestTree>>,
    pub(super) mountains: Arc<HashMap<usize, MountainPart>>,
    pub(super) rivers: Arc<HashMap<usize, Arc<RiverReach>>>,
    pub(super) paths: Arc<HashMap<usize, PathPart>>,
    pub(super) opacity: Arc<HashMap<usize, f32>>,
    pub(super) expanded: Arc<OnceLock<Vec<Shape>>>,
    pub(super) labels: Arc<HashMap<usize, Label>>,
    pub(super) landmarks: Arc<HashMap<usize, LandmarkPart>>,
    pub(super) regions: Arc<HashMap<usize, RegionSource>>,
    pub(super) shoreline: Arc<Shoreline>,
    pub(super) territory_views: Arc<HashMap<u64, TerritoryView>>,
}

impl PartialEq for Scene {
    fn eq(&self, other: &Self) -> bool {
        self.batches == other.batches && self.trees == other.trees && self.opacity == other.opacity
    }
}

impl Scene {
    pub(super) fn order_paths_between_rivers_and_relief(&mut self) {
        if self.paths.is_empty()
            || (self.rivers.is_empty() && self.trees.is_empty() && self.mountains.is_empty())
        {
            return;
        }
        let mut after_river = self
            .batches
            .iter()
            .rposition(|batch| !batch.is_empty() && self.river_reach(batch).is_some())
            .map_or(0, |index| index + 1);
        let is_relief = |batch: &Batch| {
            let key = Arc::as_ptr(batch) as usize;
            self.trees.contains_key(&key) || self.mountains.contains_key(&key)
        };
        let mut before_relief = self
            .batches
            .iter()
            .position(is_relief)
            .unwrap_or(self.batches.len());
        let batches = Arc::make_mut(&mut self.batches);

        if before_relief < after_river {
            // Keep the trees and interleaved mountain flanks in depth order.
            let relief: Vec<_> = batches
                .extract_if(before_relief..after_river, |batch| is_relief(batch))
                .collect();
            after_river -= relief.len();
            batches.splice(after_river..after_river, relief);
            before_relief = after_river;
        }

        let is_path = |batch: &mut Batch| self.paths.contains_key(&(Arc::as_ptr(batch) as usize));
        let under_river: Vec<_> = batches.extract_if(..after_river, is_path).collect();
        after_river -= under_river.len();
        before_relief -= under_river.len();
        let over_relief: Vec<_> = batches.extract_if(before_relief.., is_path).collect();
        batches.splice(before_relief..before_relief, over_relief);
        batches.splice(after_river..after_river, under_river);
    }

    pub fn same_snapshot(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.batches, &other.batches)
    }

    pub fn shoreline(&self) -> &Arc<Shoreline> {
        &self.shoreline
    }

    pub fn region_view(&self, layer: u64) -> Option<&RegionView> {
        self.territory_views
            .get(&layer)
            .map(|(_, view)| view.as_ref())
    }

    pub fn current_region_view(
        &self,
        layer: u64,
        regions: &Arc<RegionLayer>,
    ) -> Option<&RegionView> {
        let (source, view) = self.territory_views.get(&layer)?;
        Arc::ptr_eq(source, regions).then_some(view.as_ref())
    }

    pub fn batches(&self) -> impl Iterator<Item = &Arc<Vec<Shape>>> {
        self.batches.iter()
    }

    pub fn batch_opacity(&self, batch: &Arc<Vec<Shape>>) -> f32 {
        self.opacity
            .get(&(Arc::as_ptr(batch) as usize))
            .copied()
            .unwrap_or(1.)
    }

    pub fn landmark_part(&self, batch: &Arc<Vec<Shape>>) -> Option<&LandmarkPart> {
        self.landmarks.get(&(Arc::as_ptr(batch) as usize))
    }

    pub fn label(&self, batch: &Arc<Vec<Shape>>) -> Option<&Label> {
        self.labels.get(&(Arc::as_ptr(batch) as usize))
    }

    /// Source layer, brush and local shoreline grid of a finished batch.
    pub fn region(&self, batch: &Arc<Vec<Shape>>) -> Option<RegionSource> {
        self.regions.get(&(Arc::as_ptr(batch) as usize)).copied()
    }

    pub fn forest_tree(&self, batch: &Arc<Vec<Shape>>) -> Option<&ForestTree> {
        self.trees.get(&(Arc::as_ptr(batch) as usize))
    }

    pub fn mountain_part(&self, batch: &Arc<Vec<Shape>>) -> Option<&MountainPart> {
        self.mountains.get(&(Arc::as_ptr(batch) as usize))
    }
    pub fn mountain_parts(&self) -> impl Iterator<Item = &MountainPart> {
        self.mountains.values()
    }

    pub fn river_reach(&self, batch: &Arc<Vec<Shape>>) -> Option<&RiverReach> {
        self.rivers
            .get(&(Arc::as_ptr(batch) as usize))
            .map(AsRef::as_ref)
    }
    pub fn river_reaches(&self) -> impl Iterator<Item = &RiverReach> {
        self.rivers.values().map(AsRef::as_ref)
    }

    pub fn path_part(&self, batch: &Arc<Vec<Shape>>) -> Option<&PathPart> {
        self.paths.get(&(Arc::as_ptr(batch) as usize))
    }
    pub fn path_parts(&self) -> impl Iterator<Item = &PathPart> {
        self.paths.values()
    }

    /// Pick from the finished artwork in draw order, including clipped river reaches
    /// and erased gaps. Within the pick tolerance, prefer a direct hit.
    pub fn linear_stroke_at(&self, point: Point, tolerance: f32) -> Option<LinearStrokeId> {
        use crate::scene::geometry::{contains, segment_distance};
        if self.paths.is_empty() && self.rivers.is_empty() {
            return None;
        }
        let pick_bounds = ContentBounds {
            min: point,
            max: point,
        }
        .padded(tolerance);
        let mut nearest = None;
        let mut distance = tolerance;
        for batch in self.batches.iter().rev() {
            let Some(id) = self.course_id(batch) else {
                continue;
            };
            for shape in batch.iter() {
                let Shape::Polygon { points, .. } = shape else {
                    continue;
                };
                if !shape
                    .bounds()
                    .is_some_and(|bounds| bounds.intersects(pick_bounds))
                {
                    continue;
                }
                if contains(points, point) {
                    return Some(id);
                }
                for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
                    let edge = segment_distance(point, a, b);
                    if edge < distance {
                        distance = edge;
                        nearest = Some(id);
                    }
                }
            }
        }
        nearest
    }

    pub fn forest_trees(&self) -> impl Iterator<Item = &ForestTree> {
        self.trees.values()
    }

    pub fn shapes(&self) -> impl Iterator<Item = &Shape> {
        self.expanded
            .get_or_init(|| {
                self.batches
                    .iter()
                    .flat_map(|batch| {
                        self.forest_tree(batch).map_or_else(
                            || {
                                batch
                                    .iter()
                                    .cloned()
                                    .map(|mut shape| {
                                        *shape.opacity_mut() *= self.batch_opacity(batch);
                                        shape
                                    })
                                    .collect()
                            },
                            forest::tree_shapes,
                        )
                    })
                    .collect()
            })
            .iter()
    }
}
