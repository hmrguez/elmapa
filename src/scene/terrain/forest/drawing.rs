//! Seeded pen-and-wash drawing primitives. No traced or downloaded artwork.
use crate::document::Point as P;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::fmt::Write;
pub(super) const INK: &str = "#75654f";

/// A closed, smoothed, irregular silhouette: small lobes, no regular polygons.
pub(super) fn blob(rng: &mut ChaCha8Rng, x: f32, y: f32, rx: f32, ry: f32, n: usize) -> String {
    let phase = rng.random::<f32>() * 6.;
    let points: Vec<P> = (0..n)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / n as f32;
            let r = 1.
                + (a * 7. + phase).sin() * 0.09
                + (a * 13. - phase).sin() * 0.045
                + rng.random_range(-0.055..0.055);
            P::new(x + a.cos() * rx * r, y + a.sin() * ry * r)
        })
        .collect();
    closed(&points)
}
fn closed(p: &[P]) -> String {
    let mut s = format!("M{:.2} {:.2}", p[0].x, p[0].y);
    let n = p.len();
    for i in 0..n {
        let a = p[(i + n - 1) % n];
        let b = p[i];
        let c = p[(i + 1) % n];
        let d = p[(i + 2) % n];
        write!(
            s,
            "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
            b.x + (c.x - a.x) / 6.,
            b.y + (c.y - a.y) / 6.,
            c.x - (d.x - b.x) / 6.,
            c.y - (d.y - b.y) / 6.,
            c.x,
            c.y
        )
        .unwrap();
    }
    s.push('Z');
    s
}
pub(super) fn leaf_marks(
    s: &mut String,
    rng: &mut ChaCha8Rng,
    center: P,
    radii: P,
    count: usize,
    colors: [&str; 2],
) {
    let (x, y, rx, ry) = (center.x, center.y, radii.x, radii.y);
    let [light, dark] = colors;
    for _ in 0..count {
        let a = rng.random::<f32>() * std::f32::consts::TAU;
        let r = rng.random::<f32>().sqrt() * 0.82;
        let px = x + a.cos() * rx * r;
        let py = y + a.sin() * ry * r;
        let w = rng.random_range(1.3..3.1);
        let h = rng.random_range(1.0..2.2);
        let color = if rng.random::<f32>() < 0.55 {
            light
        } else {
            dark
        };
        write!(s,"<path d=\"M{px:.2} {py:.2}q{:.2} {:.2} {:.2} {:.2}\" fill=\"none\" stroke=\"{color}\" stroke-width=\".65\" stroke-linecap=\"round\" opacity=\".78\"/>",w,-h,w*1.7,h*0.2).unwrap();
    }
}
