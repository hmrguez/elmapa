use super::{Shape, geometry::p, noise::hash, shape::line};
use crate::document::ContentBounds;

pub(super) const CELL: f32 = 64.;

pub(super) fn fibre_seed(x: i64, y: i64) -> u64 {
    (x as u64).wrapping_mul(3719) ^ (y as u64).wrapping_mul(7919)
}

/// World-anchored fibres continue across the workspace without a page seam.
/// At overview scales the subpixel detail is omitted rather than sampling vast
/// empty regions. Terrain geometry is never affected by this detail budget.
pub fn paper_detail(bounds: ContentBounds) -> Vec<Shape> {
    let cell = CELL;
    let minx = (bounds.min.x / cell).floor() as i64;
    let maxx = (bounds.max.x / cell).ceil() as i64;
    let miny = (bounds.min.y / cell).floor() as i64;
    let maxy = (bounds.max.y / cell).ceil() as i64;
    if (maxx - minx).saturating_mul(maxy - miny) > 4096 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for y in miny..maxy {
        for x in minx..maxx {
            let seed = fibre_seed(x, y);
            for i in 0..3_u64 {
                let seed = seed.wrapping_add(i * 7);
                let x = x as f32 * cell + hash(seed) * cell;
                let y = y as f32 * cell + hash(seed.wrapping_add(1)) * cell;
                let length = 0.7 + hash(seed.wrapping_add(2)) * 2.6;
                line(
                    &mut out,
                    vec![p(x, y), p(x + length, y + length * 0.18)],
                    0x9d997e,
                    0.55,
                    0.08 + hash(seed.wrapping_add(3)) * 0.08,
                );
            }
        }
    }
    out
}
