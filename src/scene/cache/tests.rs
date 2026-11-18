//! Shared fixtures for cached scene regression tests.
mod composition;
mod live_material;
mod retention;
mod terrain_finishes;
mod terrain_tiles;

use super::Scene;
use crate::{
    document::{Brush, Document, Point, Stroke},
    scene::build_scene,
};

fn stroke(brush: Brush) -> Stroke {
    Stroke {
        creation_id: None,
        material: None,
        brush,
        points: vec![Point::new(520.0, 390.0)].into(),
        size: 130.0,
        strength: 0.85,
        seed: 421,
    }
}

fn assert_material(scene: &Scene, doc: &Document) {
    let actual: Vec<_> = scene.shapes().cloned().collect();
    let expected = build_scene(doc);
    assert_eq!(
        actual.len(),
        expected.len(),
        "cached shape count differs from export geometry"
    );
    for (mut a, mut b) in actual.into_iter().zip(expected) {
        let alpha_a = std::mem::replace(a.opacity_mut(), 1.);
        let alpha_b = std::mem::replace(b.opacity_mut(), 1.);
        assert!(a == b, "cached geometry must match a full rebuild exactly");
        assert!(
            (alpha_a - alpha_b).abs() < 0.000001,
            "per-shape opacity changed"
        );
    }
}
