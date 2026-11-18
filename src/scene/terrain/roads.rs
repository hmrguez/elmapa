//! Worn, double-edged atlas tracks. Small stable sections also carry the native reveal.
use super::field::erased;
use crate::document::{Layer, Point, Stroke};
use crate::scene::{
    Shape,
    geometry::{distance, p, sampled},
    noise::hash,
    shape::{line, polygon},
};

pub(in crate::scene) fn road_chunks(
    layer: &Layer,
    index: usize,
    stroke: &Stroke,
) -> Vec<Vec<Shape>> {
    if stroke.strength <= 0. || stroke.points.is_empty() {
        return Vec::new();
    }
    let width = (stroke.size * 0.14).clamp(3., 38.);
    let mut samples = sampled(stroke, 8.);
    if samples.len() == 1 {
        let q = samples[0];
        samples = vec![p(q.x - width * 0.45, q.y), p(q.x + width * 0.45, q.y)];
    }
    // Shared cross sections prevent cracks at bends; bounded miters prevent spikes
    // when doubling back. Variation depends on arc-length ordinal, never frame time.
    let sections: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(i, &q)| {
            let before = samples[i.saturating_sub(1)];
            let after = samples[(i + 1).min(samples.len() - 1)];
            let normal = |a: Point, b: Point| {
                let len = distance(a, b).max(0.001);
                p(-(b.y - a.y) / len, (b.x - a.x) / len)
            };
            let a = normal(before, q);
            let b = normal(q, after);
            let n = if i == 0 {
                b
            } else if i + 1 == samples.len() {
                a
            } else {
                let sum = p(a.x + b.x, a.y + b.y);
                let len = sum.x.hypot(sum.y);
                if len < 0.1 {
                    b
                } else {
                    p(sum.x / len, sum.y / len)
                }
            };
            let miter = if i == 0 {
                1.
            } else {
                (n.x * a.x + n.y * a.y).abs().max(0.72).recip()
            };
            let half =
                width * (0.46 + hash(stroke.seed.wrapping_add(i as u64 * 19)) * 0.08) * miter;
            (q, p(n.x * half, n.y * half))
        })
        .collect();
    sections
        .windows(2)
        .enumerate()
        .take_while(|_| !crate::scene::generation::stopped())
        .map(|(i, pair)| {
            let mut out = Vec::new();
            let (a, na) = pair[0];
            let (b, nb) = pair[1];
            let mid = p((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            if erased(a, layer, index) || erased(b, layer, index) || erased(mid, layer, index) {
                return out;
            }
            let at = |q: Point, n: Point, side: f32| p(q.x + n.x * side, q.y + n.y * side);
            let ribbon = |side: f32| {
                vec![
                    at(a, na, -side),
                    at(b, nb, -side),
                    at(b, nb, side),
                    at(a, na, side),
                ]
            };
            let alpha = layer.opacity;
            let ink = alpha * stroke.strength;
            polygon(&mut out, ribbon(1.12), 0xb19a73, ink * 0.13);
            polygon(&mut out, ribbon(1.), 0xd4bc8c, alpha * 0.94);
            polygon(&mut out, ribbon(0.72), 0xebd8ad, alpha * 0.78);
            polygon(&mut out, ribbon(0.27), 0xf2e3c2, alpha * 0.38);
            for side in [-1., 1.] {
                line(
                    &mut out,
                    vec![at(a, na, side), at(b, nb, side)],
                    0x796044,
                    (width * 0.045).clamp(0.45, 0.95),
                    ink * 0.69,
                );
                if i % 5 != 3 {
                    line(
                        &mut out,
                        vec![at(a, na, side * 0.44), at(b, nb, side * 0.44)],
                        0xa68a60,
                        0.45,
                        ink * 0.24,
                    );
                }
            }
            // Broken, oblique bank hachures echo the mountain engravings.
            if i % 3 == 1 {
                let side = if i % 2 == 0 { -1. } else { 1. };
                let start = at(a, na, side * 1.03);
                let end = p(
                    start.x + na.x * side * 0.3 + (b.x - a.x) * 0.23,
                    start.y + na.y * side * 0.3 + (b.y - a.y) * 0.23,
                );
                line(&mut out, vec![start, end], 0x8b7351, 0.55, ink * 0.4);
            }
            if i % 4 == 2 {
                let q = at(
                    mid,
                    p((na.x + nb.x) * 0.5, (na.y + nb.y) * 0.5),
                    hash(stroke.seed.wrapping_add(i as u64)) * 0.5 - 0.25,
                );
                line(
                    &mut out,
                    vec![q, p(q.x + (b.x - a.x) * 0.16, q.y + (b.y - a.y) * 0.16)],
                    0x8b7351,
                    0.65,
                    ink * 0.35,
                );
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Brush, Document};
    fn road(size: f32) -> Stroke {
        Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Path,
            points: vec![p(0., 0.), p(80., 0.), p(95., 35.), p(65., 65.)].into(),
            size,
            strength: 0.85,
            seed: 19,
        }
    }
    #[test]
    fn road_has_a_continuous_bed_shared_banks_and_size_control() {
        let doc = Document::blank();
        let layer = &doc.layers[0];
        let chunks = road_chunks(layer, 0, &road(110.));
        assert!(chunks.len() > 10);
        let bed = |chunk: &Vec<Shape>| match &chunk[1] {
            Shape::Polygon { points, fill, .. } => {
                assert_eq!(*fill, 0xd4bc8c);
                points.clone()
            }
            _ => panic!("road bed missing"),
        };
        for pair in chunks.windows(2) {
            let a = bed(&pair[0]);
            let b = bed(&pair[1]);
            assert_eq!(a[1], b[0]);
            assert_eq!(a[2], b[3]);
        }
        let narrow = road_chunks(layer, 0, &road(20.));
        assert!(
            distance(bed(&chunks[0])[0], bed(&chunks[0])[3])
                > distance(bed(&narrow[0])[0], bed(&narrow[0])[3]) * 3.
        );
        assert_eq!(chunks, road_chunks(layer, 0, &road(110.)));
        assert!(
            road_chunks(
                layer,
                0,
                &Stroke {
                    strength: 0.,
                    ..road(110.)
                }
            )
            .is_empty()
        );
    }
    #[test]
    fn click_short_drag_and_doubling_back_have_finite_geometry() {
        let doc = Document::blank();
        for points in [
            vec![p(0., 0.)],
            vec![p(0., 0.), p(0.5, 0.)],
            vec![p(0., 0.), p(60., 0.), p(0., 0.)],
        ] {
            let stroke = Stroke {
                points: points.into(),
                ..road(110.)
            };
            let chunks = road_chunks(&doc.layers[0], 0, &stroke);
            assert!(!chunks.is_empty(), "every gesture paints");
            assert!(
                chunks
                    .iter()
                    .flatten()
                    .filter_map(Shape::bounds)
                    .all(|b| b.min.x.is_finite()
                        && b.max.y.is_finite()
                        && b.min.y > -30.
                        && b.max.y < 30.)
            );
        }
    }
}
