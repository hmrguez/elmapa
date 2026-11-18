//! Rasterize an outlined area once, then choose the order of its native brushwork.
use super::{MAX_POINTS, Position, mix, point, smooth};
use elmapa::document::{Brush, Point, Stroke, TerrainReplay, limits::MAX_REPLAY_PATHS};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;
use std::{collections::VecDeque, ops::Range, sync::Arc};

const MAX_SCANLINE_STROKES: usize = 1024;
const MAX_BREADTH_FIRST_STROKES: usize = 16_384;

#[derive(Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "approach", deny_unknown_fields)]
pub(super) enum Drawing {
    /// Paint whole horizontal spans from top to bottom.
    #[default]
    Scanline,
    /// Paint nearby connected spans first and spread outward.
    BreadthFirst {
        /// Starting point inside the area, in world units with x right and y down.
        from: Position,
    },
    /// Sweep a fixed-radius brush along explicit paths while revealing a land or water area.
    BrushPaths {
        /// Brush radius in world units, between 1 and 1024.
        radius: f32,
        /// Ordered gestures. Each path is a sequence of world coordinates, x right and y down.
        paths: Vec<Vec<Position>>,
    },
}

#[derive(Clone, Copy)]
struct Span {
    left: f32,
    right: f32,
    y: f32,
    radius: f32,
}

impl Span {
    fn center(self) -> Point {
        Point::new(self.left.midpoint(self.right), self.y)
    }

    fn distance_sq(self, point: Point) -> f32 {
        let dx = point.x - point.x.clamp(self.left, self.right);
        let dy = point.y - self.y;
        dx * dx + dy * dy
    }

    fn points(self, from: Option<Point>) -> Vec<Point> {
        let left = Point::new(self.left, self.y);
        let right = Point::new(self.right, self.y);
        if left == right {
            return vec![left];
        }
        let Some(from) = from else {
            return vec![left, right];
        };
        if from.x <= self.left {
            vec![left, right]
        } else if from.x >= self.right {
            vec![right, left]
        } else {
            vec![Point::new(from.x, self.y), left, right]
        }
    }
}

struct Rows {
    spans: Vec<Span>,
    // Retain empty rows so separate pieces don't connect across a gap.
    ranges: Vec<Range<usize>>,
    pitch: f32,
}

pub(super) fn fill(
    mut outline: Vec<Point>,
    drawing: Drawing,
    brush: Brush,
    material: Option<Arc<str>>,
    seed: u64,
) -> Result<(Vec<Stroke>, Option<TerrainReplay>), String> {
    outline.dedup();
    if outline.len() > 1 && outline.first() == outline.last() {
        outline.pop();
    }
    if !(3..=MAX_POINTS).contains(&outline.len()) {
        return Err(format!("Area outlines need 3 to {MAX_POINTS} vertices."));
    }
    let surface = matches!(brush, Brush::Land | Brush::Water);
    let (origin, replay) = match drawing {
        Drawing::Scanline => (None, None),
        Drawing::BreadthFirst { from } => (
            Some(point(from)?),
            surface.then_some(TerrainReplay::BreadthFirst),
        ),
        Drawing::BrushPaths { radius, paths } => {
            if !surface {
                return Err("Brush paths draw land and water areas.".into());
            }
            if !(1. ..=1024.).contains(&radius) {
                return Err("Brush path radius must be between 1 and 1024 world units.".into());
            }
            super::check_count(paths.len(), MAX_REPLAY_PATHS, "brush paths")?;
            let paths = paths
                .into_iter()
                .map(|path| {
                    super::check_count(path.len(), MAX_POINTS, "points per brush path")?;
                    let points = path.into_iter().map(point).collect::<Result<Vec<_>, _>>()?;
                    Ok(smooth(points, false).into())
                })
                .collect::<Result<Vec<_>, String>>()?;
            (
                None,
                Some(TerrainReplay::BrushPaths {
                    radius,
                    paths: Arc::new(paths),
                }),
            )
        }
    };
    let mut outline = smooth(outline, true);
    outline.push(outline[0]);
    let rows = Rows::rasterize(&outline, origin)?;
    let order = match origin {
        None => rows.spans.iter().map(|&span| (span, None)).collect(),
        Some(origin) => rows.breadth_first(origin),
    };
    let strokes = order
        .into_iter()
        .map(|(span, from)| Stroke {
            creation_id: None,
            brush,
            points: span.points(from).into(),
            size: span.radius * 2.,
            strength: 1.,
            // A common seed keeps boundary roughness coherent between spans.
            seed,
            material: material.clone(),
        })
        .collect();
    Ok((strokes, replay))
}

fn crossings(outline: &[Point], y: f32, result: &mut Vec<f32>) {
    result.clear();
    for edge in outline.windows(2) {
        let [a, b] = [edge[0], edge[1]];
        if (a.y > y) != (b.y > y) {
            result.push(mix(a, b, (y - a.y) / (b.y - a.y)).x);
        }
    }
    result.sort_by(f32::total_cmp);
}

impl Rows {
    fn rasterize(outline: &[Point], origin: Option<Point>) -> Result<Self, String> {
        let top = outline.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let bottom = outline
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let height = bottom - top;
        if height < 8. {
            return Err("An area needs at least 8 world units of height.".into());
        }

        let radius = (height / 56.).clamp(6., 38.);
        let inset = radius.min(height / 4.);
        let count = ((height - inset * 2.) / (radius * 0.7)).ceil() as usize;
        if count > 512 {
            return Err("Area outline is too tall for editable brush fill.".into());
        }

        let mut edges = Vec::new();
        if let Some(origin) = origin {
            crossings(outline, origin.y, &mut edges);
            if !edges
                .chunks_exact(2)
                .any(|x| x[0] < origin.x && origin.x < x[1])
            {
                return Err(
                    "The breadth-first starting point must lie inside the area outline.".into(),
                );
            }
        }

        let breadth_first = origin.is_some();
        let limit = if breadth_first {
            MAX_BREADTH_FIRST_STROKES
        } else {
            MAX_SCANLINE_STROKES
        };
        let pitch = (height - inset * 2.) / count.max(1) as f32;
        let mut spans = Vec::new();
        let mut ranges = Vec::with_capacity(count + 1);
        for row in 0..=count {
            let y = top + inset + (height - inset * 2.) * row as f32 / count.max(1) as f32;
            crossings(outline, y, &mut edges);
            let first = spans.len();
            for pair in edges.chunks_exact(2) {
                let half_width = (pair[1] - pair[0]) * 0.5;
                if half_width < 0.5 {
                    continue;
                }
                let radius = inset.min(half_width);
                let left = pair[0] + radius;
                let right = (pair[1] - radius).max(left);
                let parts = if breadth_first {
                    (((right - left) / pitch).ceil() as usize).max(1)
                } else {
                    1
                };
                if parts > limit - spans.len() {
                    return Err(format!(
                        "Area outline produces too many brush strokes (limit {limit})."
                    ));
                }
                for part in 0..parts {
                    let x = |i| {
                        if i == parts {
                            right
                        } else {
                            left + (right - left) * i as f32 / parts as f32
                        }
                    };
                    spans.push(Span {
                        left: x(part),
                        right: x(part + 1),
                        y,
                        radius,
                    });
                }
            }
            ranges.push(first..spans.len());
        }
        if spans.is_empty() {
            return Err("Outline has no fillable interior.".into());
        }
        Ok(Self {
            spans,
            ranges,
            pitch,
        })
    }

    fn breadth_first(&self, from: Point) -> Vec<(Span, Option<Point>)> {
        let mut roots: Vec<_> = (0..self.spans.len()).collect();
        roots.sort_by(|&a, &b| {
            self.spans[a]
                .distance_sq(from)
                .total_cmp(&self.spans[b].distance_sq(from))
        });
        let mut visited = vec![false; self.spans.len()];
        let mut queue = VecDeque::new();
        let mut ordered = Vec::with_capacity(self.spans.len());
        for root in roots {
            if std::mem::replace(&mut visited[root], true) {
                continue;
            }
            queue.push_back((root, from));
            while let Some((index, parent)) = queue.pop_front() {
                let span = self.spans[index];
                ordered.push((span, Some(parent)));
                self.neighbors(index, |next| {
                    if !std::mem::replace(&mut visited[next], true) {
                        queue.push_back((next, span.center()));
                    }
                });
            }
        }
        ordered
    }

    fn neighbors(&self, index: usize, mut visit: impl FnMut(usize)) {
        let row = self.ranges.partition_point(|range| range.end <= index);
        let range = &self.ranges[row];
        let span = self.spans[index];
        if index > range.start && self.spans[index - 1].right == span.left {
            visit(index - 1);
        }
        if index + 1 < range.end && span.right == self.spans[index + 1].left {
            visit(index + 1);
        }
        // Adjacent rows overlap near the shoreline. A small sideways allowance
        // follows slanted edges while keeping the wave in neighboring paint.
        let reach = self.pitch * 0.5;
        for adjacent in [row.checked_sub(1), Some(row + 1)].into_iter().flatten() {
            let Some(range) = self.ranges.get(adjacent) else {
                continue;
            };
            let neighbors = &self.spans[range.clone()];
            let first = neighbors.partition_point(|other| other.right < span.left - reach);
            for (offset, other) in neighbors.iter().enumerate().skip(first) {
                if other.left > span.right + reach {
                    break;
                }
                let gap = (span.left - other.right)
                    .max(other.left - span.right)
                    .max(0.);
                if gap.hypot(span.y - other.y) <= span.radius + other.radius {
                    visit(range.start + offset);
                }
            }
        }
    }
}
