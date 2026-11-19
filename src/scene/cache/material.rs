//! Retained per-layer terrain geometry and its source document layer.
use super::{
    REGIONS,
    snapshot::{Batch, RegionSource},
};
use crate::{
    document::{Brush, Layer, Point},
    scene::{
        Shape,
        builder::relief_order,
        terrain::{
            field_index::FieldIndex,
            forest::{self, ForestBatch},
            mountains::{mountain_base, mountain_chunks},
        },
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

mod live;
pub(super) use live::LiveMaterial;

mod tiles;
pub(super) use tiles::TerrainIdentity;
use tiles::TerrainTiles;

#[derive(Clone)]
pub(super) struct MaterialLayer {
    pub(super) source: Layer,
    field: Arc<FieldIndex>,
    memory: OnceLock<usize>,
    pub(super) valid: bool,
    pub(super) terrain: TerrainTiles,
    pub(super) forest: Arc<Vec<ForestBatch>>,
    pub(super) placement: Arc<forest::Placement>,
    pub(super) lines: Vec<(usize, Vec<Batch>)>,
    pub(super) rivers: HashMap<usize, Arc<crate::scene::RiverReach>>,
}

impl MaterialLayer {
    pub(super) fn promote_live(&mut self) {
        self.memory = OnceLock::new();
        self.field = Arc::new(FieldIndex::reconcile(&self.source, Some(&self.field)));
        self.valid &= !crate::scene::generation::is_stopped();
    }

    pub(super) fn matches(&self, source: &Layer) -> bool {
        self.source.id == source.id && self.source.strokes == source.strokes
    }
    /// A course edit leaves the terrain, eraser index, and other lines usable.
    pub(super) fn with_linear_edits(&self, source: &Layer) -> Option<Self> {
        let previous = &self.source.strokes;
        if previous.len() != source.strokes.len() {
            return None;
        }
        let mut edits = Vec::new();
        let mut offset = 0;
        for ((old_id, _, old), (new_id, _, new)) in previous.chunks().zip(source.strokes.chunks()) {
            if old_id != new_id {
                for (index, (a, b)) in old.iter().zip(new).enumerate() {
                    if a == b {
                        continue;
                    }
                    if a.brush != b.brush || !matches!(a.brush, Brush::River | Brush::Path) {
                        return None;
                    }
                    edits.push(offset + index);
                }
            }
            offset += old.len();
        }
        if edits.is_empty() {
            return None;
        }
        let mut result = self.clone();
        result.source = source.clone();
        result.memory = OnceLock::new();
        let mut opaque = source.clone();
        opaque.opacity = 1.;
        for index in edits {
            let slot = result
                .lines
                .binary_search_by_key(&index, |(i, _)| *i)
                .ok()?;
            let previous = &mut result.lines[slot].1;
            *previous = stroke_batches(&opaque, index, previous, &result.field, &mut result.rivers);
        }
        result.valid &= !crate::scene::generation::is_stopped();
        Some(result)
    }

    pub(super) fn bytes(&self) -> usize {
        *self.memory.get_or_init(|| self.measure_bytes())
    }
    fn measure_bytes(&self) -> usize {
        let shapes = self
            .batches()
            .map(|batch| {
                batch
                    .iter()
                    .map(|shape| {
                        shape.points().count() * std::mem::size_of::<crate::document::Point>()
                            + std::mem::size_of::<Shape>()
                    })
                    .sum::<usize>()
            })
            .sum::<usize>();
        shapes
            + self.terrain.bytes()
            + self.forest.capacity() * std::mem::size_of::<ForestBatch>()
            + self.placement.bytes()
            + self.field.bytes()
    }
    pub(super) fn new(source: &Layer) -> Self {
        let authoritative = source;
        let mut opaque = source.clone();
        opaque.opacity = 1.;
        let source = &opaque;
        let field = Arc::new(FieldIndex::new(source));
        let terrain = TerrainTiles::new(source, &field);
        let placement = Arc::new(forest::Placement::new(source));
        let mut rivers = HashMap::new();
        let mut material = Self {
            source: authoritative.clone(),
            field: field.clone(),
            memory: OnceLock::new(),
            valid: true,
            terrain,
            forest: Arc::new(placement.batches(&[])),
            placement,
            lines: source
                .strokes
                .iter()
                .enumerate()
                .take_while(|_| !crate::scene::generation::stopped())
                .filter(|(_, s)| matches!(s.brush, Brush::Mountain | Brush::River | Brush::Path))
                .map(|(i, _)| (i, stroke_batches(source, i, &[], &field, &mut rivers)))
                .collect(),
            rivers,
        };
        material.valid = !crate::scene::generation::is_stopped();
        material
    }

    fn mountain_peaks(&self) -> impl Iterator<Item = (usize, usize, Point, &[Batch; 2])> {
        self.lines
            .iter()
            .filter(|(index, _)| self.source.strokes[*index].brush == Brush::Mountain)
            .flat_map(|(index, chunks)| {
                chunks
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .enumerate()
                    .filter_map(move |(ordinal, flanks)| {
                        Some((*index, ordinal, mountain_base(&flanks[0])?, flanks))
                    })
            })
    }

    pub(super) fn mountain_parts(&self, out: &mut HashMap<usize, crate::scene::MountainPart>) {
        for (index, ordinal, base, flanks) in self.mountain_peaks() {
            let stroke = &self.source.strokes[index];
            let bounds = flanks
                .iter()
                .flat_map(|batch| batch.iter())
                .filter_map(Shape::bounds)
                .reduce(|a, b| a.union(b));
            let Some(bounds) = bounds else {
                continue;
            };
            for (side, batch) in flanks
                .iter()
                .enumerate()
                .filter(|(_, batch)| !batch.is_empty())
            {
                out.insert(
                    Arc::as_ptr(batch) as usize,
                    crate::scene::MountainPart {
                        id: (self.source.id, index, stroke.seed, ordinal),
                        base,
                        side: if flanks[1].is_empty() {
                            0.
                        } else if side == 0 {
                            -1.
                        } else {
                            1.
                        },
                        width: bounds.max.x - bounds.min.x,
                    },
                );
            }
        }
    }

    pub(super) fn path_parts(&self, out: &mut HashMap<usize, crate::scene::PathPart>) {
        for (index, chunks) in &self.lines {
            let stroke = &self.source.strokes[*index];
            if stroke.brush != Brush::Path {
                continue;
            }
            for (ordinal, batch) in chunks.iter().enumerate() {
                let Some(Shape::Polygon { points, .. }) = batch.first() else {
                    continue;
                };
                let midpoint = |a: crate::document::Point, b: crate::document::Point| {
                    crate::document::Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
                };
                out.insert(
                    Arc::as_ptr(batch) as usize,
                    crate::scene::PathPart {
                        id: (self.source.id, *index, stroke.seed, ordinal),
                        start: midpoint(points[0], points[3]),
                        end: midpoint(points[1], points[2]),
                    },
                );
            }
        }
    }

    pub(super) fn batches(&self) -> impl Iterator<Item = Batch> + '_ {
        let peaks = self
            .mountain_peaks()
            .map(|(_, _, base, flanks)| (base.y, flanks.clone()));
        let trees = self
            .forest
            .iter()
            .map(|batch| (batch.tree.center.y, batch.shapes.clone()));
        (0..REGIONS.len())
            .rev()
            .flat_map(|index| {
                self.terrain
                    .regions(index)
                    .flat_map(|(_, tile)| tile.batches.iter().cloned())
            })
            .chain(self.line_batches(Brush::River))
            .chain(relief_order(peaks, trees))
            .chain(self.line_batches(Brush::Path))
    }

    fn line_batches(&self, brush: Brush) -> impl Iterator<Item = Batch> + '_ {
        self.lines
            .iter()
            .filter(move |(index, _)| self.source.strokes[*index].brush == brush)
            .flat_map(|(_, chunks)| chunks.iter().cloned())
    }

    pub(super) fn region_sources(&self) -> impl Iterator<Item = (usize, RegionSource)> + '_ {
        REGIONS
            .into_iter()
            .enumerate()
            .flat_map(move |(index, brush)| {
                self.terrain.regions(index).flat_map(move |(_, region)| {
                    let source = (self.source.id, brush, Arc::as_ptr(&region.surface) as usize);
                    region
                        .batches
                        .iter()
                        .map(move |batch| (Arc::as_ptr(batch) as usize, source))
                })
            })
    }
}

#[cfg(test)]
pub(in crate::scene) fn build_terrain(layer: &Layer) -> Vec<Shape> {
    let terrain = TerrainTiles::new(layer, &FieldIndex::new(layer));
    (0..REGIONS.len())
        .rev()
        .flat_map(|index| terrain.regions(index).flat_map(|(_, tile)| &tile.batches))
        .flat_map(|batch| batch.iter().cloned())
        .map(|mut shape| {
            *shape.opacity_mut() *= layer.opacity;
            shape
        })
        .collect()
}

fn stroke_batches(
    layer: &Layer,
    index: usize,
    previous: &[Batch],
    field: &Arc<FieldIndex>,
    rivers: &mut HashMap<usize, Arc<crate::scene::RiverReach>>,
) -> Vec<Batch> {
    crate::scene::work::update(|work| work.line_strokes += 1);
    let stroke = &layer.strokes[index];
    if stroke.brush == Brush::River {
        for batch in previous {
            rivers.remove(&(Arc::as_ptr(batch) as usize));
        }
        return crate::scene::terrain::field::with_index(layer, field.clone(), || {
            crate::scene::terrain::rivers::river_batches(layer, index, stroke)
                .into_iter()
                .enumerate()
                .map(|(i, batch)| {
                    let shapes = previous
                        .get(i)
                        .filter(|old| old.as_ref() == &batch.shapes)
                        .cloned()
                        .unwrap_or_else(|| Arc::new(batch.shapes));
                    rivers.insert(Arc::as_ptr(&shapes) as usize, Arc::new(batch.reach));
                    shapes
                })
                .collect()
        });
    }
    let chunks = crate::scene::terrain::field::with_index(layer, field.clone(), || {
        if stroke.brush == Brush::Mountain {
            mountain_chunks(layer, index, stroke)
                .into_iter()
                .flatten()
                .collect()
        } else {
            crate::scene::terrain::linear::linear_chunks(layer, index, stroke)
        }
    });
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, shapes)| {
            previous
                .get(i)
                .filter(|old| old.as_ref() == &shapes)
                .cloned()
                .unwrap_or_else(|| Arc::new(shapes))
        })
        .collect()
}
