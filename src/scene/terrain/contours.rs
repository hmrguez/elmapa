use super::super::geometry::{area, p};
#[cfg(test)]
use crate::document::{Brush, Layer};
use crate::document::{ContentBounds, Point};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::OnceLock,
};

/// Marching-squares contours merge brush dabs into continuous terrain boundaries.
#[cfg(test)]
fn connected_bounds(layer: &Layer, brush: Brush) -> Vec<ContentBounds> {
    let mut regions: Vec<ContentBounds> = Vec::new();
    for stroke in layer
        .strokes
        .iter()
        .filter(|s| s.brush == brush && s.strength > 0.0)
    {
        if super::super::generation::stopped() {
            break;
        }
        let Some(mut bounds) = stroke.bounds() else {
            continue;
        };
        // Restart after each union: expansion can connect an earlier region.
        let mut index = 0;
        while index < regions.len() {
            if bounds.intersects(regions[index]) {
                bounds = bounds.union(regions.swap_remove(index));
                index = 0;
            } else {
                index += 1;
            }
        }
        regions.push(bounds);
    }
    regions
}

#[cfg(test)]
pub(in crate::scene) fn contours(layer: &Layer, brush: Brush) -> Vec<Vec<Point>> {
    let mut loops: Vec<_> = connected_bounds(layer, brush)
        .into_iter()
        .flat_map(|bounds| {
            RegionGrid::new(bounds, |q| super::field::region_field(q, layer, brush))
                .contours_ref()
                .to_vec()
        })
        .collect();
    loops.sort_by(|a, b| area(b).abs().total_cmp(&area(a).abs()));
    loops
}

type Segment = ((u64, Point), (u64, Point));

pub(in crate::scene) struct RegionGrid {
    pub bounds: ContentBounds,
    loops: OnceLock<Vec<Vec<Point>>>,
    segments: OnceLock<BTreeMap<usize, Vec<Segment>>>,
    pub step: f32,
    pub origin: Point,
    pub nx: usize,
    pub ny: usize,
    pub values: Vec<f32>,
}

impl RegionGrid {
    pub fn new(bounds: ContentBounds, sample: impl FnMut(Point) -> f32) -> Self {
        Self::with_uniform_blocks(bounds, sample, |_, _| None)
    }

    pub fn with_uniform_blocks(
        bounds: ContentBounds,
        mut sample: impl FnMut(Point) -> f32,
        mut uniform: impl FnMut(Point, f32) -> Option<f32>,
    ) -> Self {
        // Bound work per connected region; distant islands keep their local detail.
        let step = (bounds.width().max(bounds.height()) / 192.0).max(12.0);
        let origin = p(
            (bounds.min.x / step).floor() * step,
            (bounds.min.y / step).floor() * step,
        );
        let nx = ((bounds.max.x - origin.x) / step).ceil() as usize + 3;
        let ny = ((bounds.max.y - origin.y) / step).ceil() as usize + 3;
        let _span = crate::performance::Span::new("region_grid");
        let mut grid = Self {
            bounds,
            loops: OnceLock::new(),
            segments: OnceLock::new(),
            step,
            origin,
            nx,
            ny,
            values: vec![-1.0; nx * ny],
        };
        const BLOCK: usize = 4;
        for y in (1..ny - 1).step_by(BLOCK) {
            for x in (1..nx - 1).step_by(BLOCK) {
                let x_end = (x + BLOCK).min(nx - 1);
                let y_end = (y + BLOCK).min(ny - 1);
                let low = grid.point(x, y);
                let high = grid.point(x_end - 1, y_end - 1);
                let center = p((low.x + high.x) * 0.5, (low.y + high.y) * 0.5);
                let extent = (high.x - low.x).hypot(high.y - low.y) * 0.5;
                if !super::super::generation::cells(1) {
                    return grid;
                }
                if let Some(value) = uniform(center, extent) {
                    for row in y..y_end {
                        grid.values[row * nx + x..row * nx + x_end].fill(value);
                    }
                } else {
                    let count = (x_end - x) * (y_end - y);
                    if !super::super::generation::cells(count) {
                        return grid;
                    }
                    super::super::work::update(|work| work.field_samples += count);
                    for row in y..y_end {
                        for col in x..x_end {
                            grid.values[row * nx + col] = sample(grid.point(col, row));
                        }
                    }
                }
            }
        }
        grid
    }

    /// Sample the changed part of the same world-aligned grid.
    pub fn apply_delta(
        &self,
        dirty: ContentBounds,
        mut sample: impl FnMut(Point) -> f32,
    ) -> Option<Self> {
        if !self.bounds.intersects(dirty) {
            return None;
        }
        let first = self.point(1, 1);
        let minx = (((dirty.min.x - first.x) / self.step).floor() as isize + 1)
            .clamp(1, self.nx as isize - 2) as usize;
        let maxx = (((dirty.max.x - first.x) / self.step).ceil() as isize + 1)
            .clamp(1, self.nx as isize - 2) as usize;
        let miny = (((dirty.min.y - first.y) / self.step).floor() as isize + 1)
            .clamp(1, self.ny as isize - 2) as usize;
        let maxy = (((dirty.max.y - first.y) / self.step).ceil() as isize + 1)
            .clamp(1, self.ny as isize - 2) as usize;
        let mut values = self.values.clone();
        let mut contour_dirty = None;
        let count = (maxy - miny + 1) * (maxx - minx + 1);
        if !super::super::generation::cells(count) {
            return None;
        }
        super::super::work::update(|work| work.field_samples += count);
        for y in miny..=maxy {
            for x in minx..=maxx {
                let q = self.point(x, y);
                let index = y * self.nx + x;
                let value = sample(q);
                if value != values[index] {
                    values[index] = value;
                    let point = ContentBounds { min: q, max: q };
                    contour_dirty = Some(
                        contour_dirty.map_or(point, |prior: ContentBounds| prior.union(point)),
                    );
                }
            }
        }
        let contour_dirty = contour_dirty?;
        let updated = Self {
            bounds: self.bounds,
            loops: OnceLock::new(),
            segments: OnceLock::new(),
            step: self.step,
            origin: self.origin,
            nx: self.nx,
            ny: self.ny,
            values,
        };
        if let Some(previous) = self.segments.get() {
            let column = |q: f32| ((q - first.x) / self.step).round() as usize + 1;
            let row = |q: f32| ((q - first.y) / self.step).round() as usize + 1;
            let minx = column(contour_dirty.min.x);
            let maxx = column(contour_dirty.max.x);
            let miny = row(contour_dirty.min.y);
            let maxy = row(contour_dirty.max.y);
            let mut cells = previous.clone();
            for y in miny.saturating_sub(1)..=maxy.min(self.ny - 2) {
                for x in minx.saturating_sub(1)..=maxx.min(self.nx - 2) {
                    super::super::work::update(|work| work.contour_cells += 1);
                    let edges = updated.cell_segments(x, y);
                    if edges.is_empty() {
                        cells.remove(&(y * self.nx + x));
                    } else {
                        cells.insert(y * self.nx + x, edges);
                    }
                }
            }
            let _ = updated.segments.set(cells);
        }
        Some(updated)
    }

    pub fn contour_bytes(&self) -> usize {
        self.segments.get().map_or(0, |cells| {
            cells
                .values()
                .map(|edges| edges.capacity() * std::mem::size_of::<Segment>() + 64)
                .sum()
        }) + self.loops.get().map_or(0, |loops| {
            loops
                .iter()
                .map(|points| points.capacity() * std::mem::size_of::<Point>())
                .sum()
        })
    }

    pub fn sample_cached(&self, q: Point) -> Option<f32> {
        let x = ((q.x - self.origin.x) / self.step + 1.).round() as isize;
        let y = ((q.y - self.origin.y) / self.step + 1.).round() as isize;
        if x < 1 || y < 1 || x >= self.nx as isize - 1 || y >= self.ny as isize - 1 {
            return None;
        }
        (self.point(x as usize, y as usize) == q)
            .then(|| self.values[y as usize * self.nx + x as usize])
    }

    pub fn point(&self, x: usize, y: usize) -> Point {
        p(
            self.origin.x + (x as f32 - 1.0) * self.step,
            self.origin.y + (y as f32 - 1.0) * self.step,
        )
    }

    pub fn contours_ref(&self) -> &[Vec<Point>] {
        self.loops.get_or_init(|| self.compute_contours())
    }

    fn cell_segments(&self, x: usize, y: usize) -> Vec<Segment> {
        let (step, origin, nx, grid) = (self.step, self.origin, self.nx, &self.values);
        let mut segments = Vec::new();
        let values = [
            grid[y * nx + x],
            grid[y * nx + x + 1],
            grid[(y + 1) * nx + x + 1],
            grid[(y + 1) * nx + x],
        ];
        let bits = values
            .iter()
            .enumerate()
            .fold(0_u8, |b, (i, v)| b | if *v > 0.0 { 1 << i } else { 0 });
        if bits == 0 || bits == 15 {
            return Vec::new();
        }
        let corners = [
            p(
                origin.x + (x as f32 - 1.0) * step,
                origin.y + (y as f32 - 1.0) * step,
            ),
            p(
                origin.x + x as f32 * step,
                origin.y + (y as f32 - 1.0) * step,
            ),
            p(origin.x + x as f32 * step, origin.y + y as f32 * step),
            p(
                origin.x + (x as f32 - 1.0) * step,
                origin.y + y as f32 * step,
            ),
        ];
        let keys = [
            ((y * nx + x) * 2) as u64,
            ((y * nx + x + 1) * 2 + 1) as u64,
            (((y + 1) * nx + x) * 2) as u64,
            ((y * nx + x) * 2 + 1) as u64,
        ];
        let edge = |e: usize| {
            let (a, b) = [(0, 1), (1, 2), (3, 2), (0, 3)][e];
            let t = (values[a] / (values[a] - values[b])).clamp(0.0, 1.0);
            (
                keys[e],
                p(
                    corners[a].x + (corners[b].x - corners[a].x) * t,
                    corners[a].y + (corners[b].y - corners[a].y) * t,
                ),
            )
        };
        let pairs: &[(usize, usize)] = match bits {
            1 | 14 => &[(3, 0)],
            2 | 13 => &[(0, 1)],
            3 | 12 => &[(3, 1)],
            4 | 11 => &[(1, 2)],
            5 => &[(3, 0), (1, 2)],
            6 | 9 => &[(0, 2)],
            7 | 8 => &[(3, 2)],
            10 => &[(0, 1), (2, 3)],
            _ => &[],
        };
        for &(a, b) in pairs {
            segments.push((edge(a), edge(b)));
        }
        segments
    }

    fn compute_contours(&self) -> Vec<Vec<Point>> {
        let _span = crate::performance::Span::new("contours");
        let cells = self.segments.get_or_init(|| {
            let mut cells = BTreeMap::new();
            for y in 0..self.ny - 1 {
                if super::super::generation::stopped() {
                    break;
                }
                for x in 0..self.nx - 1 {
                    super::super::work::update(|work| work.contour_cells += 1);
                    let edges = self.cell_segments(x, y);
                    if !edges.is_empty() {
                        cells.insert(y * self.nx + x, edges);
                    }
                }
            }
            cells
        });
        let segments: Vec<_> = cells
            .values()
            .flat_map(|edges| edges.iter().copied())
            .collect();
        let mut links: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, (a, b)) in segments.iter().enumerate() {
            links.entry(a.0).or_default().push(i);
            links.entry(b.0).or_default().push(i);
        }
        let mut used = HashSet::new();
        let mut loops = Vec::new();
        for first in 0..segments.len() {
            if used.contains(&first) {
                continue;
            }
            let mut current = first;
            let mut key = segments[first].0.0;
            let start = key;
            let mut points = Vec::new();
            loop {
                if !used.insert(current) {
                    break;
                }
                let (a, b) = segments[current];
                let (from, to) = if a.0 == key { (a, b) } else { (b, a) };
                points.push(from.1);
                key = to.0;
                if key == start {
                    break;
                }
                let next = links
                    .get(&key)
                    .and_then(|items| items.iter().find(|i| !used.contains(i)))
                    .copied();
                match next {
                    Some(next) => current = next,
                    None => {
                        points.push(to.1);
                        break;
                    }
                }
            }
            if points.len() > 3 {
                loops.push(points);
            }
        }
        loops.sort_by(|a, b| area(b).abs().total_cmp(&area(a).abs()));
        loops
    }
}
