//! Seeded placement and geometry for the scattered tree brush.
mod drawing;
mod specimens;
mod vector;

use crate::scene::{
    Shape,
    geometry::{distance, p, stroke_distance},
};
use crate::{
    brush_materials::{self, ForestStyle},
    document::{Brush, Layer, Point, ReliefKey, Specimen, Stroke},
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

const MIXED: std::ops::Range<usize> = 0..10;
const FIR: std::ops::Range<usize> = MIXED.end..MIXED.end + 3;
const OAK: std::ops::Range<usize> = FIR.end..FIR.end + 3;
pub const FOREST_SPECIMEN_COUNT: usize = OAK.end;

/// Original canopy artwork in coordinates local to the trunk.
pub fn forest_specimen(variant: usize) -> String {
    forest_specimen_svg(variant).to_owned()
}

/// Shared original SVG fragments used by export and both vector/raster adapters.
pub fn forest_specimen_svg(variant: usize) -> &'static str {
    static SVG: OnceLock<[String; FOREST_SPECIMEN_COUNT]> = OnceLock::new();
    &SVG.get_or_init(|| std::array::from_fn(specimens::specimen))[variant]
}

/// Parse each original specimen once for the lifetime of the process.
pub fn forest_specimen_tree(variant: usize) -> &'static usvg::Tree {
    static TREES: OnceLock<[usvg::Tree; FOREST_SPECIMEN_COUNT]> = OnceLock::new();
    &TREES.get_or_init(|| std::array::from_fn(|variant| {
        let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"100\" viewBox=\"-50 -50 100 100\">{}</svg>", forest_specimen_svg(variant));
        usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("original tree specimen")
    }))[variant]
}

/// Representative specimens for a Forest material swatch.
pub fn forest_swatch(style: Option<ForestStyle>) -> &'static [usize] {
    match style {
        None => &[0, 7],
        Some(ForestStyle::Fir) => &[FIR.start],
        Some(ForestStyle::Oak) => &[OAK.start],
    }
}

pub type TreeId = (u64, usize, usize, usize);

#[derive(Clone, Debug, PartialEq)]
pub struct ForestTree {
    pub id: TreeId,
    pub seed: u64,
    pub center: Point,
    pub origin: Point,
    pub radius: f32,
    pub scale: f32,
    pub rotation: f32,
    pub variant: usize,
    pub opacity: f32,
}

#[derive(Clone)]
pub(in crate::scene) struct ForestBatch {
    pub tree: ForestTree,
    pub shapes: Arc<Vec<Shape>>,
}

// Preserve the source brush's scatter density, variation, spacing and collision rule.
// Input paths are resampled without a moving endpoint, so extending a gesture
// cannot reshuffle trees that have already appeared.
#[cfg(test)]
pub(in crate::scene) fn trees(layer: &Layer) -> Vec<ForestTree> {
    let field = super::field_index::FieldIndex::new(layer);
    Placement::new(layer)
        .batches(&[], layer, &field)
        .into_iter()
        .map(|batch| batch.tree)
        .collect()
}

/// Retained collision cells and accepted candidates; live updates never replay
/// committed forest RNGs. Tombstones preserve indices while erasing locally.
#[derive(Clone, Default)]
pub(in crate::scene) struct Placement {
    trees: Vec<ForestTree>,
    erased: Vec<bool>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    active: usize,
}
impl Placement {
    pub fn bytes(&self) -> usize {
        self.trees.capacity() * std::mem::size_of::<ForestTree>()
            + self.erased.capacity()
            + self.cells.capacity() * std::mem::size_of::<((i32, i32), Vec<usize>)>()
            + self
                .cells
                .values()
                .map(|ids| ids.capacity() * std::mem::size_of::<usize>())
                .sum::<usize>()
    }
    pub fn new(layer: &Layer) -> Self {
        let mut state = Self::default();
        for (index, stroke) in layer.strokes.iter().enumerate() {
            if super::super::generation::stopped() {
                break;
            }
            state.append(layer, index, stroke, &mut DabSampler::default());
        }
        state
    }
    pub fn erase(&mut self, stroke: &Stroke) {
        if stroke.strength <= 0. {
            return;
        }
        let Some(bounds) = stroke.bounds() else {
            return;
        };
        let min = cell(bounds.min);
        let max = cell(bounds.max);
        let mut candidates = Vec::new();
        let cell_count = i64::from(max.0 - min.0 + 1) * i64::from(max.1 - min.1 + 1);
        if cell_count <= (self.cells.len() * 4) as i64 {
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    if let Some(ids) = self.cells.get(&(x, y)) {
                        candidates.extend_from_slice(ids);
                    }
                }
            }
        } else {
            for (&(x, y), ids) in &self.cells {
                if x >= min.0 && x <= max.0 && y >= min.1 && y <= max.1 {
                    candidates.extend_from_slice(ids);
                }
            }
        }
        let distance = (stroke.points.len() > 16)
            .then(|| super::field_index::StrokeDistance::new(&stroke.points));
        for i in candidates {
            if super::super::generation::stopped() {
                break;
            }
            let center = self.trees[i].center;
            if !self.erased[i]
                && distance.as_ref().map_or_else(
                    || stroke_distance(center, stroke),
                    |index| index.distance(center),
                ) < super::field::radius(stroke)
            {
                self.erased[i] = true;
                self.active -= 1;
            }
        }
    }
    pub fn append(
        &mut self,
        layer: &Layer,
        stroke_index: usize,
        stroke: &Stroke,
        sampler: &mut DabSampler,
    ) {
        if stroke.brush == Brush::Erase {
            self.erase(stroke);
            return;
        }
        if stroke.brush != Brush::Forest || stroke.strength <= 0. {
            return;
        }
        let _span = crate::performance::Span::new("forest_placement");
        let radius = stroke.size * 0.5;
        let count = (stroke.strength * (0.58 / 0.85) * 18.).ceil() as usize;
        let style = stroke.material.as_deref().and_then(|id| {
            brush_materials::for_brush(Brush::Forest)
                .find(|(_, material)| material.id == id)
                .and_then(|(_, material)| material.forest)
        });
        let variants = match style {
            None => MIXED,
            Some(ForestStyle::Fir) => FIR,
            Some(ForestStyle::Oak) => OAK,
        };
        for (dab, origin) in sampler.advance(stroke) {
            if super::super::generation::stopped() || self.active >= 30_000 {
                break;
            }
            super::super::work::update(|work| work.forest_dabs += 1);
            let mut rng = ChaCha8Rng::seed_from_u64(stroke.seed.wrapping_add(dab as u64));
            let mut placed = 0;
            for candidate in 0..count * 4 {
                if placed >= count || self.active >= 30_000 {
                    break;
                }
                super::super::work::update(|work| work.forest_candidates += 1);
                let angle = rng.random::<f32>() * std::f32::consts::TAU;
                let r = rng.random::<f32>().sqrt() * radius;
                let center = p(origin.x + angle.cos() * r, origin.y + angle.sin() * r);
                let variant = rng.random_range(variants.clone());
                let scale: f32 = 0.9 * (1. + rng.random_range(-0.32..=0.32));
                let spacing = if variant < 6 || FIR.contains(&variant) {
                    13.
                } else {
                    16.
                };
                let clearance = (spacing * scale * 0.8).max(9.);
                let (x, y) = cell(center);
                if (y - 1..=y + 1).any(|cy| {
                    (x - 1..=x + 1).any(|cx| {
                        self.cells.get(&(cx, cy)).is_some_and(|ids| {
                            ids.iter().any(|i| {
                                !self.erased[*i]
                                    && distance(self.trees[*i].center, center) < clearance
                            })
                        })
                    })
                }) {
                    continue;
                }
                let rotation = rng.random_range(-9.0..9.0) * 0.4;
                if !super::super::generation::vertices(64) {
                    break;
                }
                self.cells.entry((x, y)).or_default().push(self.trees.len());
                let tree = ForestTree {
                    id: (layer.id, stroke_index, dab, candidate),
                    seed: stroke.seed,
                    center,
                    origin,
                    radius,
                    // Keep the existing scatter/collision layout while making
                    // each tree read smaller beside the mountain relief.
                    scale: scale * 0.8,
                    rotation,
                    variant,
                    opacity: layer.opacity.clamp(0., 1.),
                };
                self.trees.push(tree);
                self.erased.push(false);
                self.active += 1;
                placed += 1;
            }
        }
    }
    pub fn batches(
        &self,
        previous: &[ForestBatch],
        layer: &Layer,
        field: &super::field_index::FieldIndex,
    ) -> Vec<ForestBatch> {
        let old: HashMap<_, _> = previous
            .iter()
            .map(|batch| (batch.tree.id, batch))
            .collect();
        // Resolve edited placements after the seeded scatter. Moving one tree
        // cannot change the collision layout or looks of its original neighbors.
        let mut trees: Vec<_> = self
            .trees
            .iter()
            .zip(&self.erased)
            .filter_map(|(tree, erased)| {
                let (_, stroke, dab, candidate) = tree.id;
                if let Some(item) = layer.relief_move(ReliefKey {
                    stroke,
                    specimen: Specimen::Tree { dab, candidate },
                }) {
                    if field.erased(item.position, item.after_strokes.saturating_sub(1)) {
                        return None;
                    }
                    let mut moved = tree.clone();
                    moved.origin.x += item.position.x - tree.center.x;
                    moved.origin.y += item.position.y - tree.center.y;
                    moved.center = item.position;
                    Some(moved)
                } else {
                    (!erased).then(|| tree.clone())
                }
            })
            .collect();
        trees.sort_by(painter_order);
        trees
            .into_iter()
            .map(|tree| {
                let shapes = old
                    .get(&tree.id)
                    .filter(|batch| batch.tree == tree)
                    .map_or_else(|| Arc::new(Vec::new()), |batch| batch.shapes.clone());
                ForestBatch { tree, shapes }
            })
            .collect()
    }
}

// Rear crowns first; foreground trunks remain visible even when a later brush
// dab travels upward. Tree identity and scatter are independent of this order.
fn painter_order(a: &ForestTree, b: &ForestTree) -> std::cmp::Ordering {
    a.center
        .y
        .total_cmp(&b.center.y)
        .then_with(|| a.center.x.total_cmp(&b.center.x))
        .then_with(|| a.id.cmp(&b.id))
}

fn cell(q: Point) -> (i32, i32) {
    ((q.x / 32.).floor() as i32, (q.y / 32.).floor() as i32)
}

#[derive(Clone, Default)]
pub(in crate::scene) struct DabSampler {
    points: usize,
    carry: f32,
    next: usize,
}
impl DabSampler {
    fn advance(&mut self, stroke: &Stroke) -> Vec<(usize, Point)> {
        let mut dabs = Vec::new();
        if self.points == 0
            && let Some(first) = stroke.points.first()
        {
            dabs.push((self.next, *first));
            self.next += 1;
        }
        let spacing = (stroke.size * 0.5 * 0.38).max(9.);
        for pair in stroke.points[self.points.saturating_sub(1)..].windows(2) {
            if self.next >= 30_000 || super::super::generation::stopped() {
                break;
            }
            super::super::work::update(|work| work.resampled_segments += 1);
            let len = distance(pair[0], pair[1]);
            if len < 0.001 {
                continue;
            }
            let mut d = spacing - self.carry;
            self.carry = (self.carry + len) % spacing;
            while d <= len && self.next < 30_000 {
                let t = d / len;
                dabs.push((
                    self.next,
                    p(
                        pair[0].x + (pair[1].x - pair[0].x) * t,
                        pair[0].y + (pair[1].y - pair[0].y) * t,
                    ),
                ));
                self.next += 1;
                d += spacing;
            }
        }
        self.points = stroke.points.len();
        dabs
    }
}

#[cfg(test)]
fn dabs(stroke: &Stroke) -> impl Iterator<Item = Point> + '_ {
    let spacing = (stroke.size * 0.5 * 0.38).max(9.);
    let mut carry = 0.;
    let tail = stroke
        .points
        .windows(2)
        .filter_map(move |pair| {
            let len = distance(pair[0], pair[1]);
            if len < 0.001 {
                return None;
            }
            let d = spacing - carry;
            carry = (carry + len) % spacing;
            Some(
                std::iter::successors(Some(d), move |d| Some(d + spacing))
                    .take_while(move |d| *d <= len)
                    .map(move |d| {
                        let t = d / len;
                        p(
                            pair[0].x + (pair[1].x - pair[0].x) * t,
                            pair[0].y + (pair[1].y - pair[0].y) * t,
                        )
                    }),
            )
        })
        .flatten();
    stroke.points.first().copied().into_iter().chain(tail)
}

pub(in crate::scene) fn tree_shapes(tree: &ForestTree) -> Vec<Shape> {
    let (sin, cos) = tree.rotation.to_radians().sin_cos();
    let transform = |q: Point| {
        p(
            tree.center.x + (q.x * cos - q.y * sin) * tree.scale,
            tree.center.y + (q.x * sin + q.y * cos) * tree.scale,
        )
    };
    vector::specimens()[tree.variant]
        .iter()
        .map(|shape| match shape {
            Shape::RegionSurface { .. } | Shape::Sprite { .. } => {
                unreachable!("forest specimens contain vector shapes only")
            }
            Shape::Polygon {
                points,
                fill,
                opacity,
            } => Shape::Polygon {
                points: points.iter().copied().map(transform).collect(),
                fill: *fill,
                opacity: opacity * tree.opacity,
            },
            Shape::Line {
                points,
                color,
                width,
                opacity,
            } => Shape::Line {
                points: points.iter().copied().map(transform).collect(),
                color: *color,
                width: width * tree.scale,
                opacity: opacity * tree.opacity,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests;

impl ForestTree {
    pub(in crate::scene) fn contains(&self, point: Point, tolerance: f32) -> bool {
        use crate::scene::geometry::{contains, segment_distance};
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        let x = (point.x - self.center.x) / self.scale;
        let y = (point.y - self.center.y) / self.scale;
        let local = p(x * cos + y * sin, y * cos - x * sin);
        vector::specimens()[self.variant]
            .iter()
            .any(|shape| match shape {
                Shape::Polygon {
                    points, opacity, ..
                } => *opacity > 0. && contains(points, local),
                Shape::Line {
                    points,
                    width,
                    opacity,
                    ..
                } => {
                    *opacity > 0.
                        && points.windows(2).any(|edge| {
                            segment_distance(local, edge[0], edge[1])
                                <= width / 2. + tolerance / self.scale
                        })
                }
                _ => false,
            })
    }

    /// A tight rectangle around the inked specimen in world coordinates.
    pub fn bounds(&self) -> crate::document::ContentBounds {
        use crate::document::ContentBounds;
        static BOUNDS: OnceLock<[ContentBounds; FOREST_SPECIMEN_COUNT]> = OnceLock::new();
        let local = BOUNDS.get_or_init(|| {
            std::array::from_fn(|variant| {
                vector::specimens()[variant]
                    .iter()
                    .filter_map(Shape::bounds)
                    .reduce(ContentBounds::union)
                    .expect("inked specimen")
            })
        })[self.variant];
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        [
            local.min,
            p(local.max.x, local.min.y),
            local.max,
            p(local.min.x, local.max.y),
        ]
        .map(|q| {
            let point = p(
                self.center.x + (q.x * cos - q.y * sin) * self.scale,
                self.center.y + (q.x * sin + q.y * cos) * self.scale,
            );
            ContentBounds {
                min: point,
                max: point,
            }
        })
        .into_iter()
        .reduce(ContentBounds::union)
        .expect("four corners")
    }
}
