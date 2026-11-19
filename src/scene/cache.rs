//! Retain committed terrain and manage the transient material of a live stroke.
mod composition;
mod junctions;
mod material;
mod picking;
mod snapshot;
mod territories;

use super::{
    noise::noise,
    terrain::field::{radius, roughness},
};
use crate::document::{Brush, Document, Point, Stroke};
use composition::{AnnotationCache, AnnotationSource, compose};
use material::{LiveMaterial, MaterialLayer};
pub use picking::TerrainHit;
use snapshot::Batch;
pub use snapshot::Scene;

#[cfg(test)]
pub(super) use material::build_terrain;

const REGIONS: [Brush; 2] = [Brush::Land, Brush::Water];

/// Tests the uncomposited footprint when assigning a cue to a finished region.
pub fn region_stroke_covers(stroke: &Stroke, point: Point) -> bool {
    let reach = radius(stroke) + noise(point, stroke.seed) * roughness(stroke);
    super::geometry::stroke_distance(point, stroke) <= reach
}

/// Retains unchanged layers and paper geometry. A live gesture replaces its
/// layer in the composite, so erasing, opacity and higher layers work identically
/// during a drag and after commit. Cancel simply drops the transient material.
#[derive(Default)]
pub struct SceneCache {
    layers: Vec<MaterialLayer>,
    live: Option<LiveMaterial>,
    retained: Vec<MaterialLayer>,
    work: super::work::WorkCounts,
    rollback: Option<LiveMaterial>,
    rollback_order: Vec<u64>,
    annotations: AnnotationCache,
    junctions: junctions::Junctions,
    territories: territories::Territories,
}

impl SceneCache {
    /// Preserve the last complete live material while a cancellable worker
    /// advances it. Geometry and immutable indices share their allocations.
    pub fn begin_generation(&mut self) {
        self.rollback = self.live.clone();
        self.rollback_order = self.layers.iter().map(|layer| layer.source.id).collect();
    }
    pub fn finish_generation(&mut self, success: bool) {
        if success {
            self.rollback = None;
            self.rollback_order.clear();
            return;
        }
        self.live = self.rollback.take();
        self.junctions = junctions::Junctions::default();
        self.territories = territories::Territories::default();
        self.layers.retain(|layer| layer.valid);
        self.retained.retain(|layer| layer.valid);
        let mut present: std::collections::HashSet<_> =
            self.layers.iter().map(|layer| layer.source.id).collect();
        for index in (0..self.retained.len()).rev() {
            if present.insert(self.retained[index].source.id) {
                self.layers.push(self.retained.swap_remove(index));
            }
        }
        self.layers.sort_by_key(|layer| {
            self.rollback_order
                .iter()
                .position(|id| *id == layer.source.id)
                .unwrap_or(usize::MAX)
        });
        self.rollback_order.clear();
    }

    /// A ready authoritative base snapshot for immediate gesture cancellation.
    pub fn committed_scene(&self) -> Scene {
        compose(
            self.layers.iter(),
            &self.annotations,
            AnnotationSource::Committed,
            &mut junctions::Junctions::default(),
            &mut territories::Territories::default(),
        )
    }

    pub fn work(&self) -> super::work::WorkCounts {
        self.work
    }

    pub fn render(&mut self, doc: &Document, live: Option<(u64, &Stroke)>) -> Scene {
        let _span = crate::performance::Span::new("scene_generation");
        super::work::take();
        // Mouse-up can append points without a rendered live frame. Advance from
        // the retained base before promotion, including gestures between frames.
        if live.is_none() {
            for base in &self.layers {
                let Some(layer) = doc.layer(base.source.id) else {
                    continue;
                };
                if layer.strokes.len() != base.source.strokes.len() + 1
                    || layer.opacity != base.source.opacity
                    || layer.relief_moves != base.source.relief_moves
                    || !layer.strokes.has_prefix(&base.source.strokes)
                {
                    continue;
                }
                let stroke = layer.strokes.last().expect("appended stroke");
                if !self
                    .live
                    .as_ref()
                    .is_some_and(|pending| pending.accepts(layer.id, stroke))
                {
                    self.live = Some(LiveMaterial::new(base, stroke));
                }
                self.live
                    .as_mut()
                    .expect("pending release")
                    .update(base, stroke);
            }
        }
        // Promote the already displayed material on release; no second render.
        if live.is_none()
            && self.live.as_ref().is_some_and(|live| {
                doc.layer(live.material.source.id) == Some(&live.material.source)
            })
        {
            let mut material = self.live.take().expect("checked live material").material;
            material.promote_live();
            if let Some(index) = self
                .layers
                .iter()
                .position(|layer| layer.source.id == material.source.id)
            {
                self.retained.push(self.layers.swap_remove(index));
            }
            self.layers.push(material);
        }
        let mut old = std::mem::take(&mut self.layers);
        let mut changed = false;
        for layer in &doc.layers {
            if super::generation::stopped() {
                break;
            }
            let cached = if let Some(index) = old.iter().position(|cached| cached.matches(layer)) {
                Some(old.swap_remove(index))
            } else {
                self.retained
                    .iter()
                    .position(|cached| cached.matches(layer))
                    .map(|index| self.retained.swap_remove(index))
            };
            if let Some(mut material) = cached {
                material.source = layer.clone();
                if let Some(live) = self
                    .live
                    .as_mut()
                    .filter(|live| live.material.source.id == layer.id)
                {
                    live.material.source.opacity = layer.opacity;
                    live.material.source.visible = layer.visible;
                    live.material.source.name = layer.name.clone();
                }
                self.layers.push(material);
            } else if layer.visible && layer.opacity > 0. {
                let edited = old
                    .iter()
                    .enumerate()
                    .find(|(_, cached)| cached.source.id == layer.id)
                    .and_then(|(index, cached)| Some((index, cached.with_selection_edits(layer)?)));
                if let Some((index, material)) = edited {
                    if material.valid {
                        old.swap_remove(index);
                    }
                    self.layers.push(material);
                } else {
                    self.layers.push(MaterialLayer::new(layer));
                }
                changed = true;
            }
        }
        self.retained.extend(old);
        let mut bytes: usize = self.retained.iter().map(MaterialLayer::bytes).sum();
        while bytes > 32 * 1024 * 1024 || self.retained.len() > 64 {
            bytes -= self.retained.remove(0).bytes();
        }
        if changed {
            self.live = None;
        }
        if let Some((id, stroke)) = live.filter(|(_, stroke)| !stroke.points.is_empty()) {
            if let Some(base) = self.layers.iter().find(|layer| {
                layer.source.id == id && layer.source.visible && layer.source.opacity > 0.
            }) {
                if !self
                    .live
                    .as_ref()
                    .is_some_and(|live| live.accepts(id, stroke))
                {
                    self.live = Some(LiveMaterial::new(base, stroke));
                }
                self.live
                    .as_mut()
                    .expect("live material")
                    .update(base, stroke);
            } else {
                self.live = None;
            }
        } else {
            self.live = None;
        }
        let live = self.live.as_ref().map(|live| &live.material);
        let materials = self.layers.iter().map(|base| {
            live.filter(|material| material.source.id == base.source.id)
                .unwrap_or(base)
        });
        let mut updated = AnnotationCache::default();
        let scene = compose(
            materials,
            &self.annotations,
            AnnotationSource::Current {
                document: doc,
                updated: &mut updated,
            },
            &mut self.junctions,
            &mut self.territories,
        );
        self.annotations = updated;
        self.work = super::work::take();
        scene
    }
}

#[cfg(test)]
mod tests;
