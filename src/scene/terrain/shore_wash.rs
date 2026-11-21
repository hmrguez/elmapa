//! Resolve a water-side wash on the same world-aligned tiles as the coast.
use super::materials::{Contours, Masks, points};
use crate::{
    brush_materials::{self, Material},
    document::{AtlasStyle, ContentBounds, Point},
    scene::{Shape, generation, geometry::segment_distance, noise::noise},
};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
    mesh::float::{
        outline::offset::OutlineOffset,
        style::{LineJoin, OutlineStyle},
    },
};
use std::sync::Arc;

// Split the native mesh across the ramp to keep interpolation local. These
// same strips give SVG and shelf previews a portable approximation.
const WIDTH: f32 = 28.;
const DISTANCES: [f32; 7] = [0., 3., 6., 10., 15., 21., WIDTH];

#[derive(Clone, Debug, PartialEq)]
pub struct ShoreWash {
    pub contours: Vec<Vec<Point>>,
    pub opacity: f32,
    segments: Arc<[[Point; 2]]>,
    pigment: Pigment,
    distance: [f32; 2],
}

impl ShoreWash {
    pub(in crate::scene) fn segment_bytes(&self) -> usize {
        std::mem::size_of_val(self.segments.as_ref())
    }

    pub fn vertex_color(&self, point: Point) -> [f32; 4] {
        let distance = self
            .segments
            .iter()
            .map(|&[a, b]| segment_distance(point, a, b))
            .fold(f32::INFINITY, f32::min);
        let variation = noise(point, 9416);
        let (rgb, alpha) = self.pigment.at(distance, variation);
        [rgb[0], rgb[1], rgb[2], self.opacity * alpha]
    }

    pub(in crate::scene) fn svg_color(&self) -> (u32, f32) {
        let (rgb, alpha) = self
            .pigment
            .at((self.distance[0] + self.distance[1]) * 0.5, 0.);
        let [r, g, b] = rgb.map(|channel| (channel * 255.).round() as u32);
        ((r << 16) | (g << 8) | b, self.opacity * alpha)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Pigment {
    rim: [f32; 3],
    shelf: [f32; 3],
    width: f32,
    strength: f32,
}

fn rgb(hex: u32) -> [f32; 3] {
    [16, 8, 0].map(|shift| ((hex >> shift) & 255) as f32 / 255.)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

impl Pigment {
    fn for_style(style: AtlasStyle) -> Self {
        let (rim, shelf, strength) = match style {
            AtlasStyle::Classic => (0xf9eac3, 0x94a890, 0.58),
            AtlasStyle::Woodland => (0xf1e7c6, 0x748f83, 0.56),
        };
        Self {
            rim: rgb(rim),
            shelf: rgb(shelf),
            width: WIDTH,
            strength,
        }
    }

    fn for_finish(&self, material: &Material) -> Self {
        let water = rgb(material.color);
        let (rim, width) = match material.id.as_str() {
            "oasis-water" => (rgb(0xf6d9ad), 16.),
            "glacial-water" => (rgb(0xe8f3e5), self.width),
            _ => (self.rim, self.width),
        };
        Self {
            rim: mix(water, rim, 0.72),
            shelf: mix(water, material.shoreline_ink.map_or(self.shelf, rgb), 0.38),
            width,
            strength: 0.49,
        }
    }

    fn at(&self, distance: f32, variation: f32) -> ([f32; 3], f32) {
        let depth = (distance / self.width).min(1.);
        let depth = depth + 0.12 * variation * depth * (1. - depth);
        let pigment = mix(self.rim, self.shelf, smooth((depth - 0.08) / 0.48));
        let fade = 1. - smooth((depth + 0.1) / 1.1);
        (pigment, self.strength * fade * (1. + 0.09 * variation))
    }
}

fn overlay(subject: &Contours, clip: &Contours, rule: OverlayRule) -> Contours {
    subject
        .overlay_as::<i64>(clip, rule, FillRule::EvenOdd)
        .into_iter()
        .flatten()
        .collect()
}

pub(super) fn build(
    coast: &Contours,
    frame: &Contours,
    bounds: ContentBounds,
    water_finishes: &Masks,
    style: AtlasStyle,
) -> Vec<Shape> {
    let pigment = Pigment::for_style(style);
    let apron = bounds.padded(pigment.width);
    let segments: Arc<[[Point; 2]]> = coast
        .iter()
        .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
        .map(|(a, b)| [*a, *b].map(|[x, y]| Point::new(x as f32, y as f32)))
        .filter(|[a, b]| {
            a.x.min(b.x) <= apron.max.x
                && a.x.max(b.x) >= apron.min.x
                && a.y.min(b.y) <= apron.max.y
                && a.y.max(b.y) >= apron.min.y
        })
        .collect();
    if segments.is_empty() {
        return Vec::new();
    }
    let mut washes = Vec::new();
    let mut inside = overlay(coast, frame, OverlayRule::Intersect);
    for pair in DISTANCES.windows(2) {
        if generation::stopped() {
            break;
        }
        let distance = [pair[0], pair[1]];
        let outline = OutlineStyle::new(f64::from(distance[1]))
            .line_join(LineJoin::Round(std::f64::consts::PI / 12.));
        let expanded = coast
            .outline_as::<i64>(&outline)
            .into_iter()
            .flatten()
            .collect();
        let outside = overlay(&expanded, frame, OverlayRule::Intersect);
        let mut strip = overlay(&outside, &inside, OverlayRule::Difference);
        inside = outside;
        let mut push = |contours: Contours, pigment: Pigment| {
            let distance = [distance[0], distance[1].min(pigment.width)];
            if distance[0] < distance[1]
                && !contours.is_empty()
                && generation::vertices(contours.iter().map(Vec::len).sum())
            {
                washes.push(Shape::ShoreWash(Arc::new(ShoreWash {
                    contours: contours.iter().map(|ring| points(ring)).collect(),
                    opacity: 1.,
                    segments: segments.clone(),
                    pigment,
                    distance,
                })));
            }
        };
        for (id, mask) in water_finishes.iter().take_while(|_| !generation::stopped()) {
            let finished = overlay(&strip, mask, OverlayRule::Intersect);
            if !finished.is_empty() {
                strip = overlay(&strip, &finished, OverlayRule::Difference);
                push(
                    finished,
                    pigment.for_finish(&brush_materials::artwork_catalog()[*id]),
                );
            }
        }
        push(strip, pigment.clone());
    }
    washes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{geometry::contains, terrain::regions::rectangle};

    fn bounds(left: f32, top: f32, right: f32, bottom: f32) -> ContentBounds {
        ContentBounds {
            min: Point::new(left, top),
            max: Point::new(right, bottom),
        }
    }

    fn at(washes: &[Shape], point: Point) -> Vec<[f32; 4]> {
        washes
            .iter()
            .filter_map(|shape| {
                let Shape::ShoreWash(wash) = shape else {
                    unreachable!()
                };
                let inside = wash
                    .contours
                    .iter()
                    .filter(|ring| contains(ring, point))
                    .count()
                    % 2
                    == 1;
                inside.then(|| wash.vertex_color(point))
            })
            .collect()
    }

    #[test]
    fn opposing_shores_share_one_ramp_across_tiles() {
        for style in [AtlasStyle::Classic, AtlasStyle::Woodland] {
            let make = |left, right| {
                let coast = [
                    bounds(left - 60., -90., right + 60., 0.),
                    bounds(left - 60., 22., right + 60., 90.),
                ]
                .into_iter()
                .flat_map(rectangle)
                .collect();
                let tile = bounds(left, -12., right, 35.);
                build(&coast, &rectangle(tile), tile, &vec![], style)
            };
            let left = make(-80., 0.);
            let right = make(0., 80.);
            for y in [1., 4., 7., 11., 14., 17., 20.] {
                let a = at(&left, Point::new(-0.01, y));
                let b = at(&right, Point::new(0.01, y));
                assert_eq!(a.len(), 1, "wash must be composited once at depth {y}");
                assert_eq!(b.len(), 1);
                for (x, y) in a[0].into_iter().zip(b[0]) {
                    assert!((x - y).abs() < 0.002);
                }
            }
            assert!(at(&left, Point::new(-10., -1.)).is_empty());
            assert!(at(&right, Point::new(10., 23.)).is_empty());
            let alpha = |y| at(&left, Point::new(-1., y))[0][3];
            assert!(alpha(1.) > alpha(4.) && alpha(4.) > alpha(11.));
            assert!(alpha(20.) > alpha(11.));
        }
    }

    #[test]
    fn local_water_finish_changes_rim_and_fades_at_its_width() {
        let style = AtlasStyle::Woodland;
        let tile = bounds(-30., -20., 30., 50.);
        let coast = rectangle(bounds(-90., -80., 90., 0.));
        let oasis = brush_materials::artwork_catalog()
            .iter()
            .position(|material| material.id == "oasis-water" && material.color == 0x91ab8e)
            .unwrap();
        let masks = vec![(oasis, rectangle(bounds(-30., 0., 0., 50.)))];
        let washes = build(&coast, &rectangle(tile), tile, &masks, style);
        assert_ne!(
            at(&washes, Point::new(-5., 2.)),
            at(&washes, Point::new(5., 2.))
        );
        assert_eq!(at(&washes, Point::new(-5., 19.))[0][3], 0.);
        assert!(at(&washes, Point::new(5., 19.))[0][3] > 0.);
        assert!(at(&washes, Point::new(5., WIDTH + 1.)).is_empty());
    }
}
