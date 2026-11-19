use super::{assert_material, stroke};
use crate::document::{Brush, Document, Point};
use crate::scene::{SceneCache, Shape};
use std::sync::Arc;

#[test]
fn appending_forests_only_resamples_new_input_segments() {
    let doc = Document::demo();
    let mut cache = SceneCache::default();
    cache.render(&doc, None);
    let mut paint = stroke(Brush::Forest);
    paint.points = vec![Point::new(-900., -600.)].into();
    for i in 0..80 {
        paint
            .points
            .push_point(Point::new(-900. + i as f32 * 16., -600.));
        cache.render(&doc, Some((doc.layers[1].id, &paint)));
        assert_eq!(cache.work().resampled_segments, 1);
        assert_eq!(cache.work().field_samples, 0);
    }
}

#[test]
fn cancelled_worker_retains_last_ready_live_trees_for_unrendered_release() {
    use crate::scene::generation::{Limits, bounded};
    let mut doc = Document::demo();
    let id = doc.layers[1].id;
    let mut cache = SceneCache::default();
    cache.render(&doc, None);
    let mut paint = stroke(Brush::Forest);
    paint.points = vec![Point::new(-500., -500.)].into();
    let ready = cache.render(&doc, Some((id, &paint)));
    paint.points.push(Point::new(-490., -500.));
    cache.begin_generation();
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
    let result = bounded(Limits::INTERACTIVE, Some((cancelled, 0)), || {
        cache.render(&doc, Some((id, &paint)))
    });
    assert!(result.is_err());
    cache.finish_generation(false);
    doc.layers[1].strokes.push(paint);
    let released = cache.render(&doc, None);
    assert_material(&released, &doc);
    for batch in ready
        .batches()
        .filter(|batch| ready.forest_tree(batch).is_some())
    {
        assert!(released.batches().any(|new| Arc::ptr_eq(batch, new)));
    }
}

#[test]
fn local_erasing_retains_untouched_mountain_and_trail_sample_chunks() {
    for brush in [Brush::Mountain, Brush::Path] {
        let mut doc = Document::blank();
        let mut base = stroke(brush);
        base.points = (0..500)
            .map(|i| Point::new(i as f32 * 20., (i as f32 * 0.1).sin() * 30.))
            .collect();
        doc.layers[0].strokes.push(base);
        let mut cache = SceneCache::default();
        let original = cache.render(&doc, None);
        let mut erase = stroke(Brush::Erase);
        erase.size = 60.;
        erase.points = vec![Point::new(120., 0.)].into();
        let live = cache.render(&doc, Some((1, &erase)));
        let reused = original
            .batches()
            .filter(|batch| live.batches().any(|next| Arc::ptr_eq(batch, next)))
            .count();
        let affected = original
            .batches()
            .filter(|batch| {
                batch
                    .iter()
                    .filter_map(Shape::bounds)
                    .any(|bounds| bounds.intersects(erase.bounds().unwrap()))
            })
            .count();
        assert!(
            reused + affected >= original.batches().count(),
            "remote sample chunks were invalidated"
        );
        erase.points.push_point(Point::new(120., 500.));
        let prior = cache.render(&doc, Some((1, &erase)));
        erase.points.push_point(Point::new(2000., 500.));
        let remote = cache.render(&doc, Some((1, &erase)));
        assert_eq!(
            cache.work().line_strokes,
            0,
            "eraser replayed already processed stroke intersections"
        );
        assert!(
            prior
                .batches()
                .zip(remote.batches())
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
        doc.layers[0].strokes.push(erase);
        assert_material(&remote, &doc);
    }
}

#[test]
fn nonforest_worker_snapshots_share_settled_forest_placement_and_instances() {
    let document = Document::demo();
    let layer = document
        .layers
        .iter()
        .find(|layer| layer.strokes.iter().any(|s| s.brush == Brush::Forest))
        .unwrap()
        .id;
    let mut cache = SceneCache::default();
    cache.render(&document, None);
    let base = cache.layers.iter().find(|m| m.source.id == layer).unwrap();
    let placement = base.placement.clone();
    let forest = base.forest.clone();
    let mut paint = stroke(Brush::Land);
    cache.render(&document, Some((layer, &paint)));
    cache.begin_generation();
    paint.points.push_point(Point::new(560., 400.));
    cache.render(&document, Some((layer, &paint)));
    let live = &cache.live.as_ref().unwrap().material;
    assert!(
        Arc::ptr_eq(&live.placement, &placement),
        "unrelated brush copied all forest collision cells"
    );
    assert!(
        Arc::ptr_eq(&live.forest, &forest),
        "unrelated brush copied all forest instances"
    );
    cache.finish_generation(false);
    assert!(Arc::ptr_eq(
        &cache.live.as_ref().unwrap().material.placement,
        &placement
    ));
}

#[test]
fn linear_edits_keep_terrain_and_respect_erasers_and_visibility() {
    let mut doc = Document::blank();
    let lay = &mut doc.layers[0];
    let mut land = stroke(Brush::Land);
    land.size = 420.;
    land.points = vec![Point::new(145., 0.)].into();
    lay.strokes.push(land);
    for brush in [Brush::River, Brush::Path] {
        let mut course = stroke(brush);
        course.size = 55.;
        course.points = vec![Point::new(0., 0.), Point::new(280., 0.)].into();
        lay.strokes.push(course);
    }
    let mut erase = stroke(Brush::Erase);
    erase.size = 50.;
    erase.points = vec![Point::new(170., 0.)].into();
    lay.strokes.push(erase);
    let mut cache = SceneCache::default();
    let original = cache.render(&doc, None);
    let pick = |scene: &crate::scene::Scene, x, y| {
        scene
            .linear_stroke_at(Point::new(x, y), 6.)
            .map(|id| id.index)
    };
    assert_eq!(pick(&original, 76., 0.), Some(2));
    assert_eq!(pick(&original, 170., 0.), None);
    doc.layers[0].strokes[2].points = vec![
        Point::new(0., 0.),
        Point::new(76., -66.),
        Point::new(170., 0.),
        Point::new(280., 0.),
    ]
    .into();
    let edited = cache.render(&doc, None);
    assert_eq!(cache.work().field_samples, 0);
    assert_eq!(cache.work().line_strokes, 1);
    assert_eq!(pick(&edited, 76., 0.), Some(1));
    assert_eq!(pick(&edited, 76., -66.), Some(2));
    assert_eq!(pick(&edited, 170., 0.), None);
    assert_material(&edited, &doc);
    doc.layers[0].visible = false;
    assert_eq!(pick(&cache.render(&doc, None), 76., -66.), None);
}
