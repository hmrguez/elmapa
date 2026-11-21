// Project each canopy above a trunk in parchment ink.
use super::drawing::{INK, blob, leaf_marks};
use crate::document::Point as P;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::fmt::Write;
fn broadleaf_layers(seed: u64) -> (String, String) {
    let mut r = ChaCha8Rng::seed_from_u64(9101 + seed * 31);
    let palette = match seed % 4 {
        0 => ["#a49578", "#d1c09e", "#e5d6b5", "#e5d6b5"],
        1 => ["#d1c09e", "#d1c09e", "#e5d6b5", "#f6ecd4"],
        2 => ["#a49578", "#d1c09e", "#e5d6b5", "#f6ecd4"],
        _ => ["#a49578", "#d1c09e", "#e5d6b5", "#f6ecd4"],
    };
    let rx = 25. + r.random_range(-2.0..2.0);
    let ry = 23. + r.random_range(-2.0..3.0);
    let silhouette = blob(&mut r, 0., 0., rx, ry, 52);
    let ground = ground(&silhouette);
    let mut s = crown_base(&silhouette, palette[0]);
    // Branches remain visible between major canopy masses, anchoring the tree visually.
    for i in 0..7 {
        let a = i as f32 * 0.9 + seed as f32 * 0.43;
        let x = a.cos() * 21.;
        let y = a.sin() * 20.;
        write!(s,"<path d=\"M1 2Q{:.2} {:.2} {x:.2} {y:.2}m{:.2} {:.2}l{:.2} {:.2}\" fill=\"none\" stroke=\"#75654f\" stroke-width=\"2.1\" stroke-linecap=\"round\"/>",x*0.45-2.,y*0.4,-x*0.25,-y*0.25,-y*0.23,x*0.23).unwrap();
    }
    // Overlapping, asymmetric leaf masses: larger shady lobes first, sunlit lobes last.
    for i in 0..13 {
        let a = i as f32 * 2.39996 + seed as f32 * 0.27;
        let radius = if i < 8 { 14.5 } else { 6. };
        let x = a.cos() * radius - 1.;
        let y = a.sin() * radius - 1.;
        let w = r.random_range(8.0..12.4);
        let h = r.random_range(7.0..11.2);
        let d = blob(&mut r, x, y, w, h, 28);
        write!(
            s,
            "<path d=\"{d}\" transform=\"translate(1.1 1.7)\" fill=\"#665a48\" opacity=\".32\"/>"
        )
        .unwrap();
        let shade = if x + y > 7. { 1 } else { 2 };
        write!(s,"<path d=\"{d}\" fill=\"{}\" stroke=\"{}\" stroke-width=\".65\"/><path d=\"{d}\" transform=\"translate(-.7 -1)\" fill=\"none\" stroke=\"{}\" stroke-width=\".4\" opacity=\".35\"/>",palette[shade],palette[0],palette[3]).unwrap();
        let highlight = blob(&mut r, x - 2.1, y - 2.5, w * 0.63, h * 0.55, 18);
        write!(
            s,
            "<path d=\"{highlight}\" fill=\"{}\" opacity=\".39\"/>",
            palette[3]
        )
        .unwrap();
        leaf_marks(
            &mut s,
            &mut r,
            P::new(x, y),
            P::new(w, h),
            13,
            [palette[3], palette[0]],
        );
    }
    for i in 0..11 {
        let a = i as f32 * 0.57;
        let x = a.cos() * rx * 0.93;
        let y = a.sin() * ry * 0.94;
        write!(s,"<path d=\"M{x:.2} {y:.2}q-3 -4 -5 -1q2 3 5 1Z\" fill=\"{}\" stroke=\"{}\" stroke-width=\".45\"/>",palette[2],palette[0]).unwrap();
    }
    (ground, s)
}
fn fir_layers(seed: u64) -> (String, String) {
    let mut r = ChaCha8Rng::seed_from_u64(4119 + seed * 97);
    let p = match seed % 3 {
        0 => ["#a49578", "#d1c09e", "#d1c09e", "#e5d6b5"],
        1 => ["#a49578", "#d1c09e", "#e5d6b5", "#f6ecd4"],
        _ => ["#a49578", "#d1c09e", "#e5d6b5", "#f6ecd4"],
    };
    let rx = 23. + r.random_range(-3.0..2.0);
    let ry = 25. + r.random_range(-3.0..2.0);
    let d = blob(&mut r, 0., 0., rx, ry, 56);
    let ground = ground(&d);
    let mut s = crown_base(&d, p[0]);
    for ring in 0..3 {
        let count = if ring == 0 { 8 } else { 6 };
        let radius = 17. - ring as f32 * 5.8;
        for j in 0..count {
            let a = j as f32 * std::f32::consts::TAU / count as f32
                + ring as f32 * 0.63
                + seed as f32 * 0.21
                + r.random_range(-0.13..0.13);
            let x = a.cos() * radius;
            let y = a.sin() * radius;
            let w = r.random_range(7.2..10.);
            let h = r.random_range(7.5..11.);
            let d = blob(&mut r, x, y, w, h, 24);
            write!(
                s,
                "<path d=\"{d}\" transform=\"translate(.8 1.5)\" fill=\"#665a48\" opacity=\".36\"/>"
            )
            .unwrap();
            let shade = if x + y > 2. { 1 } else { 2 };
            write!(s,"<path d=\"{d}\" fill=\"{}\" stroke=\"{}\" stroke-width=\".65\"/><path d=\"M{:.2} {:.2}Q{:.2} {:.2} {x:.2} {y:.2}\" fill=\"none\" stroke=\"{}\" stroke-width=\".8\"/>",p[shade],p[0],x*0.45,y*0.45,x-2.,y+2.,p[0]).unwrap();
            for k in 0..9 {
                let q = k as f32 / 9.;
                let bx = x + (q - 0.5) * w * 1.6;
                let by = y + r.random_range(-h * 0.65..h * 0.65);
                write!(s,"<path d=\"M{bx:.2} {by:.2}l-2.8 -2.5m2.8 2.5l1.7 -3.2m-1.7 3.2l-3.4 .5\" fill=\"none\" stroke=\"{}\" stroke-width=\".55\" opacity=\".75\"/>",if k%3==0{p[3]}else{p[0]}).unwrap();
            }
        }
    }
    s.push_str(
        "<path d=\"M-2 1L0 -4L3 0L1 3Z\" fill=\"#f6ecd4\" stroke=\"#75654f\" stroke-width=\".6\"/>",
    );
    (ground, s)
}

// A flattened cast wash and root flare anchor the raised crown to the map.
fn ground(d: &str) -> String {
    format!(
        "<path d=\"{d}\" transform=\"translate(6 2) scale(1 .27)\" fill=\"#665a48\" opacity=\".12\"/><ellipse cx=\"1\" cy=\"2\" rx=\"7\" ry=\"2.4\" fill=\"#665a48\" opacity=\".22\"/><path d=\"M-4 3Q-1 -2 -2 -15L3 -15Q2 -2 5 3L1 1Z\" fill=\"#a49578\" stroke=\"{INK}\" stroke-width=\".8\" stroke-linejoin=\"round\"/><path d=\"M1 -12L1 0\" fill=\"none\" stroke=\"#75654f\" stroke-width=\"1.1\"/>"
    )
}
fn crown_base(d: &str, color: &str) -> String {
    format!(
        "<path d=\"{d}\" transform=\"translate(1.2 1.8)\" fill=\"#665a48\" stroke=\"{INK}\" stroke-width=\".7\"/><path d=\"{d}\" fill=\"{color}\" stroke=\"{INK}\" stroke-width=\"1.05\"/>"
    )
}
pub(super) fn specimen(variant: usize) -> String {
    const GROVES: [&str; super::OAK.end - super::MIXED.end] = [
        include_str!("../../../../assets/brush-materials/forest/fir-1.svg"),
        include_str!("../../../../assets/brush-materials/forest/fir-2.svg"),
        include_str!("../../../../assets/brush-materials/forest/fir-3.svg"),
        include_str!("../../../../assets/brush-materials/forest/oak-1.svg"),
        include_str!("../../../../assets/brush-materials/forest/oak-2.svg"),
        include_str!("../../../../assets/brush-materials/forest/oak-3.svg"),
    ];
    if super::SCRUB.contains(&variant) {
        return scrub(variant).into();
    }
    if variant >= super::MIXED.end {
        return GROVES[variant - super::MIXED.end].into();
    }
    let (ground, crown) = if variant < 6 {
        fir_layers(variant as u64)
    } else {
        broadleaf_layers((variant - 6) as u64)
    };
    // Foreshorten the crown and lift it above the root pivot. Keep the same
    // projection in the shared artwork so sprites, thumbnails and SVG agree.
    format!("{ground}<g transform=\"translate(0 -23) scale(1 .68)\">{crown}</g>")
}

pub(super) fn scrub(variant: usize) -> &'static str {
    const SCRUB: [&str; super::SCRUB.end - super::SCRUB.start] = [
        include_str!("../../../../assets/brush-materials/forest/scrub-1.svg"),
        include_str!("../../../../assets/brush-materials/forest/scrub-2.svg"),
        include_str!("../../../../assets/brush-materials/forest/scrub-3.svg"),
    ];
    SCRUB[variant - super::SCRUB.start]
}
