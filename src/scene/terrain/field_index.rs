//! Exact nearest-segment queries over immutable stroke payloads.
use super::{
    super::{
        geometry::{distance, segment_distance},
        noise::noise,
    },
    field::{cuts_region, radius, roughness},
};
use crate::document::{Brush, ContentBounds, Layer, Point, SharedVec, Stroke};
use std::sync::Arc;

struct Node {
    bounds: ContentBounds,
    range: std::ops::Range<usize>,
    children: Option<(usize, usize)>,
}
struct Segments {
    points: SharedVec<Point>,
    indices: Vec<usize>,
    nodes: Vec<Node>,
}
impl Segments {
    fn new(points: &SharedVec<Point>) -> Self {
        let mut index = Self {
            points: points.clone(),
            indices: (0..points.len().saturating_sub(1)).collect(),
            nodes: Vec::new(),
        };
        if !index.indices.is_empty() {
            index.build(0..index.indices.len());
        }
        index
    }
    fn build(&mut self, range: std::ops::Range<usize>) -> usize {
        let bounds = self.indices[range.clone()]
            .iter()
            .map(|&i| {
                ContentBounds {
                    min: self.points[i],
                    max: self.points[i],
                }
                .union(ContentBounds {
                    min: self.points[i + 1],
                    max: self.points[i + 1],
                })
            })
            .reduce(ContentBounds::union)
            .expect("nonempty BVH");
        let id = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            range: range.clone(),
            children: None,
        });
        if range.len() > 16 && !super::super::generation::stopped() {
            let horizontal = bounds.width() >= bounds.height();
            let points = &self.points;
            self.indices[range.clone()].select_nth_unstable_by(range.len() / 2, |&a, &b| {
                let center = |i: usize| {
                    if horizontal {
                        points[i].x + points[i + 1].x
                    } else {
                        points[i].y + points[i + 1].y
                    }
                };
                center(a).total_cmp(&center(b))
            });
            let middle = range.start + range.len() / 2;
            let left = self.build(range.start..middle);
            let right = self.build(middle..range.end);
            self.nodes[id].children = Some((left, right));
        }
        id
    }
    fn distance(&self, q: Point) -> f32 {
        if self.points.len() == 1 {
            return distance(q, self.points[0]);
        }
        if self.nodes.is_empty() {
            return f32::INFINITY;
        }
        let mut best = f32::INFINITY;
        let mut stack = vec![0];
        while let Some(id) = stack.pop() {
            let node = &self.nodes[id];
            if lower(q, node.bounds) > best {
                continue;
            }
            if let Some((a, b)) = node.children {
                if lower(q, self.nodes[a].bounds) < lower(q, self.nodes[b].bounds) {
                    stack.push(b);
                    stack.push(a);
                } else {
                    stack.push(a);
                    stack.push(b);
                }
            } else {
                for &i in &self.indices[node.range.clone()] {
                    best = best.min(segment_distance(q, self.points[i], self.points[i + 1]));
                }
            }
        }
        best
    }
}

fn lower(q: Point, bounds: ContentBounds) -> f32 {
    // Conservatively include floating-point projection error at large world coordinates.
    let margin = bounds
        .min
        .x
        .abs()
        .max(bounds.min.y.abs())
        .max(bounds.max.x.abs())
        .max(bounds.max.y.abs())
        * f32::EPSILON
        * 8.
        + 0.001;
    let dx = (bounds.min.x - margin - q.x)
        .max(0.)
        .max(q.x - bounds.max.x - margin);
    let dy = (bounds.min.y - margin - q.y)
        .max(0.)
        .max(q.y - bounds.max.y - margin);
    (dx * dx + dy * dy).sqrt()
}

pub(in crate::scene) struct FieldIndex {
    layer: Layer,
    indices: Vec<Option<Arc<Segments>>>,
    erasers: EraserIndex,
}
impl FieldIndex {
    pub fn new(layer: &Layer) -> Self {
        Self::reconcile(layer, None)
    }
    pub fn reconcile(layer: &Layer, old: Option<&Self>) -> Self {
        Self {
            layer: layer.clone(),
            erasers: EraserIndex::new(layer),
            indices: layer
                .strokes
                .iter()
                .enumerate()
                .take_while(|_| !super::super::generation::stopped())
                .map(|(i, s)| {
                    if !matches!(s.brush, Brush::Land | Brush::Water | Brush::Erase)
                        || s.strength <= 0.
                    {
                        return None;
                    }
                    if let Some(old) = old
                        && old
                            .layer
                            .strokes
                            .get(i)
                            .is_some_and(|previous| previous.points.ptr_eq(&s.points))
                    {
                        return old.indices.get(i).cloned().flatten();
                    }
                    Some(Arc::new(Segments::new(&s.points)))
                })
                .collect(),
        }
    }
    pub fn erased(&self, q: Point, after: usize) -> bool {
        self.erasers
            .candidates(q)
            .into_iter()
            .filter(|i| *i > after)
            .any(|i| {
                self.indices[i]
                    .as_ref()
                    .is_some_and(|index| index.distance(q) < radius(&self.layer.strokes[i]))
            })
    }
    pub fn bytes(&self) -> usize {
        self.erasers.bytes()
            + self.indices.capacity() * std::mem::size_of::<Option<Arc<Segments>>>()
            + self
                .indices
                .iter()
                .flatten()
                .map(|index| {
                    index.nodes.capacity() * std::mem::size_of::<Node>()
                        + index.indices.capacity() * std::mem::size_of::<usize>()
                })
                .sum::<usize>()
    }
    #[cfg(test)]
    pub fn sample(&self, q: Point, brush: Brush) -> f32 {
        self.sample_for(q, brush, None, 0..self.indices.len(), 10_000.)
    }

    pub fn sample_tile(
        &self,
        q: Point,
        brush: Brush,
        material: Option<&str>,
        strokes: &[usize],
        band: f32,
    ) -> f32 {
        self.sample_for(q, brush, material, strokes.iter().copied(), band)
    }

    fn sample_for(
        &self,
        q: Point,
        brush: Brush,
        material: Option<&str>,
        strokes: impl Iterator<Item = usize>,
        band: f32,
    ) -> f32 {
        let mut field = -band;
        for i in strokes {
            if super::super::generation::stopped() {
                break;
            }
            let (stroke, index) = (&self.layer.strokes[i], &self.indices[i]);
            if stroke.strength <= 0. || stroke.points.is_empty() {
                continue;
            }
            let Some(index) = index else {
                continue;
            };
            let Some(subtract) = field_effect(stroke, brush, material) else {
                continue;
            };
            let lower = index.nodes.first().map_or_else(
                || distance(q, stroke.points[0]),
                |node| lower(q, node.bounds),
            );
            let decoration = noise(q, stroke.seed) * roughness(stroke);
            let radius = radius(stroke);
            let upper = radius - lower + decoration;
            if subtract {
                if -upper < field {
                    let value = radius - index.distance(q) + decoration;
                    field = field.min(-value).max(-band);
                }
            } else if upper >= field {
                field = field.max(radius - index.distance(q) + decoration).min(band);
            }
        }
        field
    }

    /// Prove that the whole block saturates the narrow band. The distance
    /// field is 1-Lipschitz; noise stays inside the brush's roughness bound.
    pub fn uniform_tile_samples(
        &self,
        center: Point,
        extent: f32,
        brush: Brush,
        material: Option<&str>,
        strokes: &[usize],
        band: f32,
    ) -> Option<f32> {
        let (mut low, mut high) = (-band, -band);
        for &i in strokes {
            let stroke = &self.layer.strokes[i];
            let Some(index) = &self.indices[i] else {
                continue;
            };
            let Some(subtract) = field_effect(stroke, brush, material) else {
                continue;
            };
            let margin = extent + roughness(stroke) + 1.;
            let distance = index.distance(center);
            let radius = radius(stroke);
            let a = radius - distance - margin;
            let b = radius - distance + margin;
            (low, high) = if subtract {
                (low.min(-b).max(-band), high.min(-a).max(-band))
            } else {
                (low.max(a).min(band), high.max(b).min(band))
            };
        }
        (low == high).then_some(low)
    }
}

fn field_effect(stroke: &Stroke, brush: Brush, material: Option<&str>) -> Option<bool> {
    // Finishes only compete with finishes of the same brush. Final geometry
    // clips the material mask to the visible land or water surface.
    if stroke.brush == brush {
        Some(material.is_some() && material != stroke.material.as_deref())
    } else if stroke.brush == Brush::Erase
        || (material.is_none() && cuts_region(stroke.brush, brush))
    {
        Some(true)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Stroke};
    #[test]
    fn indexed_ordered_fold_is_bit_exact_with_reference_history() {
        let mut document = Document::blank();
        for i in 0..36 {
            document.layers[0].strokes.push(Stroke {
                creation_id: None,
                material: None,
                brush: if i % 3 == 2 {
                    Brush::Erase
                } else if i % 2 == 0 {
                    Brush::Land
                } else {
                    Brush::Water
                },
                points: (0..40)
                    .map(|j| {
                        Point::new(
                            -600. + j as f32 * 37.,
                            -300. + (j as f32 * 0.4 + i as f32).sin() * 260.,
                        )
                    })
                    .collect(),
                size: 40. + i as f32 * 3.,
                strength: if i % 7 == 0 { 0. } else { 0.7 },
                seed: i,
            });
        }
        let layer = &document.layers[0];
        let indexed = FieldIndex::new(layer);
        for brush in [Brush::Land, Brush::Water] {
            for y in -12..12 {
                for x in -20..20 {
                    let q = Point::new(x as f32 * 41., y as f32 * 39.);
                    assert_eq!(
                        indexed.sample(q, brush),
                        super::super::field::region_field(q, layer, brush)
                    );
                }
            }
            for q in [
                Point::new(-1_000_000., 900_000.),
                Point::new(800_000., -900_000.),
            ] {
                assert_eq!(
                    indexed.sample(q, brush),
                    super::super::field::region_field(q, layer, brush)
                );
            }
        }
    }
}

/// Append-only live distance index. Completed 256-segment blocks are retained;
/// only the last block is rebuilt, and blocks own small local point arrays.
#[derive(Clone, Default)]
pub(in crate::scene) struct LiveDistance {
    blocks: Vec<Arc<Segments>>,
    completed: usize,
    first: Option<Point>,
}
impl LiveDistance {
    pub fn extend(&mut self, points: &SharedVec<Point>) {
        self.first = points.first().copied();
        self.blocks.truncate(self.completed);
        let pairs = points.len().saturating_sub(1);
        let complete = pairs / 256;
        for block in self.completed..complete {
            let local = points[block * 256..=block * 256 + 256].to_vec().into();
            self.blocks.push(Arc::new(Segments::new(&local)));
        }
        self.completed = complete;
        if !pairs.is_multiple_of(256) {
            let local = points[complete * 256..].to_vec().into();
            self.blocks.push(Arc::new(Segments::new(&local)));
        }
    }
    pub fn distance(&self, q: Point) -> f32 {
        let mut best = f32::INFINITY;
        for block in &self.blocks {
            if block
                .nodes
                .first()
                .is_some_and(|node| lower(q, node.bounds) <= best)
            {
                best = best.min(block.distance(q));
            }
        }
        if self.blocks.is_empty() {
            self.first.map_or(f32::INFINITY, |p| distance(q, p))
        } else {
            best
        }
    }
}

pub(in crate::scene) struct StrokeDistance(Segments);
impl StrokeDistance {
    pub fn new(points: &SharedVec<Point>) -> Self {
        Self(Segments::new(points))
    }
    pub fn distance(&self, q: Point) -> f32 {
        self.0.distance(q)
    }
}

struct EraserNode {
    bounds: ContentBounds,
    left: Option<(usize, usize)>,
    range: std::ops::Range<usize>,
}
struct EraserIndex {
    nodes: Vec<EraserNode>,
    entries: Vec<(usize, ContentBounds)>,
}
impl EraserIndex {
    fn new(layer: &Layer) -> Self {
        let mut index = Self {
            nodes: Vec::new(),
            entries: layer
                .strokes
                .iter()
                .enumerate()
                .filter(|(_, s)| s.brush == Brush::Erase && s.strength > 0.)
                .filter_map(|(i, s)| s.bounds().map(|bounds| (i, bounds)))
                .collect(),
        };
        if !index.entries.is_empty() {
            index.build(0..index.entries.len());
        }
        index
    }
    fn build(&mut self, range: std::ops::Range<usize>) -> usize {
        let bounds = self.entries[range.clone()]
            .iter()
            .map(|(_, b)| *b)
            .reduce(ContentBounds::union)
            .expect("eraser bounds");
        let id = self.nodes.len();
        self.nodes.push(EraserNode {
            bounds,
            left: None,
            range: range.clone(),
        });
        if range.len() > 8 && !super::super::generation::stopped() {
            let horizontal = bounds.width() >= bounds.height();
            self.entries[range.clone()].select_nth_unstable_by(
                range.len() / 2,
                |(_, a), (_, b)| {
                    let center = |b: &ContentBounds| {
                        if horizontal {
                            b.min.x + b.max.x
                        } else {
                            b.min.y + b.max.y
                        }
                    };
                    center(a).total_cmp(&center(b))
                },
            );
            let middle = range.start + range.len() / 2;
            let a = self.build(range.start..middle);
            let b = self.build(middle..range.end);
            self.nodes[id].left = Some((a, b));
        }
        id
    }
    fn candidates(&self, q: Point) -> Vec<usize> {
        let query = ContentBounds { min: q, max: q };
        let mut output = Vec::new();
        let mut stack = if self.nodes.is_empty() {
            Vec::new()
        } else {
            vec![0]
        };
        while let Some(id) = stack.pop() {
            let node = &self.nodes[id];
            if !node.bounds.intersects(query) {
                continue;
            }
            if let Some((a, b)) = node.left {
                stack.push(a);
                stack.push(b);
            } else {
                output.extend(
                    self.entries[node.range.clone()]
                        .iter()
                        .filter(|(_, b)| b.intersects(query))
                        .map(|(i, _)| *i),
                );
            }
        }
        output
    }
    fn bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<EraserNode>()
            + self.entries.capacity() * std::mem::size_of::<(usize, ContentBounds)>()
    }
}
