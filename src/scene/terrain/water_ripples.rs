//! Broken engraving strokes follow the real shore, clipped to their finish.
use super::super::{Shape, generation, shape::line};
use crate::document::Point;
use i_overlay::{core::fill_rule::FillRule, float::clip::FloatClip, string::clip::ClipRule};

pub(super) fn draw(
    out: &mut Vec<Shape>,
    shore: &[Vec<[f64; 2]>],
    finish: &[Vec<[f64; 2]>],
    ink: u32,
) {
    let arcs: Vec<_> = shore
        .iter()
        .enumerate()
        .flat_map(|(i, contour)| arcs_along(contour, i > 0))
        .collect();
    if arcs.is_empty() || generation::stopped() {
        return;
    }
    for path in arcs.clip_by_as::<i64>(
        &finish,
        FillRule::EvenOdd,
        ClipRule {
            invert: false,
            boundary_included: false,
        },
    ) {
        line(
            out,
            path.into_iter()
                .map(|[x, y]| Point::new(x as f32, y as f32))
                .collect(),
            ink,
            1.15,
            0.52,
        );
    }
}

fn arcs_along(contour: &[[f64; 2]], hole: bool) -> Vec<Vec<[f64; 2]>> {
    let mut distances = vec![0.];
    let mut area = 0.;
    for (&[x, y], &[next_x, next_y]) in contour.iter().zip(contour.iter().cycle().skip(1)) {
        let length = (next_x - x).hypot(next_y - y);
        distances.push(distances.last().unwrap() + length);
        area += x * next_y - y * next_x;
    }
    let total = *distances.last().unwrap();
    if total < 32. {
        return Vec::new();
    }
    let point_at = |s: f64| {
        let s = s.rem_euclid(total);
        let i = distances.partition_point(|&d| d <= s) - 1;
        let [x, y] = contour[i];
        let [next_x, next_y] = contour[(i + 1) % contour.len()];
        let t = (s - distances[i]) / (distances[i + 1] - distances[i]);
        [x + t * (next_x - x), y + t * (next_y - y)]
    };
    let inward = if hole { -area.signum() } else { area.signum() };
    // Anchor the broken engraving to world-space crossings. Measuring from
    // the start of a closed loop would shift its pattern on a neighboring tile.
    const SPACING: f64 = 78.;
    const OFFSET: f64 = 19.;
    let mut anchors = Vec::new();
    for (i, (a, b)) in contour
        .iter()
        .zip(contour.iter().cycle().skip(1))
        .enumerate()
    {
        let axis = usize::from((b[1] - a[1]).abs() > (b[0] - a[0]).abs());
        let start = a[axis];
        let end = b[axis];
        let low = start.min(end);
        let high = start.max(end);
        let first = ((low - OFFSET) / SPACING).ceil() as i64;
        let last = ((high - OFFSET) / SPACING).ceil() as i64;
        for index in first..last {
            let crossing = index as f64 * SPACING + OFFSET;
            let t = (crossing - start) / (end - start);
            let along = distances[i] + (distances[i + 1] - distances[i]) * t;
            let across = a[1 - axis] + (b[1 - axis] - a[1 - axis]) * t;
            let variant = (index + (across / SPACING).floor() as i64).rem_euclid(12) as usize;
            anchors.push((along, variant));
        }
    }
    // A compact island may sit between all the world-space marks. Its contour
    // fits inside the shared apron, so retain the small-scale three-mark design.
    if anchors.is_empty() && total < 140. {
        anchors.extend((0..3).map(|i| (total * (i as f64 + 0.3) / 3., i)));
    }
    let mut arcs = Vec::new();
    for (anchor, i) in anchors {
        if !generation::samples(7) {
            break;
        }
        let length = [18_f64, 27., 22., 13.][i % 4].min(total * 0.18);
        let start = anchor - length * 0.5;
        let arc = (0..=6)
            .map(|j| {
                let t = f64::from(j) / 6.;
                let s = start + t * length;
                let [x, y] = point_at(s);
                let [ax, ay] = point_at(s - 3.);
                let [bx, by] = point_at(s + 3.);
                let (dx, dy) = (bx - ax, by - ay);
                let inset = inward * (8. + 2.5 * (i % 3) as f64 + 3. * t * (1. - t));
                let normal = inset / dx.hypot(dy).max(0.01);
                [x - dy * normal, y + dx * normal]
            })
            .collect();
        arcs.push(arc);
    }
    arcs
}
