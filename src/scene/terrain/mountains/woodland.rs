//! Pictorial relief with ledges, branching spurs and a broken ground wash.
use super::{Point, Shape, curved, ground_wash, hash, line, mix, p, polygon};

pub(super) fn peak(center: Point, size: f32, seed: u64, alpha: f32, shade: f32) -> [Vec<Shape>; 2] {
    let h = |n| hash(seed.wrapping_add(n));
    let height = size * (0.66 + h(2) * 0.46);
    let width = size * (0.65 + h(1) * 0.15);
    let point = |x: f32, y: f32| p(center.x + width * x, center.y + height * y);
    let crown = (h(3) - 0.5) * 0.41;
    let top = point(crown, -1.);
    let crest = curved(&[
        top,
        point(crown * 0.68 + 0.08, -0.77),
        point(-0.04, -0.61),
        point(0.12, -0.43),
        point(0.04, -0.21),
        center,
    ]);

    [-1., 1.].map(|side| {
        let tag: u64 = if side < 0. { 20 } else { 110 };
        let rock = |n: u64| h(tag + n);
        let flank = |x: f32, y: f32| point(side * x, y);
        let lit = side < 0.;
        let ledge_y = -0.48 + rock(2) * 0.14;
        let toe_y = -0.02 + rock(3) * 0.10;
        let edge = curved(&[
            top,
            flank(0.21 + rock(4) * 0.12, -0.81),
            flank(0.37, -0.65),
            flank(0.56 + rock(5) * 0.12, ledge_y),
            flank(0.82, ledge_y + 0.12),
            flank(0.87, -0.11),
            flank(1.04, toe_y),
        ]);
        let toe = *edge.last().unwrap();
        let spur = flank(0.52 + rock(6) * 0.22, 0.13);
        let lower = flank(0.24 + rock(7) * 0.12, 0.095);
        let mut shapes = Vec::new();
        ground_wash(
            &mut shapes,
            flank,
            toe_y,
            0x948061,
            alpha * shade * if lit { 0.85 } else { 1. },
        );
        let mut face = edge.to_vec();
        face.extend([spur, lower]);
        face.extend(crest.iter().rev().copied());
        polygon(
            &mut shapes,
            face,
            if lit { 0xdfd2b7 } else { 0xb09a7e },
            alpha * 0.94,
        );
        polygon(
            &mut shapes,
            vec![
                edge[edge.len() - 5],
                toe,
                spur,
                lower,
                flank(0.37, -0.13),
                flank(0.71, -0.19),
            ],
            0xc0ad8d,
            alpha * shade * if lit { 0.25 } else { 0.49 },
        );
        line(
            &mut shapes,
            edge[..edge.len() - 3].to_vec(),
            0x5d4b3b,
            1.16,
            alpha * 0.78,
        );
        line(
            &mut shapes,
            vec![crest[9], flank(0.22, -0.38), flank(0.39, -0.24), spur],
            0x6d573f,
            0.78,
            alpha * shade * 0.66,
        );
        if !lit {
            line(
                &mut shapes,
                crest[..crest.len() - 4].to_vec(),
                0x5d4b3b,
                1.06,
                alpha * 0.79,
            );
        }
        let sample = |path: &[Point], t: f32| {
            let index = t * (path.len() - 1) as f32;
            mix(
                path[index as usize],
                path[(index as usize + 1).min(path.len() - 1)],
                index.fract(),
            )
        };
        for index in 0..31 {
            let t = 0.12 + rock(20 + index) * 0.70;
            let u = 0.16 + rock(65 + index) * 0.71;
            let start = mix(sample(&crest, t), sample(&edge, t), u);
            let end_t = (t + 0.09 + rock(101 + index) * 0.13).min(0.95);
            let end = mix(
                sample(&crest, end_t),
                sample(&edge, end_t),
                (u + 0.11).min(0.97),
            );
            line(
                &mut shapes,
                vec![start, end],
                0x6d573f,
                0.48 + rock(150 + index) * 0.36,
                alpha * shade * (1. - 0.55 * t) * if lit { 0.28 } else { 0.66 },
            );
        }
        for (x, y) in [(0.28, 0.15), (0.61, 0.18), (0.9, 0.1)] {
            line(
                &mut shapes,
                vec![
                    flank(x - 0.04, y + 0.013),
                    flank(x, y - 0.01),
                    flank(x + 0.05, y + 0.006),
                ],
                0x786346,
                0.58,
                alpha * shade * 0.38,
            );
        }
        shapes
    })
}
