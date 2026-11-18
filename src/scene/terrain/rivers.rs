//! Original engraved river channels. The pointer supplies the course; paper,
//! asymmetrical banks and interrupted longitudinal penwork supply the material.
use super::super::{
    Shape,
    geometry::{distance, p, sampled},
    noise::hash,
    shape::{line, polygon},
};
use super::field::erased;
use crate::{
    brush_materials,
    document::{Brush, Layer, Point, Stroke},
};

pub const CURRENT_INK: u32 = 0x8b9586;
const CHANNEL_FILL: u32 = 0xd2d4bf;
const CHANNEL_HIGHLIGHT: u32 = 0xe6e7d2;
const STEP: f32 = 4.;
const REACH: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct RiverReach {
    pub id: (u64, usize, u64, usize),
    pub course: Vec<Point>,
    pub distance: f32,
    pub length: f32,
    pub half_width: f32,
    pub channel_fill: u32,
    /// Allow the moving front to throw droplets outside the channel on dry land.
    pub spray: bool,
}

impl RiverReach {
    pub fn is_water_fill(&self, color: u32) -> bool {
        color == self.channel_fill || color == CHANNEL_HIGHLIGHT
    }

    /// Arc length at a tessellated vertex, including bank overhang. Projection
    /// onto this small reach follows bends rather than a global x/y direction.
    pub fn coordinate(&self, q: Point) -> f32 {
        self.coordinates(q).0
    }
    /// Longitudinal distance and signed transverse distance in channel widths.
    /// End projections extend slightly past the artwork for native spray.
    pub fn coordinates(&self, q: Point) -> (f32, f32) {
        let mut best = f32::INFINITY;
        let mut along = 0.;
        let mut across = 0.;
        let mut offset = 0.;
        for (i, pair) in self.course.windows(2).enumerate() {
            let a = pair[0];
            let b = pair[1];
            let len = distance(a, b).max(0.001);
            let mut t = ((q.x - a.x) * (b.x - a.x) + (q.y - a.y) * (b.y - a.y)) / (len * len);
            if i > 0 {
                t = t.max(0.);
            }
            if i + 2 < self.course.len() {
                t = t.min(1.);
            }
            let d = distance(q, mix(a, b, t));
            if d < best {
                best = d;
                along = offset + t * len;
                across = ((b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x))
                    / len
                    / self.half_width.max(1.);
            }
            offset += len;
        }
        (self.distance + along, across)
    }
}

pub(in crate::scene) struct RiverBatch {
    pub shapes: Vec<Shape>,
    pub reach: RiverReach,
}

#[derive(Clone, Copy)]
struct Station {
    q: Point,
    normal: Point,
    left: f32,
    right: f32,
    s: f32,
}
impl Station {
    fn offset(self, amount: f32) -> Point {
        p(
            self.q.x + self.normal.x * amount,
            self.q.y + self.normal.y * amount,
        )
    }
    fn lane(self, t: f32) -> Point {
        self.offset(if t < 0. {
            self.left * t
        } else {
            self.right * t
        })
    }
}
fn mix(a: Point, b: Point, t: f32) -> Point {
    p(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
fn course(stroke: &Stroke) -> Vec<Point> {
    let samples = sampled(stroke, STEP);
    if samples.len() < 3 {
        return samples;
    }
    // A brush-width-scaled triangular filter rounds bends and absorbs small
    // coalesced-pointer reversals before they can fold the banks across water.
    // Only the last two filter windows can change as the course extends.
    let radius = (stroke.size * 0.25 / STEP).ceil().clamp(2., 12.) as usize;
    let mut points = samples;
    for _ in 0..2 {
        let previous = points.clone();
        for i in 1..points.len() - 1 {
            let mut sum = p(0., 0.);
            let mut weight = 0.;
            for d in -(radius as isize)..=radius as isize {
                let q = previous[i.saturating_add_signed(d).min(previous.len() - 1)];
                let w = (radius + 1 - d.unsigned_abs()) as f32;
                sum.x += q.x * w;
                sum.y += q.y * w;
                weight += w;
            }
            points[i] = p(sum.x / weight, sum.y / weight);
        }
    }
    points
}

pub(in crate::scene) fn river_batches(
    layer: &Layer,
    index: usize,
    stroke: &Stroke,
) -> Vec<RiverBatch> {
    if stroke.strength <= 0. || stroke.points.is_empty() {
        return Vec::new();
    }
    let channel_fill = stroke.material.as_deref().map_or(CHANNEL_FILL, |id| {
        brush_materials::for_brush(Brush::Water)
            .find(|(_, material)| material.id == id)
            .map_or(CHANNEL_FILL, |(_, material)| material.color)
    });
    let points = course(stroke);
    if points.len() == 1 {
        return spring(layer, index, stroke, points[0], channel_fill);
    }
    let half = (stroke.size * 0.105).clamp(2., 22.);
    let phase = hash(stroke.seed) * std::f32::consts::TAU;
    let mut s = 0.;
    let mut stations = Vec::with_capacity(points.len());
    for (i, q) in points.iter().copied().enumerate() {
        if i > 0 {
            s += distance(points[i - 1], q);
        }
        let a = points[i.saturating_sub(1)];
        let b = points[(i + 1).min(points.len() - 1)];
        let len = distance(a, b).max(0.001);
        let normal = p(-(b.y - a.y) / len, (b.x - a.x) / len);
        let width = half * (0.92 + 0.10 * (s / 39. + phase).sin());
        stations.push(Station {
            q,
            normal,
            left: width * (0.97 + 0.10 * (s / 19. + phase).sin()),
            right: width * (1.03 + 0.09 * (s / 27. - phase).cos()),
            s,
        });
    }
    let shade = stroke.strength.clamp(0., 1.);
    let alpha = layer.opacity;
    let mut batches = Vec::new();
    for start in (0..stations.len().saturating_sub(1)).step_by(REACH) {
        if super::super::generation::stopped() {
            break;
        }
        let end = (start + REACH).min(stations.len() - 1);
        let reach = &stations[start..=end];
        let mut out = Vec::new();
        // Test each tiny interval: erasing leaves no bridge across a removed
        // reach, and painting after the eraser still survives by chronology.
        let mut fragment = Vec::new();
        for pair in reach.windows(2) {
            if erased(pair[0].q, layer, index)
                || erased(pair[1].q, layer, index)
                || erased(mix(pair[0].q, pair[1].q, 0.5), layer, index)
            {
                draw(&mut out, &fragment, shade, alpha, channel_fill);
                fragment.clear();
            } else {
                if fragment.is_empty() {
                    fragment.push(pair[0]);
                }
                fragment.push(pair[1]);
            }
        }
        draw(&mut out, &fragment, shade, alpha, channel_fill);
        batches.push(RiverBatch {
            shapes: out,
            reach: RiverReach {
                id: (layer.id, index, stroke.seed, start / REACH),
                course: reach.iter().map(|station| station.q).collect(),
                distance: reach[0].s,
                length: reach.last().unwrap().s - reach[0].s,
                half_width: half,
                channel_fill,
                spray: true,
            },
        });
    }
    batches
}

fn spring(
    layer: &Layer,
    index: usize,
    stroke: &Stroke,
    q: Point,
    channel_fill: u32,
) -> Vec<RiverBatch> {
    if erased(q, layer, index) {
        return Vec::new();
    }
    let radius = (stroke.size * 0.07).clamp(2., 12.);
    let rim: Vec<_> = (0..=32)
        .map(|i| {
            let t = i as f32 * std::f32::consts::TAU / 32.;
            let r = 1. + (t * 3. + hash(stroke.seed) * 6.).sin() * 0.06;
            p(
                q.x + t.cos() * radius * 1.2 * r,
                q.y + t.sin() * radius * 0.8 * r,
            )
        })
        .collect();
    let mut shapes = Vec::new();
    polygon(&mut shapes, rim.clone(), channel_fill, layer.opacity * 0.97);
    line(
        &mut shapes,
        rim,
        0x75654f,
        0.72,
        layer.opacity * (0.48 + stroke.strength * 0.30),
    );
    for side in [-0.26, 0.26] {
        line(
            &mut shapes,
            (-3..=3)
                .map(|i| {
                    let t = i as f32 / 4.;
                    p(
                        q.x + t * radius,
                        q.y + side * radius + (t * t - 0.3) * radius * 0.12,
                    )
                })
                .collect(),
            CURRENT_INK,
            0.55,
            layer.opacity * 0.5,
        );
    }
    vec![RiverBatch {
        shapes,
        reach: RiverReach {
            id: (layer.id, index, stroke.seed, 0),
            course: vec![p(q.x - radius, q.y), p(q.x + radius, q.y)],
            distance: 0.,
            length: radius * 2.,
            half_width: radius,
            channel_fill,
            spray: true,
        },
    }]
}

fn ribbon(stations: &[Station], scale: f32, margin: f32) -> Vec<Point> {
    stations
        .iter()
        .map(|a| a.offset(-a.left * scale - margin))
        .chain(
            stations
                .iter()
                .rev()
                .map(|a| a.offset(a.right * scale + margin)),
        )
        .collect()
}
fn draw(out: &mut Vec<Shape>, stations: &[Station], shade: f32, alpha: f32, channel_fill: u32) {
    if stations.len() < 2 {
        return;
    }
    // Warm bank wash, pale mineral water, then two separate engraved contours.
    polygon(
        out,
        ribbon(stations, 1., 1.8),
        0xc2af85,
        alpha * (0.10 + shade * 0.10),
    );
    polygon(out, ribbon(stations, 1., 0.), channel_fill, alpha * 0.97);
    if channel_fill == CHANNEL_FILL {
        polygon(
            out,
            ribbon(stations, 0.72, 0.),
            CHANNEL_HIGHLIGHT,
            alpha * 0.70,
        );
    }
    for side in [-1., 1.] {
        let bank: Vec<_> = stations.iter().map(|a| a.lane(side)).collect();
        line(out, bank, 0x75654f, 0.72, alpha * (0.48 + shade * 0.30));
        line(
            out,
            stations.iter().map(|a| a.lane(side * 0.83)).collect(),
            0xa89a78,
            0.45,
            alpha * shade * 0.38,
        );
    }
    // Broken longitudinal strokes, with staggered lengths and lanes. Global
    // arc-length patterns continue through reach boundaries without a seam.
    for (lane, period, duty) in [(-0.43, 23., 0.54), (0.06, 31., 0.49), (0.46, 19., 0.43)] {
        let mut pen = Vec::new();
        for station in stations {
            let phase = (station.s + lane * 37.).rem_euclid(period) / period;
            if phase < duty {
                pen.push(station.lane(lane + (station.s / 12.).sin() * 0.05));
            } else {
                line(
                    out,
                    std::mem::take(&mut pen),
                    CURRENT_INK,
                    0.6,
                    alpha * (0.30 + shade * 0.32),
                );
            }
        }
        line(out, pen, CURRENT_INK, 0.6, alpha * (0.30 + shade * 0.32));
    }
    // Sparse short bank hachures share the mountains' sepia engraving.
    for station in stations
        .iter()
        .filter(|a| (a.s / 12.).floor() as i32 % 3 == 1)
        .step_by(3)
    {
        for side in [-1., 1.] {
            let edge = station.lane(side);
            let length = 1.2 + shade * 1.8;
            line(
                out,
                vec![
                    station.offset(if side < 0. {
                        -station.left - length
                    } else {
                        station.right + length
                    }),
                    p(
                        edge.x - station.normal.y * 1.2,
                        edge.y + station.normal.x * 1.2,
                    ),
                ],
                0x968165,
                0.48,
                alpha * shade * 0.30,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Brush, Document};
    fn river() -> Stroke {
        Stroke {
            creation_id: None,
            material: None,
            brush: Brush::River,
            points: vec![p(100., 200.), p(400., 200.)].into(),
            size: 110.,
            strength: 0.85,
            seed: 7,
        }
    }
    #[test]
    fn banks_follow_a_variable_width_course_and_strength_changes_ink_not_channel() {
        let doc = Document::blank();
        let mut stroke = river();
        let full = river_batches(&doc.layers[0], 0, &stroke);
        assert!(full.len() > 5);
        let polygons: Vec<_> = full
            .iter()
            .flat_map(|b| &b.shapes)
            .filter_map(|s| match s {
                Shape::Polygon {
                    fill: 0xd2d4bf,
                    points,
                    ..
                } => Some(points),
                _ => None,
            })
            .collect();
        let source_width = |points: &[Point]| distance(points[0], *points.last().unwrap());
        let normal_width = stroke.size * 0.105 * 2.;
        assert!(
            (0.76..=1.22).contains(&(source_width(polygons[0]) / normal_width)),
            "river started as a tapered source"
        );
        let mut short = stroke.clone();
        short.points = vec![p(100., 200.), p(104., 200.)].into();
        let beginning = river_batches(&doc.layers[0], 0, &short);
        let first_water = beginning[0]
            .shapes
            .iter()
            .find_map(|s| match s {
                Shape::Polygon {
                    fill: 0xd2d4bf,
                    points,
                    ..
                } => Some(points),
                _ => None,
            })
            .unwrap();
        assert_eq!(source_width(first_water), source_width(polygons[0]));
        stroke.strength = 0.1;
        let pale = river_batches(&doc.layers[0], 0, &stroke);
        for (a, b) in full
            .iter()
            .flat_map(|b| &b.shapes)
            .filter(|s| {
                !matches!(
                    s,
                    Shape::Line {
                        color: 0x968165,
                        ..
                    }
                )
            })
            .zip(pale.iter().flat_map(|b| &b.shapes).filter(|s| {
                !matches!(
                    s,
                    Shape::Line {
                        color: 0x968165,
                        ..
                    }
                )
            }))
        {
            match (a, b) {
                (Shape::Polygon { points: a, .. }, Shape::Polygon { points: b, .. })
                | (Shape::Line { points: a, .. }, Shape::Line { points: b, .. }) => {
                    assert_eq!(a, b)
                }
                _ => panic!("strength changed channel topology"),
            }
        }
        assert!(full.iter().flat_map(|b| &b.shapes).any(|s| matches!(
            s,
            Shape::Line {
                color: CURRENT_INK,
                ..
            }
        )));
        stroke.strength = 0.;
        assert!(river_batches(&doc.layers[0], 0, &stroke).is_empty());
    }
    #[test]
    fn short_pointer_reversals_do_not_fold_banks_across_the_channel() {
        let mut stroke = river();
        let mut points = Vec::new();
        for i in 0..80 {
            let t = i as f32 / 79.;
            let q = p(200. + (t * 9.).sin() * 60., 100. + t * 380.);
            points.push(q);
            if i == 16 || i == 56 {
                points.push(p(q.x + 12., q.y - 18.));
                points.push(p(q.x - 3., q.y + 3.));
            }
        }
        stroke.points = points.into();
        let smoothed = course(&stroke);
        assert!(
            smoothed.windows(2).all(|p| p[1].y >= p[0].y),
            "pointer jitter folded a downstream bank"
        );
        assert_eq!(smoothed[0], stroke.points[0]);
        assert_eq!(smoothed.last(), stroke.points.last());
    }
    #[test]
    fn erasing_breaks_both_banks_and_water_without_reconnecting_the_gap() {
        let mut doc = Document::blank();
        let stroke = river();
        let original = river_batches(&doc.layers[0], 0, &stroke);
        doc.layers[0].strokes.push(stroke.clone());
        doc.layers[0].strokes.push(Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Erase,
            points: vec![p(250., 180.), p(250., 220.)].into(),
            size: 50.,
            strength: 1.,
            seed: 8,
        });
        let cut = river_batches(&doc.layers[0], 0, &stroke);
        assert!(
            cut.iter()
                .flat_map(|b| &b.shapes)
                .filter_map(Shape::bounds)
                .all(|b| b.max.x < 230. || b.min.x > 270.)
        );
        let later = river_batches(&doc.layers[0], 2, &stroke);
        assert_eq!(
            original.iter().map(|b| &b.shapes).collect::<Vec<_>>(),
            later.iter().map(|b| &b.shapes).collect::<Vec<_>>()
        );
    }
    #[test]
    fn arc_coordinates_follow_reversed_and_vertical_bends_and_stay_in_export_bounds() {
        let doc = Document::blank();
        for size in [1., 110., 500.] {
            let mut stroke = river();
            stroke.size = size;
            stroke.points = vec![
                p(-100., -200.),
                p(-300., -200.),
                p(-300., -20.),
                p(-50., -20.),
            ]
            .into();
            let bounds = stroke.bounds().unwrap();
            let batches = river_batches(&doc.layers[0], 0, &stroke);
            for batch in &batches {
                let reach = &batch.reach;
                assert!((reach.coordinate(reach.course[0]) - reach.distance).abs() < 0.001);
                assert!(
                    (reach.coordinate(*reach.course.last().unwrap())
                        - reach.distance
                        - reach.length)
                        .abs()
                        < 0.001
                );
                for b in batch.shapes.iter().filter_map(Shape::bounds) {
                    assert!(
                        b.min.x >= bounds.min.x
                            && b.max.x <= bounds.max.x
                            && b.min.y >= bounds.min.y
                            && b.max.y <= bounds.max.y
                    );
                }
            }
        }
    }
}
