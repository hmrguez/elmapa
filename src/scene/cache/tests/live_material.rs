use super::super::material::MaterialLayer;
use super::{assert_material, stroke};
use crate::document::{Brush, Document, Point};
use crate::scene::{SceneCache, build_scene};
use std::sync::Arc;

#[test]
fn live_material_matches_committed_material_for_every_brush() {
    for brush in Brush::ALL {
        let doc = Document::demo();
        let id = doc.layers[0].id;
        let mut cache = SceneCache::default();
        cache.render(&doc, None);
        let mut stroke = stroke(brush);
        for point in [
            None,
            Some(Point::new(640.0, 450.0)),
            Some(Point::new(770.0, 340.0)),
            Some(Point::new(560.0, 380.0)),
        ] {
            if let Some(point) = point {
                stroke.points.push(point);
            }
            let live = cache.render(&doc, Some((id, &stroke)));
            let mut committed = doc.clone();
            committed.layers[0].strokes.push(stroke.clone());
            assert_material(&live, &committed);
            // A pointer-only frame reuses all material batches.
            let repeated = cache.render(&doc, Some((id, &stroke)));
            assert!(
                live.batches()
                    .zip(repeated.batches())
                    .all(|(a, b)| Arc::ptr_eq(a, b))
            );
        }
        let live = cache.render(&doc, Some((id, &stroke)));
        let mut committed = doc.clone();
        committed.layers[0].strokes.push(stroke);
        let released = cache.render(&committed, None);
        assert_material(&released, &committed);
        assert!(
            live.batches()
                .zip(released.batches())
                .all(|(a, b)| Arc::ptr_eq(a, b)),
            "release must reuse the exact material for {}",
            brush.label()
        );
    }
}

#[test]
fn cancel_undo_redo_and_layer_changes_never_leave_live_material() {
    use crate::editor::Editor;
    let mut editor = Editor::demo();
    let mut cache = SceneCache::default();
    let original = cache.render(editor.document(), None);
    let id = editor.active_layer();
    let paint = stroke(Brush::Forest);
    let live = cache.render(editor.document(), Some((id, &paint)));
    assert!(live != original);
    assert!(cache.render(editor.document(), None) == original);
    cache.render(editor.document(), Some((id, &paint)));
    assert!(editor.commit_stroke(paint));
    assert_material(&cache.render(editor.document(), None), editor.document());
    assert!(editor.undo());
    assert!(cache.render(editor.document(), None) == original);
    assert!(editor.redo());
    assert_material(&cache.render(editor.document(), None), editor.document());
    editor.set_layer_opacity(id, 0.4);
    assert_material(&cache.render(editor.document(), None), editor.document());
    editor.move_layer(id, 1);
    assert_material(&cache.render(editor.document(), None), editor.document());
    editor.toggle_layer(id);
    assert_material(
        &cache.render(editor.document(), Some((id, &stroke(Brush::Land)))),
        editor.document(),
    );
    let mut replacement = Document::blank();
    replacement.width = 800.0;
    editor.replace_document(replacement);
    assert_material(&cache.render(editor.document(), None), editor.document());
}

#[test]
fn painting_reuses_other_layers_and_unaffected_materials() {
    let doc = Document::demo();
    let mut cache = SceneCache::default();
    let original = cache.render(&doc, None);
    let unchanged: Vec<_> = cache
        .layers
        .iter()
        .skip(1)
        .flat_map(MaterialLayer::batches)
        .chain(
            cache.layers[0]
                .terrain
                .regions(1)
                .flat_map(|(_, tile)| tile.batches.iter().cloned()),
        )
        .chain(
            cache.layers[0]
                .forest
                .iter()
                .map(|tree| tree.shapes.clone()),
        )
        .chain(
            cache.layers[0]
                .lines
                .iter()
                .flat_map(|(_, chunks)| chunks.iter().cloned()),
        )
        .collect();
    let live = cache.render(&doc, Some((doc.layers[0].id, &stroke(Brush::Land))));
    assert_eq!(cache.work().forest_candidates, 0);
    for batch in unchanged {
        assert!(
            live.batches().any(|next| Arc::ptr_eq(&batch, next)),
            "unrelated material was replaced: region={:?}, river={}, forest={}, matching={}",
            original.region(&batch),
            original.river_reach(&batch).is_some(),
            original.forest_tree(&batch).is_some(),
            live.batches().any(|b| *b == batch)
        );
    }
}

#[test]
fn successive_erasers_and_paint_follow_layer_order_at_partial_opacity() {
    let mut doc = Document::demo();
    let mut cache = SceneCache::default();
    for brush in [
        Brush::Erase,
        Brush::Land,
        Brush::Forest,
        Brush::Erase,
        Brush::River,
    ] {
        let paint = stroke(brush);
        doc.layers[0].opacity = 0.45;
        let id = doc.layers[0].id;
        let live = cache.render(&doc, Some((id, &paint)));
        doc.layers[0].strokes.push(paint);
        assert_material(&live, &doc);
        assert_material(&cache.render(&doc, None), &doc);
    }
}

#[test]
fn zero_strength_and_replaced_gestures_use_current_settings() {
    let doc = Document::demo();
    let mut cache = SceneCache::default();
    let original = cache.render(&doc, None);
    for brush in Brush::ALL {
        let mut paint = stroke(brush);
        paint.strength = 0.0;
        assert!(
            cache
                .render(&doc, Some((doc.layers[0].id, &paint)))
                .shapes()
                .eq(original.shapes())
        );
        paint.strength = 0.5;
        paint.seed += 1;
        let live = cache.render(&doc, Some((doc.layers[0].id, &paint)));
        let mut committed = doc.clone();
        committed.layers[0].strokes.push(paint);
        assert_material(&live, &committed);
    }
}

#[test]
fn live_material_preserves_open_world_islands_as_tiles_join() {
    let mut doc = Document::blank();
    for brush in [Brush::Land, Brush::Water, Brush::Forest] {
        for x in [0.0, 9000.0] {
            let mut paint = stroke(brush);
            paint.points = vec![Point::new(x, -600.0)].into();
            doc.layers[0].strokes.push(paint);
        }
    }
    for brush in Brush::ALL {
        let mut cache = SceneCache::default();
        let original = cache.render(&doc, None);
        let mut paint = stroke(brush);
        paint.points.clear();
        for point in [
            Point::new(-500.0, -600.0),
            Point::new(0.0, -600.0),
            Point::new(4000.0, -600.0),
            Point::new(9000.0, -600.0),
        ] {
            paint.points.push(point);
            let live = cache.render(&doc, Some((doc.layers[0].id, &paint)));
            let mut committed = doc.clone();
            committed.layers[0].strokes.push(paint.clone());
            assert_material(&live, &committed);
        }
        assert!(cache.render(&doc, None) == original);
        let live = cache.render(&doc, Some((doc.layers[0].id, &paint)));
        let mut committed = doc.clone();
        committed.layers[0].strokes.push(paint);
        assert!(cache.render(&committed, None) == live);
    }
}

#[test]
#[ignore = "manual rendering benchmark; run with --ignored --nocapture"]
fn live_brush_render_timing() {
    use std::time::Instant;
    for brush in [Brush::Land, Brush::Forest, Brush::Mountain, Brush::Erase] {
        let doc = Document::demo();
        let mut cache = SceneCache::default();
        cache.render(&doc, None);
        let mut paint = stroke(brush);
        let mut cached = std::time::Duration::ZERO;
        let mut rebuilt = std::time::Duration::ZERO;
        for i in 0..120 {
            paint.points.push(Point::new(
                250.0 + i as f32 * 8.0,
                500.0 + (i as f32 * 0.1).sin() * 110.0,
            ));
            let start = Instant::now();
            std::hint::black_box(cache.render(&doc, Some((doc.layers[0].id, &paint))));
            cached += start.elapsed();
            let mut committed = doc.clone();
            committed.layers[0].strokes.push(paint.clone());
            let start = Instant::now();
            std::hint::black_box(build_scene(&committed));
            rebuilt += start.elapsed();
        }
        eprintln!(
            "{}: cached {:.2} ms/update, full rebuild {:.2} ms/update ({:.1}x)",
            brush.label(),
            cached.as_secs_f64() * 1000.0 / 120.0,
            rebuilt.as_secs_f64() * 1000.0 / 120.0,
            rebuilt.as_secs_f64() / cached.as_secs_f64()
        );
    }
}

#[test]
fn release_advances_unrendered_endpoint_and_gestures_between_frames() {
    for brush in Brush::ALL {
        for render_live in [false, true] {
            let mut doc = Document::demo();
            let id = doc.layers[1].id;
            let mut cache = SceneCache::default();
            let before = cache.render(&doc, None);
            let mut paint = stroke(brush);
            paint.points = vec![Point::new(-500., -500.)].into();
            let displayed = if render_live {
                cache.render(&doc, Some((id, &paint)))
            } else {
                before
            };
            paint.points.push(Point::new(-490., -500.));
            doc.layers[1].strokes.push(paint);
            let released = cache.render(&doc, None);
            assert_material(&released, &doc);
            for old in displayed
                .batches()
                .filter(|b| displayed.forest_tree(b).is_some())
            {
                assert!(
                    released.batches().any(|new| Arc::ptr_eq(old, new)),
                    "lost existing tree on {:?} release (live frame: {})",
                    brush,
                    render_live
                );
            }
        }
    }
}
