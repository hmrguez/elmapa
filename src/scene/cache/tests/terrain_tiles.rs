use super::{assert_material, stroke};
use crate::document::{Brush, Document, Point};
use crate::scene::{SceneCache, Shape};
use std::sync::Arc;

#[test]
fn fast_land_growth_retains_untouched_textured_shore_batches() {
    let mut doc = Document::blank();
    let mut land = stroke(Brush::Land);
    land.material = Some("ochre-sands".into());
    land.points = vec![Point::new(-2600., -35.), Point::new(450., -35.)].into();
    land.size = 290.;
    doc.layers[0].strokes.push(land);
    let mut cache = SceneCache::default();
    let before = cache.render(&doc, None);
    let untouched: Vec<_> = before
        .batches()
        .filter(|batch| {
            batch
                .iter()
                .filter_map(Shape::bounds)
                .all(|bounds| bounds.max.x < 100.)
        })
        .cloned()
        .collect();
    assert!(
        untouched.len() >= 4,
        "the long textured shore must have retained tiles"
    );

    let mut live = stroke(Brush::Land);
    live.points = vec![Point::new(450., -35.)].into();
    for i in 1..=16 {
        live.points.push(Point::new(
            450. + i as f32 * 31.,
            -35. + (i as f32).sin() * 44.,
        ));
        let scene = cache.render(&doc, Some((1, &live)));
        for batch in &untouched {
            assert!(
                scene.batches().any(|next| Arc::ptr_eq(batch, next)),
                "an untouched tile would be reuploaded"
            );
        }
    }

    doc.layers[0].strokes.push(live);
    let released = cache.render(&doc, None);
    assert_material(&released, &doc);
}

#[test]
fn textured_fill_and_shoreline_cross_a_tile_boundary() {
    use crate::{
        brush_materials,
        scene::{geometry::contains, terrain::regions::SHORE_INK},
    };

    let mut doc = Document::blank();
    doc.style = crate::document::AtlasStyle::Classic;
    let mut land = stroke(Brush::Land);
    land.size = 300.;
    land.material = Some("ochre-sands".into());
    land.points = vec![Point::new(-460., -95.), Point::new(540., 120.)].into();
    doc.layers[0].strokes.push(land);
    let scene = SceneCache::default().render(&doc, None);

    let fills_at = |point| {
        scene
            .shapes()
            .filter_map(|shape| {
                let (rings, fill, material) = match shape {
                    Shape::RegionSurface {
                        contours,
                        fill,
                        material,
                        ..
                    } => (contours.as_slice(), *fill, *material),
                    Shape::Polygon { points, fill, .. } => {
                        (std::slice::from_ref(points), *fill, None)
                    }
                    Shape::ShoreWash(wash) => (wash.contours.as_slice(), wash.svg_color().0, None),
                    _ => return None,
                };
                (rings.iter().filter(|ring| contains(ring, point)).count() % 2 == 1).then_some((
                    fill,
                    material.map(|id| brush_materials::artwork_catalog()[id].id.as_str()),
                ))
            })
            .collect::<Vec<_>>()
    };
    // x=192 is a tile edge. Check the opaque finish, the painted shelf and the
    // thin shoreline within a fraction of a world unit on either side.
    let crest = (-1400..-400)
        .map(|y| y as f32 / 10.)
        .find(|&y| {
            fills_at(Point::new(191.98, y))
                .iter()
                .any(|&(fill, _)| fill == SHORE_INK)
        })
        .expect("shoreline should cross the seam");
    assert_eq!(
        fills_at(Point::new(191.98, 30.)),
        [(crate::scene::PARCHMENT_COLOR, Some("ochre-sands"))]
    );
    assert_eq!(
        fills_at(Point::new(192.02, 30.)),
        [(crate::scene::PARCHMENT_COLOR, Some("ochre-sands"))]
    );
    for y in [crest + 0.3, crest - 4., crest - 12.] {
        assert_eq!(
            fills_at(Point::new(191.98, y)),
            fills_at(Point::new(192.02, y))
        );
    }
}

#[test]
fn local_region_update_preserves_distant_islands_and_samples_local_cells() {
    let mut doc = Document::blank();
    for x in [0., 9000., -9000.] {
        let mut paint = stroke(Brush::Land);
        paint.points = vec![Point::new(x, -600.)].into();
        paint.size = 700.;
        doc.layers[0].strokes.push(paint);
    }
    let mut cache = SceneCache::default();
    cache.render(&doc, None);
    let cold = cache.work();
    let before: Vec<_> = cache.layers[0]
        .terrain
        .regions(0)
        .filter(|(coord, _)| coord.bounds().min.x > 8000.)
        .flat_map(|(_, tile)| tile.batches.iter().cloned())
        .collect();
    assert!(!before.is_empty());
    let mut paint = stroke(Brush::Erase);
    paint.size = 60.;
    paint.points = vec![Point::new(0., -600.)].into();
    let scene = cache.render(&doc, Some((1, &paint)));
    assert!(cache.work().field_samples < cold.field_samples / 4);
    assert!(cache.work().contour_cells < cold.contour_cells / 2);
    for batch in before {
        assert!(scene.batches().any(|b| Arc::ptr_eq(b, &batch)));
    }
    doc.layers[0].strokes.push(paint);
    assert_material(&scene, &doc);
}

#[test]
fn metadata_visibility_and_history_reuse_geometry() {
    let mut editor = crate::editor::Editor::demo();
    let mut cache = SceneCache::default();
    let original = cache.render(editor.document(), None);
    let id = editor.active_layer();
    editor.rename_layer(id, "Same terrain".into());
    let renamed = cache.render(editor.document(), None);
    assert!(
        original
            .batches()
            .zip(renamed.batches())
            .all(|(a, b)| Arc::ptr_eq(a, b))
    );
    assert_eq!(cache.work(), crate::scene::work::WorkCounts::default());
    editor.toggle_layer(id);
    cache.render(editor.document(), None);
    editor.toggle_layer(id);
    let shown = cache.render(editor.document(), None);
    assert!(
        original
            .batches()
            .zip(shown.batches())
            .all(|(a, b)| Arc::ptr_eq(a, b))
    );
    editor.commit_stroke(stroke(Brush::Land));
    let edited = cache.render(editor.document(), None);
    editor.undo();
    let undone = cache.render(editor.document(), None);
    assert!(
        original
            .batches()
            .zip(undone.batches())
            .all(|(a, b)| Arc::ptr_eq(a, b))
    );
    editor.redo();
    let redone = cache.render(editor.document(), None);
    assert!(
        edited
            .batches()
            .zip(redone.batches())
            .all(|(a, b)| Arc::ptr_eq(a, b))
    );
}

#[test]
fn unrelated_line_strokes_survive_local_erasing() {
    let mut doc = Document::blank();
    for brush in [Brush::Mountain, Brush::River, Brush::Path] {
        let mut paint = stroke(brush);
        paint.points = vec![Point::new(9000., -600.), Point::new(9200., -500.)].into();
        doc.layers[0].strokes.push(paint);
    }
    let mut cache = SceneCache::default();
    let before = cache.render(&doc, None);
    let mut erase = stroke(Brush::Erase);
    erase.points = vec![Point::new(0., 0.)].into();
    let live = cache.render(&doc, Some((1, &erase)));
    assert_eq!(cache.work().line_strokes, 0);
    for batch in before.batches() {
        assert!(live.batches().any(|b| Arc::ptr_eq(b, batch)));
    }
    doc.layers[0].strokes.push(erase);
    assert_material(&live, &doc);
}

fn wide_terrain(half_width: f32) -> Document {
    let mut doc = Document::blank();
    for (y, brush) in [(-4000., Brush::Land), (0., Brush::Water)] {
        let mut base = stroke(brush);
        base.points = vec![Point::new(-half_width, y), Point::new(half_width, y)].into();
        base.size = 1600.;
        doc.layers[0].strokes.push(base);
    }
    doc
}

#[test]
fn hundred_thousand_unit_shores_fit_the_interactive_budget() {
    use crate::scene::generation::{Limits, bounded};

    let doc = wide_terrain(50_000.);
    let scene = bounded(Limits::INTERACTIVE, None, || {
        SceneCache::default().render(&doc, None)
    })
    .expect("a wide sparse coast should fit the interactive budget");
    assert!(scene.batches().count() > 250);
}

#[test]
fn wide_tiled_regions_keep_exact_contours_through_local_paint_and_erase_history() {
    let mut doc = wide_terrain(6_000.);
    let mut cache = SceneCache::default();
    cache.render(&doc, None);
    for index in 0..9 {
        let mut paint = stroke([Brush::Land, Brush::Erase, Brush::Water][index % 3]);
        paint.size = 100. + index as f32 * 37.;
        paint.points = vec![Point::new(
            index as f32 * 270. - 900.,
            if index % 3 == 2 { 0. } else { -4000. },
        )]
        .into();
        for delta in [0., 900., -1100.] {
            paint.points.push(Point::new(
                index as f32 * 270. + delta,
                if index % 3 == 2 { 100. } else { -3900. },
            ));
            let scene = cache.render(&doc, Some((1, &paint)));
            let mut expected = doc.clone();
            expected.layers[0].strokes.push(paint.clone());
            assert_material(&scene, &expected);
        }
        doc.layers[0].strokes.push(paint);
        assert_material(&cache.render(&doc, None), &doc);
    }
}

#[test]
fn generated_work_limits_and_cancellation_never_mutate_authoritative_documents() {
    use crate::scene::generation::{GenerationError, Limits, bounded};
    let document = Document::demo();
    let original = document.clone();
    let mut cache = SceneCache::default();
    assert!(matches!(
        bounded(
            Limits {
                vertices: 8,
                samples: 8,
                cells: 8
            },
            None,
            || cache.render(&document, None)
        ),
        Err(GenerationError::Budget)
    ));
    assert_eq!(document, original);
    let token = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1));
    let mut fresh = SceneCache::default();
    assert!(matches!(
        bounded(Limits::INTERACTIVE, Some((token, 0)), || fresh
            .render(&document, None)),
        Err(GenerationError::Cancelled)
    ));
    assert_eq!(document, original);
}
