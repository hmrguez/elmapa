//! World-aligned first-contact times shared by every tile of an ordered surface.
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::{Assets, Handle, Image, Vec2, Vec4},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use elmapa::document::{Brush, ContentBounds, Layer, Point, SharedVec, Stroke, TerrainReplay};
use std::{cmp::Reverse, collections::BinaryHeap};

pub(super) const SAMPLE_BUDGET: usize = 16_000_000;
const BREADTH_FIRST_DURATION: f32 = 1.8;
const MAX_SIDE: u32 = 512;
const CELL_SIZE: f32 = 4.;

#[derive(Clone)]
pub(super) struct PaintOrder {
    pub texture: Handle<Image>,
    pub position: Vec4,
    pub duration: f32,
}

pub(super) fn prepare(
    layer: &Layer,
    brush: Brush,
    view: ContentBounds,
    images: &mut Assets<Image>,
    budget: &mut usize,
) -> Option<(PaintOrder, usize)> {
    let mut strokes: Vec<_> = layer
        .strokes
        .iter()
        .enumerate()
        .filter(|(_, stroke)| stroke.brush == brush && stroke.strength > 0.)
        .collect();
    strokes.sort_by_key(|&(_, stroke)| stroke.creation_id);
    let (first, stroke) = *strokes.first()?;
    let seed = stroke.points.first()?;
    let bounds = strokes
        .iter()
        .filter_map(|(_, stroke)| stroke.bounds())
        .reduce(ContentBounds::union)?;
    let view = view.padded(CELL_SIZE);
    let origin = Vec2::new(bounds.min.x.max(view.min.x), bounds.min.y.max(view.min.y));
    let end = Vec2::new(bounds.max.x.min(view.max.x), bounds.max.y.min(view.max.y));
    let span = end - origin;
    if span.min_element() <= 0. {
        return None;
    }
    let size = (span / CELL_SIZE)
        .ceil()
        .clamp(Vec2::ONE, Vec2::splat(MAX_SIDE as f32));
    let mut grid = Grid {
        origin,
        step: span / size,
        width: size.x as usize,
        height: size.y as usize,
        cells: vec![255; size.x as usize * size.y as usize],
    };
    let duration = match layer.terrain_replay.as_ref()? {
        TerrainReplay::BreadthFirst => {
            let last = strokes.len().saturating_sub(1).max(1);
            for (index, (_, stroke)) in strokes.into_iter().enumerate() {
                grid.stamp(stroke, (index * 254 / last) as f32, brush, budget)?;
            }
            grid.spread(Vec2::new(seed.x, seed.y));
            BREADTH_FIRST_DURATION
        }
        TerrainReplay::BrushPaths { radius, paths } => {
            grid.trace(paths, *radius, budget)?;
            (1.8 + paths.len() as f32 * 0.3).min(4.2)
        }
    };
    let mut image = Image::new(
        Extent3d {
            width: grid.width as u32,
            height: grid.height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        grid.cells,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    let position = Vec4::new(origin.x, origin.y, span.x.recip(), span.y.recip());
    Some((
        PaintOrder {
            texture: images.add(image),
            position,
            duration,
        },
        first,
    ))
}

struct Grid {
    origin: Vec2,
    step: Vec2,
    width: usize,
    height: usize,
    cells: Vec<u8>,
}

impl Grid {
    fn spread(&mut self, seed: Vec2) {
        let at = ((seed - self.origin) / self.step).floor();
        let preferred =
            (at.x >= 0. && at.y >= 0. && at.x < self.width as f32 && at.y < self.height as f32)
                .then(|| at.y as usize * self.width + at.x as usize);
        let support = std::mem::replace(&mut self.cells, vec![255; self.width * self.height]);
        let mut order: Vec<_> = (0..support.len()).filter(|&i| support[i] < 255).collect();
        order.sort_by_key(|&i| (Some(i) != preferred, support[i]));
        let mut distances = vec![u32::MAX; support.len()];
        let mut queue = BinaryHeap::new();
        let horizontal = (self.step.x * 256.).round().max(1.) as u32;
        let vertical = (self.step.y * 256.).round().max(1.) as u32;
        let diagonal = (self.step.length() * 256.).round().max(1.) as u32;
        for root in order {
            if distances[root] != u32::MAX {
                continue;
            }
            distances[root] = 0;
            queue.push(Reverse((0u32, root)));
            let mut island = Vec::new();
            let start = support[root];
            let mut end = start;
            while let Some(Reverse((dist, here))) = queue.pop() {
                if dist != distances[here] {
                    continue;
                }
                island.push(here);
                end = end.max(support[here]);
                let x = here % self.width;
                let y = here / self.width;
                let left = x.saturating_sub(1);
                let top = y.saturating_sub(1);
                for ny in top..=(y + 1).min(self.height - 1) {
                    for nx in left..=(x + 1).min(self.width - 1) {
                        let target = ny * self.width + nx;
                        if target == here || support[target] == 255 {
                            continue;
                        }
                        let cost = match (nx != x, ny != y) {
                            (true, true) => {
                                // Stay in the painted footprint when going around a corner.
                                if support[y * self.width + nx] == 255
                                    && support[ny * self.width + x] == 255
                                {
                                    continue;
                                }
                                diagonal
                            }
                            (true, false) => horizontal,
                            _ => vertical,
                        };
                        let next = dist.saturating_add(cost);
                        if next < distances[target] {
                            distances[target] = next;
                            queue.push(Reverse((next, target)));
                        }
                    }
                }
            }
            // Each disconnected piece gets its own wave in the saved order.
            let span = distances[*island.last().expect("the seed is visited")].max(1);
            for cell in island {
                let elapsed = u64::from(distances[cell]) * u64::from(end - start);
                self.cells[cell] = start + (elapsed / u64::from(span)) as u8;
            }
        }
    }

    fn stamp(
        &mut self,
        stroke: &Stroke,
        phase: f32,
        brush: Brush,
        budget: &mut usize,
    ) -> Option<()> {
        // Cover the rough brush edge and the land wash that extends off the shore.
        // The retained mesh still clips the actual visible ink.
        let roughness = (stroke.size * 0.07).clamp(4., 18.);
        let coast = if brush == Brush::Land { 18. } else { 0. };
        let reach = stroke.size.max(2.) * 0.5 * stroke.strength.sqrt()
            + roughness
            + coast
            + self.step.max_element() * 0.5;
        let first = stroke.points.first()?;
        if stroke.points.len() == 1 {
            let center = Vec2::new(first.x, first.y);
            return self.segment(center, center, reach, [phase; 2], budget);
        }
        for pair in stroke.points.windows(2) {
            let start = Vec2::new(pair[0].x, pair[0].y);
            let end = Vec2::new(pair[1].x, pair[1].y);
            self.segment(start, end, reach, [phase; 2], budget)?;
        }
        Some(())
    }

    fn trace(&mut self, paths: &[SharedVec<Point>], radius: f32, budget: &mut usize) -> Option<()> {
        let length = |pair: &[Point]| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y);
        let pause = radius * 0.35;
        let duration = |path: &SharedVec<Point>| {
            path.windows(2).map(length).sum::<f32>().max(radius * 0.45) + pause
        };
        let scale = 244. / paths.iter().map(duration).sum::<f32>().max(1.);
        let reach = radius + self.step.max_element() * 0.5;
        let mut elapsed = 0.;
        for path in paths {
            let start = elapsed;
            let first = path.first()?;
            if path.len() == 1 {
                let center = Vec2::new(first.x, first.y);
                self.segment(center, center, reach, [elapsed * scale; 2], budget)?;
            }
            for pair in path.windows(2) {
                let a = Vec2::new(pair[0].x, pair[0].y);
                let b = Vec2::new(pair[1].x, pair[1].y);
                let end = elapsed + length(pair);
                self.segment(a, b, reach, [elapsed * scale, end * scale], budget)?;
                elapsed = end;
            }
            elapsed = start + duration(path);
        }
        Some(())
    }

    fn segment(
        &mut self,
        start: Vec2,
        end: Vec2,
        radius: f32,
        phase: [f32; 2],
        budget: &mut usize,
    ) -> Option<()> {
        let lower = ((start.min(end) - Vec2::splat(radius) - self.origin) / self.step)
            .floor()
            .max(Vec2::ZERO);
        let upper = ((start.max(end) + Vec2::splat(radius) - self.origin) / self.step)
            .ceil()
            .min(Vec2::new(self.width as f32, self.height as f32));
        if (upper - lower).min_element() <= 0. {
            return Some(());
        }
        let xs = lower.x as usize..upper.x as usize;
        let ys = lower.y as usize..upper.y as usize;
        let cost = xs.len() * ys.len();
        if cost > *budget {
            *budget = 0;
            return None;
        }
        *budget -= cost;
        let axis = end - start;
        let inverse = axis.length_squared().max(f32::EPSILON).recip();
        let radius_sq = radius * radius;
        for y in ys {
            let mut from_start =
                self.origin + Vec2::new(xs.start as f32 + 0.5, y as f32 + 0.5) * self.step - start;
            for cell in &mut self.cells[y * self.width + xs.start..y * self.width + xs.end] {
                if f32::from(*cell) > phase[0] {
                    let projection = from_start.dot(axis) * inverse;
                    let closest = projection.clamp(0., 1.);
                    if (from_start - axis * closest).length_squared() <= radius_sq {
                        let progress = if phase[0] == phase[1] {
                            phase[0]
                        } else {
                            // Intersect the advancing circular brush with this texel.
                            let across = (from_start - axis * projection).length_squared();
                            let advance = ((radius_sq - across).max(0.) * inverse).sqrt();
                            let enter = (projection - advance).clamp(0., 1.);
                            phase[0] + enter * (phase[1] - phase[0])
                        };
                        *cell = (*cell).min(progress.round() as u8);
                    }
                }
                from_start.x += self.step.x;
            }
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_brush_follows_the_path_and_lifts_between_gestures() {
        let mut grid = Grid {
            origin: Vec2::ZERO,
            step: Vec2::ONE,
            width: 30,
            height: 24,
            cells: vec![255; 30 * 24],
        };
        let paths: Vec<SharedVec<Point>> = vec![
            vec![Point::new(22., 4.), Point::new(5., 4.), Point::new(5., 12.)].into(),
            vec![Point::new(24., 18.)].into(),
        ];
        let mut budget = SAMPLE_BUDGET;
        grid.trace(&paths, 2., &mut budget).unwrap();
        let at = |x: usize, y: usize| grid.cells[y * 30 + x];
        assert!(at(21, 4) < at(8, 4) && at(8, 4) < at(5, 11));
        assert!(at(5, 11) < at(24, 18));
        assert_eq!(at(14, 15), 255);
    }

    #[test]
    fn reveal_prefers_shorter_world_distance_and_follows_a_connected_shape() {
        let mut grid = Grid {
            origin: Vec2::ZERO,
            step: Vec2::ONE,
            width: 29,
            height: 29,
            cells: vec![254; 29 * 29],
        };
        let index = |x: usize, y: usize| y * 29 + x;
        grid.cells[index(10, 10)] = 0;
        grid.spread(Vec2::new(10.5, 10.5));
        assert!(grid.cells[index(22, 10)] < grid.cells[index(20, 20)]);

        grid.cells.fill(255);
        for y in 0..29 {
            for x in 0..29 {
                if !(7..22).contains(&x) || y >= 22 {
                    grid.cells[index(x, y)] = 254;
                }
            }
        }
        grid.cells[index(6, 0)] = 0;
        grid.spread(Vec2::new(6.5, 0.5));
        assert_eq!(grid.cells[index(14, 10)], 255);
        assert!(grid.cells[index(14, 24)] < grid.cells[index(22, 0)]);
    }
}
