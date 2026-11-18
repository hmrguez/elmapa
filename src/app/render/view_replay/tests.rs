use super::{Canvas, Entrance, timeline};
use crate::app::render::retained::{RetainedMesh, RetainedTree, TerrainMaterial};
use bevy::prelude::{Assets, Handle, World, default};
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
        &mut Assets::default(),
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

#[test]
fn ordered_water_shares_one_reveal_across_terrain_tiles() {
    let mut editor = Editor::blank();
    for x in [350., 450., 250., 550., 150., 650., 50.] {
        assert!(editor.commit_stroke(Stroke {
            brush: Brush::Water,
            creation_id: None,
            material: Some("ocean".into()),
            points: vec![Point::new(x, 110.)].into(),
            size: 146.,
            strength: 1.,
            seed: 42,
        }));
    }
    editor.add_layer();
    assert!(editor.add_label(Label::default()).is_some());
    let mut saved = serde_json::to_value(editor.document()).unwrap();
    saved["layers"][0]["terrain_replay"] = serde_json::json!("breadth_first");
    let doc = serde_json::from_value(saved).unwrap();
    let scene = SceneCache::default().render(&doc, None);
    let mut world = World::new();
    let mut canvas = Canvas::default();
    for batch in scene.batches() {
        let material = if scene.region(batch).is_some() {
            TerrainMaterial::Tile {
                handle: Handle::default(),
                opacity: 1.,
            }
        } else {
            TerrainMaterial::Ink(Handle::default())
        };
        canvas.meshes.insert(
            Arc::as_ptr(batch) as usize,
            RetainedMesh {
                entity: world.spawn_empty().id(),
                _source: batch.clone(),
                material,
            },
        );
    }
    let mut images = Assets::default();
    let cues = timeline(
        &scene,
        &canvas,
        &doc,
        ContentBounds {
            min: Point::new(-40., 0.),
            max: Point::new(740., 230.),
        },
        &mut images,
    );
    let washes: Vec<_> = cues
        .iter()
        .filter(|c| matches!(&c.entrance, Entrance::Wash { .. }))
        .collect();
    assert!(washes.len() > 2);
    assert!(
        washes.iter().all(|c| c.start == washes[0].start),
        "tile-local clocks restart the wash"
    );
    let label = cues
        .iter()
        .find(|c| matches!(&c.entrance, Entrance::Label(_)))
        .unwrap();
    assert!(label.start >= washes[0].start + washes[0].entrance.duration());
    let Entrance::Wash {
        order: Some(order), ..
    } = &washes[0].entrance
    else {
        panic!("the ordered area must use a shared arrival texture");
    };
    assert!(washes.iter().all(|c| match &c.entrance {
        Entrance::Wash {
            order: Some(other), ..
        } => other.texture == order.texture,
        _ => false,
    }));
    let image = images.get(&order.texture).unwrap();
    let sample = |x: f32| {
        let column = ((x - order.position.x) * order.position.z * image.width() as f32) as usize;
        let row = ((110. - order.position.y) * order.position.w * image.height() as f32) as usize;
        image.data.as_ref().unwrap()[row * image.width() as usize + column]
    };
    assert!(sample(350.) < sample(150.) && sample(350.) < sample(550.));
    assert!((sample(573.) as i16 - sample(579.) as i16).abs() < 12);
}
