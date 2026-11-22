//! Sparse world-aligned terrain, shared by live painting, reopen and export.
mod wet_edge;

pub use wet_edge::WetEdge;

use super::{Batch, REGIONS};
use crate::{
    brush_materials,
    document::{AtlasStyle, Brush, ContentBounds, Layer, Point, Stroke},
    scene::{
        Shape, TerrainPalette, generation,
        noise::noise,
        terrain::{
            contours::RegionGrid,
            field::{cuts_region, radius, roughness},
            field_index::{FieldIndex, LiveDistance},
            materials::{Contours, coordinates},
            regions,
        },
    },
};
use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::{Arc, Weak},
};

const SIZE: f32 = 384.;
const APRON: f32 = 60.;
// The signed field is only used to interpolate zero crossings. Adjacent samples
// can differ by at most one diagonal cell plus twice the maximum brush noise.
const BAND: f32 = 60.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::scene::cache) struct Coord(i32, i32);

impl Coord {
    fn index(value: f32) -> i32 {
        ((value + SIZE * 0.5) / SIZE).floor() as i32
    }

    pub(in crate::scene::cache) fn bounds(self) -> ContentBounds {
        let min = Point::new(
            self.0 as f32 * SIZE - SIZE * 0.5,
            self.1 as f32 * SIZE - SIZE * 0.5,
        );
        ContentBounds {
            min,
            max: Point::new(min.x + SIZE, min.y + SIZE),
        }
    }
}

// Walk the padded segment by tile row. A diagonal crossing a very large map
// should visit tiles along the brush path, without allocating its bounding box.
fn footprint(points: &[Point], padding: f32, mut visit: impl FnMut(Coord)) {
    let Some(first) = points.first().copied() else {
        return;
    };
    let mut previous = first;
    for &next in points
        .iter()
        .skip(1)
        .chain(std::iter::once(&first).take(usize::from(points.len() == 1)))
    {
        let (a, b) = if previous.y <= next.y {
            (previous, next)
        } else {
            (next, previous)
        };
        for y in Coord::index(a.y - padding)..=Coord::index(b.y + padding) {
            if generation::stopped() {
                return;
            }
            let row_min = Coord(0, y).bounds().min.y;
            let at = |q: f32| a.x + (b.x - a.x) * ((q - a.y) / (b.y - a.y)).clamp(0., 1.);
            let (left, right) = if a.y == b.y {
                (a.x, b.x)
            } else {
                (at(row_min - padding), at(row_min + SIZE + padding))
            };
            for x in
                Coord::index(left.min(right) - padding)..=Coord::index(left.max(right) + padding)
            {
                visit(Coord(x, y));
            }
        }
        previous = next;
    }
}

fn influence(stroke: &Stroke) -> f32 {
    radius(stroke) + roughness(stroke) + BAND + 1.
}

#[derive(Clone, Default)]
struct Field {
    coverage: Option<Arc<RegionGrid>>,
    finishes: BTreeMap<usize, Arc<RegionGrid>>,
}

#[derive(Clone, Default)]
pub(in crate::scene::cache) struct TileRegion {
    pub surface: Arc<Contours>,
    pub batches: Vec<Batch>,
    pub palette: Option<Arc<TerrainPalette>>,
}

#[derive(Clone, Default)]
struct Tile {
    fields: [Field; 2],
    pub regions: [TileRegion; 2],
}

#[derive(Clone, Default)]
pub(in crate::scene::cache) struct TerrainTiles {
    tiles: Arc<BTreeMap<Coord, Arc<Tile>>>,
    style: AtlasStyle,
}

pub(in crate::scene::cache) struct TerrainIdentity(Weak<BTreeMap<Coord, Arc<Tile>>>);

impl TerrainIdentity {
    pub(in crate::scene::cache) fn matches(&self, tiles: &TerrainTiles) -> bool {
        self.0.as_ptr() == Arc::as_ptr(&tiles.tiles)
    }
}

#[derive(Default)]
struct TileHistory {
    strokes: Vec<usize>,
    painted: [Painted; 2],
}

#[derive(Default)]
struct Painted {
    coverage: bool,
    finishes: Vec<usize>,
}

fn brush_index(brush: Brush) -> Option<usize> {
    REGIONS.iter().position(|&region| region == brush)
}

fn texture(stroke: &Stroke, style: AtlasStyle) -> Option<usize> {
    let id = stroke.material.as_deref()?;
    brush_materials::for_brush(stroke.brush)
        .find(|(_, material)| material.id == id && material.texture.is_some())
        .map(|(index, _)| brush_materials::artwork_index(index, style))
}

impl TerrainTiles {
    pub(in crate::scene::cache) fn identity(&self) -> TerrainIdentity {
        TerrainIdentity(Arc::downgrade(&self.tiles))
    }

    pub(super) fn new(layer: &Layer, field: &FieldIndex, style: AtlasStyle) -> Self {
        let mut history: BTreeMap<Coord, TileHistory> = BTreeMap::new();
        for (stroke_index, stroke) in layer.strokes.iter().enumerate() {
            let brush = brush_index(stroke.brush);
            if stroke.strength <= 0. || (brush.is_none() && stroke.brush != Brush::Erase) {
                continue;
            }
            let finish = texture(stroke, style);
            footprint(&stroke.points, influence(stroke) + APRON, |coord| {
                let tile = match history.entry(coord) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) if generation::cells(32) => {
                        entry.insert(TileHistory::default())
                    }
                    Entry::Vacant(_) => return,
                };
                if tile.strokes.last() == Some(&stroke_index) || !generation::cells(1) {
                    return;
                }
                tile.strokes.push(stroke_index);
                if let Some(brush) = brush {
                    let paint = &mut tile.painted[brush];
                    paint.coverage = true;
                    if let Some(finish) = finish
                        && !paint.finishes.contains(&finish)
                    {
                        paint.finishes.push(finish);
                    }
                }
            });
        }
        let tiles = history
            .into_iter()
            .take_while(|_| !generation::stopped())
            .filter(|(_, tile)| tile.painted.iter().any(|paint| paint.coverage))
            .map(|(coord, tile)| {
                let bounds = coord.bounds().padded(APRON);
                let mut paints = tile.painted.into_iter();
                let fields = REGIONS.map(|brush| {
                    let paint = paints.next().expect("one paint record per region");
                    let sampled = |material: Option<&str>| {
                        Arc::new(RegionGrid::with_uniform_blocks(
                            bounds,
                            |q| field.sample_tile(q, brush, material, &tile.strokes, BAND),
                            |center, extent| {
                                field.uniform_tile_samples(
                                    center,
                                    extent,
                                    brush,
                                    material,
                                    &tile.strokes,
                                    BAND,
                                )
                            },
                        ))
                    };
                    Field {
                        coverage: paint.coverage.then(|| sampled(None)),
                        finishes: paint
                            .finishes
                            .into_iter()
                            .map(|id| {
                                (
                                    id,
                                    sampled(Some(&brush_materials::artwork_catalog()[id].id)),
                                )
                            })
                            .collect(),
                    }
                });
                (coord, Arc::new(Tile::new(coord, fields, None, style)))
            })
            .collect();
        Self {
            tiles: Arc::new(tiles),
            style,
        }
    }

    pub(super) fn apply(
        &mut self,
        base: &Self,
        stroke: &Stroke,
        distance: &LiveDistance,
        tail: &[Point],
    ) {
        let padding = influence(stroke);
        let mut dirty: BTreeMap<Coord, ContentBounds> = BTreeMap::new();
        // The sample rectangle is the union of relevant appended segments in
        // this tile. Distant tiles between widely separated bends stay clean.
        let segments = tail
            .windows(2)
            .map(|p| [p[0], p[1]])
            .chain(tail.first().filter(|_| tail.len() == 1).map(|&p| [p, p]));
        for [a, b] in segments {
            let bounds = ContentBounds { min: a, max: a }
                .union(ContentBounds { min: b, max: b })
                .padded(padding);
            footprint(&[a, b], padding + APRON, |coord| {
                dirty
                    .entry(coord)
                    .and_modify(|prior| *prior = prior.union(bounds))
                    .or_insert(bounds);
            });
        }
        let finish = texture(stroke, self.style);
        let tiles = Arc::make_mut(&mut self.tiles);
        let empty = Tile::default();
        for (coord, dirty_bounds) in dirty {
            if generation::stopped() {
                break;
            }
            let old = tiles.get(&coord).map_or(&empty, Arc::as_ref);
            let committed = base.tiles.get(&coord).map_or(&empty, Arc::as_ref);
            let bounds = coord.bounds().padded(APRON);
            let mut fields = old.fields.clone();
            for (index, brush) in REGIONS.into_iter().enumerate() {
                let matches = stroke.brush == brush;
                let cut = cuts_region(stroke.brush, brush);
                if !matches && (!cut || fields[index].coverage.is_none()) {
                    continue;
                }
                let old = &old.fields[index];
                let committed = &committed.fields[index];
                let dab = |q| {
                    radius(stroke) - distance.distance(q)
                        + noise(q, stroke.seed) * roughness(stroke)
                };
                let coverage = update_grid(old.coverage.as_ref(), bounds, dirty_bounds, |q| {
                    let base = sample(committed.coverage.as_ref(), q);
                    let dab = dab(q);
                    if cut { base.min(-dab) } else { base.max(dab) }
                });
                fields[index].coverage = Some(coverage);
                if matches || stroke.brush == Brush::Erase {
                    for id in old
                        .finishes
                        .keys()
                        .copied()
                        .chain(finish.filter(|id| matches && !old.finishes.contains_key(id)))
                    {
                        let next = update_grid(old.finishes.get(&id), bounds, dirty_bounds, |q| {
                            let base = sample(committed.finishes.get(&id), q);
                            let dab = dab(q);
                            if Some(id) == finish {
                                base.max(dab)
                            } else {
                                base.min(-dab)
                            }
                        });
                        fields[index].finishes.insert(id, next);
                    }
                }
            }
            if fields
                .iter()
                .zip(&old.fields)
                .any(|(next, prior)| !next.same(prior))
            {
                let next = Tile::new(coord, fields, Some(old), self.style);
                tiles.insert(coord, Arc::new(next));
            }
        }
    }

    pub(in crate::scene::cache) fn regions(
        &self,
        brush_index: usize,
    ) -> impl Iterator<Item = (Coord, &TileRegion)> {
        self.tiles
            .iter()
            .map(move |(&coord, tile)| (coord, &tile.regions[brush_index]))
    }

    pub(super) fn bytes(&self) -> usize {
        self.tiles
            .values()
            .map(|tile| {
                tile.fields
                    .iter()
                    .flat_map(|field| field.coverage.iter().chain(field.finishes.values()))
                    .map(|grid| grid.values.capacity() * 4 + grid.contour_bytes())
                    .sum::<usize>()
                    + tile
                        .regions
                        .iter()
                        .filter_map(|region| region.palette.as_ref())
                        .map(|palette| palette.pixels.capacity())
                        .sum::<usize>()
                    + tile
                        .regions
                        .iter()
                        .flat_map(|region| region.surface.iter())
                        .map(|ring| ring.capacity() * std::mem::size_of::<[f64; 2]>())
                        .sum::<usize>()
            })
            .sum()
    }
}

impl Field {
    fn same(&self, other: &Self) -> bool {
        self.coverage
            .as_ref()
            .zip(other.coverage.as_ref())
            .map_or_else(
                || self.coverage.is_none() && other.coverage.is_none(),
                |(a, b)| Arc::ptr_eq(a, b),
            )
            && self.finishes.len() == other.finishes.len()
            && self
                .finishes
                .iter()
                .zip(&other.finishes)
                .all(|((a, ga), (b, gb))| a == b && Arc::ptr_eq(ga, gb))
    }

    fn same_geometry(&self, other: &Self) -> bool {
        same_contours(self.coverage.as_ref(), other.coverage.as_ref())
            && self.finishes.len() == other.finishes.len()
            && self
                .finishes
                .iter()
                .zip(&other.finishes)
                .all(|((a, ga), (b, gb))| a == b && same_contours(Some(ga), Some(gb)))
    }
}

fn same_contours(a: Option<&Arc<RegionGrid>>, b: Option<&Arc<RegionGrid>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a.contours_ref() == b.contours_ref(),
        (None, None) => true,
        (Some(grid), None) | (None, Some(grid)) => grid.contours_ref().is_empty(),
    }
}

impl Tile {
    fn new(coord: Coord, fields: [Field; 2], old: Option<&Self>, style: AtlasStyle) -> Self {
        let land = fields[0].coverage.as_ref();
        let land_unchanged =
            old.is_some_and(|old| same_contours(land, old.fields[0].coverage.as_ref()));
        let water_unchanged = old.is_some_and(|old| fields[1].same_geometry(&old.fields[1]));
        let mut regions = REGIONS.map(|_| TileRegion::default());
        // The shelf samples the resolved water finishes. Build them before
        // the land artwork while retaining the usual water-then-land draw order.
        for (index, brush) in REGIONS.into_iter().enumerate().rev() {
            let field = &fields[index];
            if let Some(old) = old.filter(|old| {
                if brush == Brush::Land {
                    water_unchanged && field.same_geometry(&old.fields[index])
                } else {
                    land_unchanged && water_unchanged
                }
            }) {
                regions[index] = old.regions[index].clone();
            } else if let Some(coverage) = &field.coverage {
                let masks = field
                    .finishes
                    .iter()
                    .map(|(&id, grid)| (id, regions::contours(grid)))
                    .collect();
                let coast = (index == 1).then_some(land).flatten();
                let mut water_finishes = Vec::new();
                if brush == Brush::Land {
                    for batch in &regions[1].batches {
                        if let Some(Shape::RegionSurface {
                            contours,
                            material: Some(id),
                            ..
                        }) = batch.first()
                        {
                            water_finishes.push((
                                *id,
                                contours.iter().map(|ring| coordinates(ring)).collect(),
                            ));
                        }
                    }
                }
                let (surface, batches, palette) = regions::build(
                    coverage,
                    brush,
                    &masks,
                    coast,
                    &water_finishes,
                    coord.bounds(),
                    style,
                );
                let prior = old
                    .map(|old| &old.regions[index])
                    .filter(|prior| !prior.batches.is_empty());
                let old_palette = prior.and_then(|old| old.palette.as_ref());
                let palette_unchanged = old_palette.map(Arc::as_ref) == palette.as_ref();
                let palette = if palette_unchanged {
                    old_palette.cloned()
                } else {
                    palette.map(Arc::new)
                };
                regions[index] = TileRegion {
                    surface: prior
                        .filter(|old| *old.surface == surface)
                        .map_or_else(|| Arc::new(surface), |old| old.surface.clone()),
                    batches: batches
                        .into_iter()
                        .map(|batch| {
                            // The retained material is keyed to the batch. A palette
                            // change can reach it even when its contours stay fixed.
                            let uses_palette = matches!(&batch[..], [shape] if TerrainPalette::pigment(shape).is_some());
                            prior
                                .filter(|_| palette_unchanged || !uses_palette)
                                .and_then(|old| {
                                    old.batches
                                        .iter()
                                        .find(|previous| previous.as_ref() == &batch)
                                })
                                .cloned()
                                .unwrap_or_else(|| Arc::new(batch))
                        })
                        .collect(),
                    palette,
                };
            }
        }
        Self { fields, regions }
    }
}

fn sample(grid: Option<&Arc<RegionGrid>>, q: Point) -> f32 {
    grid.and_then(|grid| grid.sample_cached(q)).unwrap_or(-BAND)
}

fn grid(bounds: ContentBounds, mut sample: impl FnMut(Point) -> f32) -> Arc<RegionGrid> {
    Arc::new(RegionGrid::new(bounds, |q| sample(q).clamp(-BAND, BAND)))
}

fn update_grid(
    old: Option<&Arc<RegionGrid>>,
    bounds: ContentBounds,
    dirty: ContentBounds,
    mut sample: impl FnMut(Point) -> f32,
) -> Arc<RegionGrid> {
    if let Some(old) = old {
        old.apply_delta(dirty, |q| sample(q).clamp(-BAND, BAND))
            .map_or_else(|| old.clone(), Arc::new)
    } else {
        grid(bounds, sample)
    }
}
