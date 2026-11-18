use super::{Canvas, Entrance, timeline};
use crate::app::render::retained::{RetainedMesh, RetainedTree, TerrainMaterial};
use bevy::prelude::{Handle, World, default};
use elmapa::{
    document::{Brush, ContentBounds, Label, Point, Stroke},
    editor::Editor,
    scene::SceneCache,
};
use std::sync::Arc;

#[test]
fn replay_keeps_stroke_timing_and_sorts_trees_and_peaks_by_ground_depth() {
    let mut editor = Editor::blank();
    let paint = |brush, x, size| Stroke {
        brush,
        creation_id: None,
        material: None,
        points: vec![Point::new(x, 100.)].into(),
        size,
        strength: 1.,
        seed: 42,
    };
    let mut cache = SceneCache::default();
    let mut forest = paint(Brush::Forest, 100., 150.);
    forest.points.push_point(Point::new(330., 100.));
    assert!(editor.commit_stroke(forest));
    let original = cache.render(editor.document(), None).forest_trees().count();
    assert!(editor.commit_stroke(paint(Brush::Erase, 320., 175.)));
    assert!(editor.commit_stroke(paint(Brush::Forest, 9000., 150.)));
    editor.add_layer();
    assert!(
        editor
            .add_label(Label {
                position: Point::new(150., 160.),
                ..default()
            })
            .is_some()
    );
    assert!(editor.select_layer(1));
    assert!(editor.commit_stroke(paint(Brush::Mountain, 130., 170.)));
    assert!(editor.commit_stroke(paint(Brush::River, 125., 120.)));

    let scene = cache.render(editor.document(), None);
    let mut world = World::new();
    let mut canvas = Canvas::default();
    for (order, batch) in scene.batches().enumerate() {
        let entity = world.spawn_empty().id();
        if let Some(tree) = scene.forest_tree(batch) {
            canvas.trees.insert(
                tree.id,
                RetainedTree {
                    entity,
                    tree: tree.clone(),
                    order,
                },
            );
        } else {
            canvas.meshes.insert(
                Arc::as_ptr(batch) as usize,
                RetainedMesh {
                    entity,
                    _source: batch.clone(),
                    material: if scene.river_reach(batch).is_some() {
                        TerrainMaterial::River(Handle::default())
                    } else {
                        TerrainMaterial::Ink(Handle::default())
                    },
                },
            );
        }
    }

    let cues = timeline(
        &scene,
        &canvas,
        editor.document(),
        ContentBounds {
            min: Point::new(0., 0.),
            max: Point::new(450., 300.),
        },
    );
    let peaks: Vec<_> = cues
        .iter()
        .filter_map(|cue| match &cue.entrance {
            Entrance::Mountain(part) => Some((cue, part)),
            _ => None,
        })
        .collect();
    assert_eq!(peaks.len(), 2);
    let name = cues
        .iter()
        .find(|c| matches!(c.entrance, Entrance::Label(_)))
        .unwrap();
    let river = cues
        .iter()
        .find(|c| matches!(c.entrance, Entrance::River(..)))
        .unwrap();
    let trees: Vec<_> = cues
        .iter()
        .filter_map(|cue| match &cue.entrance {
            Entrance::Tree(tree) => Some((cue, tree)),
            _ => None,
        })
        .collect();
    assert!(!trees.is_empty() && trees.len() < original);
    assert_eq!(
        trees.len(),
        scene.forest_trees().filter(|tree| tree.id.1 == 0).count()
    );
    for (peak, part) in peaks {
        assert!(name.start < peak.start && peak.start < river.start);
        assert!(trees.iter().any(|(_, tree)| tree.center.y <= part.base.y));
        assert!(trees.iter().any(|(_, tree)| tree.center.y > part.base.y));
        for (cue, tree) in &trees {
            assert!(cue.start < name.start && cue.z > river.z);
            assert_eq!(cue.z > peak.z, tree.center.y > part.base.y);
        }
    }
}
