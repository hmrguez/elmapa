//! Incremental updates for an in-progress stroke on a retained layer.
use super::{MaterialLayer, REGIONS, forest, stroke_batches};
use crate::{
    document::{Brush, Stroke},
    scene::{
        generation,
        terrain::field_index::{FieldIndex, LiveDistance},
    },
};
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub(in crate::scene::cache) struct LiveMaterial {
    pub(in crate::scene::cache) material: MaterialLayer,
    dabs: forest::DabSampler,
    distance: LiveDistance,
}

impl LiveMaterial {
    pub(in crate::scene::cache) fn new(base: &MaterialLayer, stroke: &Stroke) -> Self {
        let mut source = base.source.clone();
        source.strokes.push(Stroke {
            creation_id: stroke.creation_id,
            material: stroke.material.clone(),
            brush: stroke.brush,
            points: Vec::new().into(),
            size: stroke.size,
            strength: stroke.strength,
            seed: stroke.seed,
        });
        Self {
            dabs: forest::DabSampler::default(),
            distance: LiveDistance::default(),
            material: MaterialLayer {
                source,
                field: base.field.clone(),
                memory: OnceLock::new(),
                valid: base.valid,
                terrain: base.terrain.clone(),
                forest: base.forest.clone(),
                placement: base.placement.clone(),
                lines: base.lines.clone(),
                rivers: base.rivers.clone(),
            },
        }
    }

    pub(in crate::scene::cache) fn accepts(&self, id: u64, stroke: &Stroke) -> bool {
        let previous = self.material.source.strokes.last().expect("live stroke");
        self.material.source.id == id
            && previous.brush == stroke.brush
            && previous.creation_id == stroke.creation_id
            && previous.material == stroke.material
            && previous.size == stroke.size
            && previous.strength == stroke.strength
            && previous.seed == stroke.seed
            && stroke.points.starts_with(&previous.points)
    }

    pub(in crate::scene::cache) fn update(&mut self, base: &MaterialLayer, stroke: &Stroke) {
        let previous = self
            .material
            .source
            .strokes
            .last_mut()
            .expect("live stroke");
        let count = previous.points.len();
        if count == stroke.points.len() {
            return;
        }
        previous.points = stroke.points.clone();
        self.material.memory = OnceLock::new();
        let mut delta = stroke.clone();
        delta.points = stroke.points[count.saturating_sub(1)..].to_vec().into();
        let mut opaque_source = self.material.source.clone();
        opaque_source.opacity = 1.;
        if stroke.strength <= 0. {
            return;
        }
        let affects_regions = REGIONS.contains(&stroke.brush) || stroke.brush == Brush::Erase;
        if affects_regions {
            self.distance.extend(&stroke.points);
            self.material
                .terrain
                .apply(&base.terrain, stroke, &self.distance, &delta.points);
        }
        if matches!(stroke.brush, Brush::Forest | Brush::Erase) {
            if stroke.brush == Brush::Erase {
                let mut tail = stroke.clone();
                tail.points = stroke.points[count.saturating_sub(1)..].to_vec().into();
                Arc::make_mut(&mut self.material.placement).erase(&tail);
            } else {
                Arc::make_mut(&mut self.material.placement).append(
                    self.material.source.id,
                    1.,
                    self.material.source.strokes.len() - 1,
                    stroke,
                    &mut self.dabs,
                );
            }
            self.material.forest = Arc::new(self.material.placement.batches(&self.material.forest));
        }
        if stroke.brush == Brush::Erase {
            self.material.field = Arc::new(FieldIndex::reconcile(
                &opaque_source,
                Some(&self.material.field),
            ));
            self.material.lines = self
                .material
                .lines
                .iter()
                .map(|(index, shapes)| {
                    let affected = delta
                        .bounds()
                        .zip(base.source.strokes[*index].bounds())
                        .is_some_and(|(a, b)| a.intersects(b));
                    (
                        *index,
                        if affected {
                            stroke_batches(
                                &opaque_source,
                                *index,
                                shapes,
                                &self.material.field,
                                &mut self.material.rivers,
                            )
                        } else {
                            shapes.clone()
                        },
                    )
                })
                .collect();
        } else if matches!(stroke.brush, Brush::Mountain | Brush::River | Brush::Path) {
            let index = self.material.source.strokes.len() - 1;
            let previous = self
                .material
                .lines
                .iter()
                .find(|(i, _)| *i == index)
                .map(|(_, chunks)| chunks.clone())
                .unwrap_or_default();
            self.material.lines = base.lines.clone();
            let index = self.material.source.strokes.len() - 1;
            self.material.lines.push((
                index,
                stroke_batches(
                    &opaque_source,
                    index,
                    &previous,
                    &self.material.field,
                    &mut self.material.rivers,
                ),
            ));
        }
        self.material.valid &= !generation::is_stopped();
    }
}
