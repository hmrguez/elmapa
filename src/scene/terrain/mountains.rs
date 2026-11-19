//! Original pen-and-wash peaks inspired by pictorial relief in early atlases.
use super::super::{
    Shape,
    geometry::{distance, p},
    noise::hash,
    shape::{line, polygon},
};
use super::field::erased;
use crate::{
    brush_materials,
    document::{Brush, Layer, Point, Stroke},
};

pub(in crate::scene) fn mountain_base(left_flank: &[Shape]) -> Option<Point> {
    left_flank.iter().find_map(|shape| match shape {
        Shape::Polygon { points, .. } => points.last().copied(),
        Shape::Sprite { points, .. } => Some(mix(points[2], points[3], 0.5)),
        _ => None,
    })
}

// Fixed arc-length placement has no moving terminal peak: existing mountains
// retain their silhouettes and animation identities as a gesture grows.
fn centers(stroke: &Stroke) -> Vec<Point> {
    let mut result = Vec::new();
    if let Some(first) = stroke.points.first() {
        result.push(*first);
    }
    let spacing = (stroke.size * 0.68).clamp(24., 160.);
    let mut carry = 0.;
    for pair in stroke.points.windows(2) {
        let len = distance(pair[0], pair[1]);
        if len < 0.001 {
            continue;
        }
        let mut d = spacing - carry;
        while d <= len {
            if !super::super::generation::samples(1) {
                return result;
            }
            let t = d / len;
            result.push(mix(pair[0], pair[1], t));
            d += spacing;
        }
        carry = (carry + len) % spacing;
    }
    result
}
fn mix(a: Point, b: Point, t: f32) -> Point {
    p(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

// Open Catmull-Rom pen paths retain the ledges but soften mechanical corners.
fn curved(points: &[Point]) -> Vec<Point> {
    let mut out = vec![points[0]];
    for i in 0..points.len() - 1 {
        let a = points[i.saturating_sub(1)];
        let b = points[i];
        let c = points[i + 1];
        let d = points[(i + 2).min(points.len() - 1)];
        for j in 1..=4 {
            let t = j as f32 / 4.;
            let coordinate = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2. * b)
                    + (-a + c) * t
                    + (2. * a - 5. * b + 4. * c - d) * t * t
                    + (-a + 3. * b - 3. * c + d) * t * t * t)
            };
            out.push(p(
                coordinate(a.x, b.x, c.x, d.x),
                coordinate(a.y, b.y, c.y, d.y),
            ));
        }
    }
    out
}
fn level(path: &[Point], y: f32) -> Point {
    for pair in path.windows(2) {
        if y <= pair[1].y {
            return mix(
                pair[0],
                pair[1],
                ((y - pair[0].y) / (pair[1].y - pair[0].y).max(0.001)).clamp(0., 1.),
            );
        }
    }
    *path.last().expect("nonempty pen path")
}

pub(in crate::scene) fn mountain_chunks(
    layer: &Layer,
    index: usize,
    stroke: &Stroke,
) -> Vec<[Vec<Shape>; 2]> {
    if stroke.strength <= 0. {
        return Vec::new();
    }
    let alpha = layer.opacity;
    let shade = stroke.strength.clamp(0., 1.);
    let sprites = stroke.material.as_deref().and_then(|id| {
        brush_materials::for_brush(Brush::Mountain)
            .find(|(_, material)| material.id == id)
            .and_then(|(index, material)| material.sprites.map(|sheet| (index, sheet)))
    });
    let mut chunks = Vec::new();
    for (i, center) in centers(stroke).into_iter().enumerate() {
        if super::super::generation::stopped() {
            break;
        }
        let seed = stroke.seed.wrapping_add(i as u64 * 83);
        let h = |n| hash(seed.wrapping_add(n));
        let placement = layer.relief_move(crate::document::ReliefKey {
            stroke: index,
            specimen: crate::document::Specimen::Peak(i),
        });
        let (center, after) = placement.map_or((center, index), |item| {
            (item.position, item.after_strokes.saturating_sub(1))
        });
        let removed = erased(center, layer, after);
        if let Some((material, sheet)) = sprites {
            if removed {
                chunks.push([Vec::new(), Vec::new()]);
                continue;
            }
            if !super::super::generation::vertices(4) {
                break;
            }
            let width = stroke.size * (1.29 + h(1) * 0.22);
            let height = (width * sheet.frame_height as f32 / sheet.frame_width as f32)
                .min(stroke.size * 1.25);
            let left = center.x - width * 0.5;
            let right = center.x + width * 0.5;
            let top = center.y - height;
            let frame = (h(4) * sheet.frames as f32) as u32;
            let sprite = Shape::Sprite {
                points: [
                    p(left, top),
                    p(right, top),
                    p(right, center.y),
                    p(left, center.y),
                ],
                material,
                frame: frame.min(sheet.frames - 1),
                opacity: alpha * (0.45 + shade * 0.55),
            };
            // Retain the ordinal pair used by the depth and uplift caches.
            chunks.push([vec![sprite], Vec::new()]);
            continue;
        }
        let w = stroke.size * (1.00 + h(1) * 0.24);
        let height = stroke.size * (0.86 + h(2) * 0.38);
        let top = p(center.x + (h(3) - 0.5) * w * 0.24, center.y - height);
        // A wandering crest, with branching spurs below, replaces flat facets.
        let mut spine = vec![top];
        for j in 1..8 {
            let t = j as f32 / 8.;
            spine.push(p(
                top.x * (1. - t) + center.x * t + (h(10 + j) - 0.5) * w * 0.06,
                top.y + height * t,
            ));
        }
        spine.push(center);
        let spine = curved(&spine);
        let flanks = [-1., 1.].map(|side| {
            let mut out = Vec::new();
            if !removed {
                let tag = if side < 0. { 100 } else { 500 };
                let n = |i: u64| h(tag + i);
                // Weathered shoulders, ledges and a spreading foot. Left and
                // right profiles have independent crags rather than mirroring.
                let mut edge = vec![top];
                let exponent = 0.92 + n(1) * 0.40;
                for j in 1..=11 {
                    let q = j as f32 / 11.;
                    let shoulder = 4 + (n(2) * 3.) as u64;
                    let t = q + if j == shoulder {
                        0.095
                    } else if j == shoulder + 1 {
                        -0.065
                    } else {
                        0.
                    };
                    let spread = 0.55 * q.powf(exponent)
                        + (n(20 + j) - 0.5) * 0.065 * (t * std::f32::consts::PI).sin();
                    edge.push(p(
                        top.x * (1. - t) + center.x * t + side * w * spread,
                        top.y + height * t,
                    ));
                }
                let edge = curved(&edge);
                let surface = |t: f32, u: f32| {
                    let y = top.y + height * t.clamp(0., 1.);
                    mix(level(&spine, y), level(&edge, y), u)
                };
                let mut face = vec![
                    p(center.x + side * w * 0.17, center.y + height * 0.018),
                    p(center.x + side * w * 0.34, center.y + height * 0.009),
                ];
                face.extend(edge.iter().rev().copied());
                face.extend(spine.iter().skip(1).copied());
                polygon(&mut out, face.clone(), 0xe9dfc2, alpha * 0.97);
                polygon(
                    &mut out,
                    face,
                    0xb5a07d,
                    alpha * shade * if side > 0. { 0.30 } else { 0.035 },
                );
                // Narrow overlapping ochre washes follow erosion channels;
                // paper remains visible between them, without a single flat face.
                for rib in 0..5 {
                    let u = 0.12 + rib as f32 * 0.18;
                    let start = 0.12 + n(40 + rib) * 0.22;
                    let mut wash = Vec::new();
                    for j in 0..9 {
                        let t = start + (0.96 - start) * j as f32 / 8.;
                        wash.push(surface(t, u * (0.3 + t * 0.7)));
                    }
                    for j in (0..9).rev() {
                        let t = start + (0.96 - start) * j as f32 / 8.;
                        wash.push(surface(
                            t,
                            (u + 0.12 + n(60 + rib) * 0.06) * (0.3 + t * 0.7),
                        ));
                    }
                    polygon(
                        &mut out,
                        wash,
                        0xa58b65,
                        alpha * shade * if side > 0. { 0.13 } else { 0.07 },
                    );
                }
                line(&mut out, edge.clone(), 0x75654f, 0.85, alpha * 0.75);
                // Branching gullies fan out from the crest into the foothills.
                for rib in 1..6 {
                    let u = rib as f32 / 6.;
                    let start = 0.08 + u * 0.34;
                    let path: Vec<_> = (0..12)
                        .map(|j| {
                            let t = j as f32 / 11.;
                            surface(
                                start + (0.97 - start) * t,
                                u * (0.13 + 0.87 * t) + (t * 8. + rib as f32).sin() * 0.025 * t,
                            )
                        })
                        .collect();
                    line(
                        &mut out,
                        path,
                        0x80694f,
                        0.65,
                        alpha * shade * if side > 0. { 0.47 } else { 0.28 },
                    );
                }
                // Many short, uneven pen strokes follow the local slope. They
                // break before crests to leave thin paper highlights.
                for row in 0..12 {
                    for col in 0..8 {
                        let k = row * 8 + col;
                        let t = 0.10 + row as f32 * 0.066 + n(80 + k) * 0.035;
                        let u = 0.06 + col as f32 * 0.115 + n(190 + k) * 0.035;
                        let length = 0.035 + n(300 + k) * 0.065;
                        let path: Vec<_> = (0..4)
                            .map(|j| {
                                let f = j as f32 / 3.;
                                surface(
                                    t + length * f,
                                    u + f * (0.06 + u * 0.045) + (f * 3.).sin() * 0.009,
                                )
                            })
                            .collect();
                        line(
                            &mut out,
                            path,
                            0x81694f,
                            0.42 + n(410 + k) * 0.18,
                            alpha
                                * shade
                                * (0.25 + n(520 + k) * 0.3)
                                * if side > 0. { 1. } else { 0.40 },
                        );
                    }
                }
                // Small rock strata cross a few gullies; scattered marks and
                // broken contours dissolve into the paper at the foot.
                for j in 0..22 {
                    let t = 0.40 + n(650 + j) * 0.53;
                    let u = 0.10 + n(690 + j) * 0.73;
                    line(
                        &mut out,
                        vec![
                            surface(t, u),
                            surface(t + 0.008, u + 0.035),
                            surface(t - 0.003, u + 0.07),
                        ],
                        0x8e7758,
                        0.45,
                        alpha * shade * 0.38,
                    );
                }
                if side > 0. {
                    line(&mut out, spine.clone(), 0x75654f, 0.75, alpha * 0.58);
                }
                for j in 0..6 {
                    let u = 0.06 + j as f32 * 0.16;
                    line(
                        &mut out,
                        vec![
                            surface(0.98, u),
                            surface(0.973, u + 0.045),
                            surface(0.983, u + 0.10),
                        ],
                        0x9d8867,
                        0.5,
                        alpha * 0.36,
                    );
                }
            }
            out
        });
        // Empty pairs preserve ordinal identities across erasing.
        chunks.push(flanks);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    fn stroke() -> Stroke {
        Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Mountain,
            points: vec![p(100., 100.)].into(),
            size: 100.,
            strength: 0.8,
            seed: 7,
        }
    }
    #[test]
    fn peaks_are_stable_while_extending_and_erase_in_chronological_order() {
        let mut doc = Document::blank();
        let mut s = stroke();
        doc.layers[0].strokes.push(s.clone());
        let click = mountain_chunks(&doc.layers[0], 0, &s);
        assert_eq!(click.len(), 1);
        s.points = vec![p(100., 100.), p(210., 100.)].into();
        doc.layers[0].strokes[0] = s.clone();
        let extended = mountain_chunks(&doc.layers[0], 0, &s);
        assert_eq!(click, extended[..1]);
        let mut eraser = stroke();
        eraser.brush = Brush::Erase;
        eraser.size = 30.;
        eraser.strength = 1.;
        doc.layers[0].strokes.push(eraser.clone());
        let erased = mountain_chunks(&doc.layers[0], 0, &s);
        assert!(erased[0].iter().all(Vec::is_empty));
        assert_eq!(erased[1..], extended[1..]);
        doc.layers[0].strokes = vec![eraser, s.clone()].into();
        assert_eq!(mountain_chunks(&doc.layers[0], 1, &s), extended);
    }
    #[test]
    fn textured_peaks_keep_their_frames_and_ground_anchors_after_extension_and_erasure() {
        let mut doc = Document::blank();
        let mut s = stroke();
        let fallback = mountain_chunks(&doc.layers[0], 0, &s);
        s.material = Some("missing-mountain-finish".into());
        assert_eq!(mountain_chunks(&doc.layers[0], 0, &s), fallback);

        s.material = Some("alpine-crags".into());
        let click = mountain_chunks(&doc.layers[0], 0, &s);
        assert!(matches!(click[0][0].as_slice(), [Shape::Sprite { .. }]));
        assert_eq!(mountain_base(&click[0][0]), Some(s.points[0]));
        s.points = vec![p(100., 100.), p(257., 121.)].into();
        doc.layers[0].strokes.push(s.clone());
        let ridgeline = mountain_chunks(&doc.layers[0], 0, &s);
        assert_eq!(click, ridgeline[..1]);
        let mut eraser = stroke();
        eraser.brush = Brush::Erase;
        eraser.size = 30.;
        eraser.strength = 1.;
        doc.layers[0].strokes.push(eraser);
        let erased = mountain_chunks(&doc.layers[0], 0, &s);
        assert!(erased[0].iter().all(Vec::is_empty));
        assert_eq!(erased[1..], ridgeline[1..]);
    }

    #[test]
    fn large_detailed_peaks_fit_the_saved_content_frame() {
        let doc = Document::blank();
        for size in [12., 110., 360.] {
            for seed in [7, 142, u64::MAX] {
                let mut s = stroke();
                s.size = size;
                s.seed = seed;
                let frame = s.bounds().unwrap();
                for shape in mountain_chunks(&doc.layers[0], 0, &s)
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let b = shape.bounds().unwrap();
                    assert!(
                        b.min.x >= frame.min.x
                            && b.min.y >= frame.min.y
                            && b.max.x <= frame.max.x
                            && b.max.y <= frame.max.y
                    );
                }
            }
        }
    }

    #[test]
    fn pigments_are_parchment_only_and_zero_strength_is_empty() {
        let doc = Document::blank();
        let mut s = stroke();
        for shape in mountain_chunks(&doc.layers[0], 0, &s)
            .into_iter()
            .flatten()
            .flatten()
        {
            let c = match shape {
                Shape::Polygon { fill, .. } => fill,
                Shape::RegionSurface { .. } | Shape::Sprite { .. } => {
                    panic!("expected engraved vector shapes")
                }
                Shape::Line { color, .. } => color,
            };
            assert!((c >> 16) > ((c >> 8) & 255) && ((c >> 8) & 255) > (c & 255));
        }
        s.strength = 0.;
        assert!(mountain_chunks(&doc.layers[0], 0, &s).is_empty());
    }
}
