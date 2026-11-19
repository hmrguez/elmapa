use super::{assert_material, stroke};
use crate::document::{Brush, Document, Point, Stroke};
use crate::scene::SceneCache;
use std::sync::Arc;

#[test]
fn trees_and_mountain_flanks_follow_ground_depth_within_a_layer() {
    for brushes in [
        [Brush::Forest, Brush::Mountain, Brush::River, Brush::Path],
        [Brush::Path, Brush::River, Brush::Mountain, Brush::Forest],
    ] {
        let mut doc = Document::blank();
        doc.layers[0].strokes = brushes.into_iter().map(stroke).collect();
        let mut cache = SceneCache::default();
        let scene = cache.render(&doc, None);
        let [left, right] = scene
            .batches()
            .enumerate()
            .filter_map(|(i, batch)| scene.mountain_part(batch).map(|part| (i, part)))
            .collect::<Vec<_>>()[..]
        else {
            panic!("expected two flanks of one peak");
        };
        assert_eq!(left.0 + 1, right.0);
        assert_eq!(left.1.id, right.1.id);
        let mut behind = 0;
        let mut ahead = 0;
        let last_river = scene
            .batches()
            .enumerate()
            .filter_map(|(i, batch)| scene.river_reach(batch).map(|_| i))
            .last()
            .unwrap();
        let last_trail = scene
            .batches()
            .enumerate()
            .filter_map(|(i, batch)| scene.path_part(batch).map(|_| i))
            .inspect(|&i| assert!(last_river < i, "trails must cover rivers"))
            .last()
            .unwrap();
        for (i, batch) in scene.batches().enumerate() {
            let Some(tree) = scene.forest_tree(batch) else {
                continue;
            };
            if tree.center.y > left.1.base.y {
                assert!(
                    right.0 < i,
                    "southern trees must cover both mountain flanks"
                );
                ahead += 1;
            } else {
                assert!(i < left.0, "the mountain must cover northern trees");
                behind += 1;
            }
            assert!(last_trail < i, "trees must cover trails");
        }
        assert!(behind > 0 && ahead > 0);
        assert_material(&scene, &doc);

        // An upper mountain layer still covers a lower trail layer.
        let mut upper = doc.layers[0].clone();
        upper.id = 2;
        upper.strokes = vec![stroke(Brush::Mountain)].into();
        let mut upper_only = doc.clone();
        upper_only.layers = vec![upper.clone()].into();
        let top = crate::scene::build_scene(&upper_only);
        doc.layers.push(upper);
        let scene = cache.render(&doc, None);
        let shapes: Vec<_> = scene.shapes().cloned().collect();
        assert_eq!(&shapes[shapes.len() - top.len()..], top.as_slice());
        assert_material(&scene, &doc);
    }
}

#[test]
fn trails_draw_below_mountains_with_or_without_a_separate_layer() {
    for separate_layer in [false, true] {
        let mut doc = Document::blank();
        doc.layers[0].strokes.push(stroke(Brush::Mountain));
        if separate_layer {
            let mut upper = doc.layers[0].clone();
            upper.id = 2;
            upper.strokes = Default::default();
            doc.layers.push(upper);
            doc.next_id = 3;
        }
        let trail_layer = doc.layers.last().unwrap().id;
        let trail = Stroke {
            points: vec![Point::new(480., 375.), Point::new(560., 375.)].into(),
            ..stroke(Brush::Path)
        };
        let mut cache = SceneCache::default();
        let live = cache.render(&doc, Some((trail_layer, &trail)));
        doc.layers.last_mut().unwrap().strokes.push(trail);
        let released = cache.render(&doc, None);
        for scene in [&live, &released] {
            let first_peak = scene
                .batches()
                .position(|batch| scene.mountain_part(batch).is_some())
                .unwrap();
            let last_trail = scene
                .batches()
                .enumerate()
                .filter(|(_, batch)| scene.path_part(batch).is_some())
                .map(|(index, _)| index)
                .last()
                .unwrap();
            assert!(last_trail < first_peak, "mountains must cover trails");
        }
    }
}

#[test]
fn trails_draw_between_rivers_and_trees_across_layers() {
    let mut doc = Document::blank();
    doc.layers[0].strokes = vec![stroke(Brush::Forest), stroke(Brush::Path)].into();
    let mut upper = doc.layers[0].clone();
    upper.id = 2;
    upper.strokes = vec![Stroke {
        points: vec![Point::new(800., 390.)].into(),
        ..stroke(Brush::Mountain)
    }]
    .into();
    let mut top = upper.clone();
    top.id = 3;
    top.strokes = vec![stroke(Brush::Path)].into();
    doc.layers.extend([upper, top]);
    doc.next_id = 4;

    let river = Stroke {
        points: vec![Point::new(520., 380.), Point::new(520., 400.)].into(),
        ..stroke(Brush::River)
    };
    let mut cache = SceneCache::default();
    cache.render(&doc, None);
    let live = cache.render(&doc, Some((2, &river)));
    doc.layers[1].strokes.push(river.clone());
    let released = cache.render(&doc, None);
    let committed = cache.committed_scene();

    for scene in [&live, &released, &committed] {
        let last_river = scene
            .batches()
            .enumerate()
            .filter_map(|(i, batch)| scene.river_reach(batch).map(|_| i))
            .last()
            .unwrap();
        let first_tree = scene
            .batches()
            .position(|batch| scene.forest_tree(batch).is_some())
            .unwrap();
        let trail_layers: Vec<_> = scene
            .batches()
            .enumerate()
            .filter_map(|(i, batch)| {
                let part = scene.path_part(batch)?;
                assert!(last_river < i && i < first_tree);
                Some(part.id.0)
            })
            .collect();
        assert!(trail_layers.contains(&1) && trail_layers.contains(&3));
        assert!(trail_layers.is_sorted());
        assert_material(scene, &doc);
    }

    doc.layers[1].strokes.push(Stroke {
        brush: Brush::Erase,
        ..river
    });
    let erased = cache.render(&doc, None);
    let mut nonempty = erased.batches().filter(|batch| !batch.is_empty());
    assert!(erased.path_part(nonempty.next().unwrap()).is_some());
    assert!(erased.mountain_part(nonempty.last().unwrap()).is_some());
    assert_material(&erased, &doc);
}

#[test]
fn committed_and_live_composition_keep_annotation_metadata_and_retained_geometry() {
    use crate::{
        document::{Label, Landmark},
        editor::Editor,
    };

    let mut editor = Editor::blank();
    let label = editor.add_label(Label::default()).unwrap();
    let landmark = editor.add_landmark(Landmark::default()).unwrap();
    let layer = editor.active_layer();
    assert!(editor.set_layer_opacity(layer, 0.4));

    let mut cache = SceneCache::default();
    let live = cache.render(editor.document(), Some((layer, &stroke(Brush::Land))));
    let committed = cache.committed_scene();

    let label_batch = live.batches().find(|b| live.label(b).is_some()).unwrap();
    assert_eq!(live.label(label_batch), editor.label(label));
    let landmark_batch = live
        .batches()
        .find(|b| live.landmark_part(b).is_some())
        .unwrap();
    assert_eq!(
        live.landmark_part(landmark_batch).unwrap().landmark.id,
        landmark
    );
    for batch in [label_batch, landmark_batch] {
        assert!(committed.batches().any(|b| Arc::ptr_eq(b, batch)));
        assert_eq!(committed.batch_opacity(batch), 0.4);
        assert_eq!(live.batch_opacity(batch), 0.4);
    }
    assert_eq!(committed.label(label_batch), live.label(label_batch));
    let committed_part = committed.landmark_part(landmark_batch).unwrap();
    let live_part = live.landmark_part(landmark_batch).unwrap();
    assert_eq!(committed_part.landmark, live_part.landmark);
    assert_eq!(committed_part.index, live_part.index);
    assert_eq!(committed_part.pivot, live_part.pivot);
}
