use super::*;
use crate::document::Document;

fn forest() -> Stroke {
    Stroke {
        creation_id: None,
        material: None,
        brush: Brush::Forest,
        points: vec![p(300., 400.)].into(),
        size: 116.,
        strength: 0.85,
        seed: 42,
    }
}

#[test]
fn source_variants_keep_layered_crowns_and_use_only_parchment_ink() {
    let specimens = vector::specimens(AtlasStyle::Classic);
    for shapes in &specimens[MIXED] {
        assert!(
            shapes.len() > 250,
            "retain the original fine branch and leaf marks"
        );
        assert!(
            shapes
                .iter()
                .filter(|s| matches!(s, Shape::Polygon { .. }))
                .count()
                >= 40
        );
        for shape in shapes {
            let color = match shape {
                Shape::Polygon { fill, .. } => *fill,
                Shape::RegionSurface { .. } | Shape::Sprite { .. } => {
                    panic!("expected engraved vector shapes")
                }
                Shape::Line { color, .. } => *color,
            };
            let (r, g, b) = (color >> 16, (color >> 8) & 255, color & 255);
            assert!(
                r >= g && g >= b,
                "all pigments must be warm monochrome, not green"
            );
        }
    }
    assert!(specimens.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn extending_a_path_preserves_every_existing_tree_and_collision_spacing() {
    let mut doc = Document::blank();
    doc.layers[0].strokes.push(forest());
    let original = trees(&doc.layers[0]);
    assert!(!original.is_empty());
    assert!(
        original.len() <= 11,
        "source default density produces eleven attempts per dab"
    );
    doc.layers[0].strokes[0].points.push(p(700., 400.));
    let expanded = trees(&doc.layers[0]);
    assert!(
        original.iter().all(|tree| expanded.contains(tree)),
        "depth sorting may interleave new trees without moving existing ones"
    );
    assert!(expanded.len() > original.len());
    // Collision acceptance follows seeded insertion order, independently of
    // painter order. The 20% visual reduction retains that original spacing.
    let mut insertion = expanded.clone();
    insertion.sort_by_key(|tree| tree.id);
    for (i, tree) in insertion.iter().enumerate() {
        let clearance =
            ((if tree.variant < 6 { 13. } else { 16. }) * (tree.scale / 0.8) * 0.8).max(9.);
        assert!(
            insertion[..i]
                .iter()
                .all(|old| distance(old.center, tree.center) >= clearance)
        );
    }
    let mut weaker = doc.layers[0].clone();
    weaker.strokes[0].strength = 0.15;
    assert!(trees(&weaker).len() < expanded.len());
}

#[test]
fn erasing_removes_whole_trees_then_repainting_survives_and_round_trips() {
    let mut doc = Document::blank();
    doc.layers[0].strokes.push(forest());
    let initial = trees(&doc.layers[0]);
    let mut erase = forest();
    erase.brush = Brush::Erase;
    erase.size = 250.;
    erase.strength = 1.;
    doc.layers[0].strokes.push(erase);
    assert!(trees(&doc.layers[0]).is_empty());
    doc.layers[0].strokes.push(forest());
    let repainted = trees(&doc.layers[0]);
    assert_eq!(initial.len(), repainted.len());
    let reopened: Document = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert_eq!(trees(&reopened.layers[0]), repainted);
    doc.layers[0].strokes[2].strength = 0.;
    assert!(trees(&doc.layers[0]).is_empty());
}

#[test]
fn trees_paint_outside_former_page_edges_and_with_large_layer_ids() {
    let mut doc = Document::blank();
    doc.layers[0].id = u64::MAX - 1;
    let mut paint = forest();
    paint.points = vec![p(-50_000., 90_000.)].into();
    doc.layers[0].strokes.push(paint);
    let trees = trees(&doc.layers[0]);
    assert!(!trees.is_empty());
    assert!(
        trees
            .iter()
            .all(|tree| distance(tree.center, p(-50_000., 90_000.)) < 58.)
    );
}

#[test]
fn oblique_specimens_keep_a_visible_trunk_and_fit_the_shared_atlas() {
    for variant in 0..FOREST_SPECIMEN_COUNT {
        let mut image = resvg::tiny_skia::Pixmap::new(200, 200).unwrap();
        resvg::render(
            forest_specimen_tree(variant, AtlasStyle::Classic),
            resvg::tiny_skia::Transform::from_scale(2., 2.),
            &mut image.as_mut(),
        );
        let alpha = |x: usize, y: usize| image.data()[(y * 200 + x) * 4 + 3];
        if SCRUB.contains(&variant) {
            assert!(alpha(100, 100) > 128, "scrub anchors at its stems");
            assert!((0..200).all(|x| alpha(x, 54) == 0), "scrub stays low");
        } else {
            assert_eq!(alpha(100, 100), 255, "exposed trunk for variant {variant}");
            assert!(alpha(120, 100) < 100, "canopy must clear the trunk sides");
            assert_eq!(alpha(100, 54), 255, "retain a solid raised crown");
        }
        for edge in 0..200 {
            assert_eq!(alpha(edge, 0), 0, "crown clipped at the top");
            assert_eq!(alpha(edge, 199), 0);
            assert_eq!(alpha(0, edge), 0);
            assert_eq!(alpha(199, edge), 0);
        }
    }
}

#[test]
fn trees_remain_small_and_upright_and_paint_from_back_to_front() {
    let mut layer = Document::blank().layers[0].clone();
    let mut paint = forest();
    paint.points.push(p(300., 250.));
    layer.strokes.push(paint);
    let trees = trees(&layer);
    assert!(trees.len() > 10);
    assert!(
        trees
            .iter()
            .all(|tree| tree.scale <= 0.951 && tree.rotation.abs() <= 3.6)
    );
    assert!(
        trees
            .windows(2)
            .all(|pair| pair[0].center.y <= pair[1].center.y)
    );
    let batches = Placement::new(&layer).batches(
        &[],
        &layer,
        &super::super::field_index::FieldIndex::new(&layer),
    );
    assert_eq!(
        trees,
        batches.into_iter().map(|b| b.tree).collect::<Vec<_>>()
    );
}

#[test]
fn retained_dab_sampler_matches_original_path_iterator_exactly() {
    let mut stroke = crate::document::Stroke {
        creation_id: None,
        material: None,
        brush: Brush::Forest,
        points: Vec::new().into(),
        size: 130.,
        strength: 0.7,
        seed: 9,
    };
    let mut sampler = super::DabSampler::default();
    let mut actual = Vec::new();
    for i in 0..100 {
        stroke.points.push(Point::new(
            -900. + i as f32 * 31.7,
            (i as f32 * 0.26).sin() * 212.,
        ));
        actual.extend(sampler.advance(&stroke).into_iter().map(|(_, q)| q));
        assert_eq!(actual, super::dabs(&stroke).collect::<Vec<_>>());
    }
}

#[test]
fn forest_styles_survive_live_paint_save_reopen_and_svg_export() {
    use crate::scene::{SceneCache, to_svg};

    let mut doc = Document::blank();
    doc.layers[0].strokes.push(forest());
    let layer = doc.layers[0].id;
    let mut cache = SceneCache::default();
    let original = cache.render(&doc, None);
    let mut paint = forest();
    paint.points = vec![p(600., 400.), p(800., 400.)].into();

    let grown = |id| {
        let mut layer = doc.layers[0].clone();
        let mut stroke = paint.clone();
        stroke.material = Some(Arc::from(id));
        layer.strokes.push(stroke);
        trees(&layer).iter().filter(|tree| tree.id.1 == 1).count()
    };
    assert!(grown("desert-scrub") < grown("oak-grove"));

    for (id, variants) in [
        ("fir-grove", FIR),
        ("oak-grove", OAK),
        ("desert-scrub", SCRUB),
    ] {
        paint.material = Some(Arc::from(id));
        let live = cache.render(&doc, Some((layer, &paint)));
        assert!(
            original
                .forest_trees()
                .all(|tree| live.forest_trees().any(|current| tree == current))
        );

        let mut committed = doc.clone();
        committed.layers[0].strokes.push(paint.clone());
        let reopened: Document =
            serde_json::from_str(&serde_json::to_string(&committed).unwrap()).unwrap();
        assert_eq!(reopened.layers[0].strokes[1].material.as_deref(), Some(id));
        assert!(cache.render(&reopened, None) == live);
        let svg = to_svg(&reopened);
        for variant in variants {
            assert!(svg.contains(&format!("href=\"#forest-{variant}\"")));
        }
    }

    for id in ["mixed-woodland", "unknown-forest", "ocean"] {
        let mut with_id = doc.clone();
        with_id.layers[0].strokes[0].material = Some(Arc::from(id));
        assert_eq!(trees(&with_id.layers[0]), trees(&doc.layers[0]));
    }
}
